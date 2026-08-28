//! Canonical IDL publication through Solana's Program Metadata program.
//!
//! Pina deliberately delegates transaction planning to the official client.
//! Program Metadata may split writes across reallocations and temporary buffers;
//! duplicating that evolving planner here would create a second, unsafe protocol
//! implementation. The pinned adapter below was verified against upstream commit
//! `33eb527e124cc4a09d8aae448cd306a9bd87db14` and package version `0.9.0`.

use std::ffi::OsString;
use std::fmt;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::str::FromStr;

use atomic_write_file::AtomicWriteFile;
use codama_nodes::RootNode;
use ed25519_dalek::SigningKey;
use flate2::read::ZlibDecoder;
use serde::Serialize;
use serde_json::Value;
use solana_address::Address;
use url::Host;
use url::Url;

use crate::generate_idl;
use crate::project::Project;

/// Exact official package used for Program Metadata transaction planning.
pub const PROGRAM_METADATA_PACKAGE: &str = "@solana-program/program-metadata@0.9.0";

const IDL_SEED: &str = "idl";

/// Errors returned by canonical IDL network workflows.
#[derive(Debug, thiserror::Error)]
pub enum IdlMetadataError {
	#[error(transparent)]
	Project(#[from] crate::project::ProjectError),

	#[error("Failed to generate the local IDL: {0}")]
	Generate(#[from] crate::error::IdlError),

	#[error("Could not read {path:?}: {source}")]
	Read {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Could not write {path:?}: {source}")]
	Write {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Invalid IDL JSON at {path:?}: {source}")]
	Json {
		path: PathBuf,
		source: serde_json::Error,
	},

	#[error("Invalid Codama IDL at {path:?}: {reason}")]
	InvalidIdl { path: PathBuf, reason: String },

	#[error("Invalid program address {value:?}")]
	InvalidAddress { value: String },

	#[error("IDL program address {idl} does not match requested program {requested}")]
	ProgramMismatch { idl: String, requested: String },

	#[error("Invalid cluster `{cluster}`: {reason}")]
	InvalidCluster { cluster: String, reason: String },

	#[error("Invalid keypair file {path:?}: {reason}")]
	InvalidKeypair { path: PathBuf, reason: String },

	#[error("Invalid publication signer configuration: {reason}")]
	InvalidSignerConfiguration { reason: &'static str },

	#[error(
		"Could not run the pinned Program Metadata client with {command:?}: {source}. Install \
		 Node.js and npm, or pass an npx-compatible runner with --npx <COMMAND>"
	)]
	RunClient {
		command: String,
		source: std::io::Error,
	},

	#[error("The pinned Program Metadata client failed with status {status}: {details}")]
	ClientFailed { status: i32, details: String },

	#[error("The pinned Program Metadata client exceeded the {limit}-byte {stream} safety limit")]
	ClientOutputTooLarge { stream: &'static str, limit: usize },

	#[error("Program Metadata returned non-UTF-8 output")]
	NonUtf8,

	#[error("Program Metadata did not return any exported transactions")]
	MissingExport,

	#[error("Canonical IDL metadata is not direct zlib-compressed UTF-8 JSON: {reason}")]
	UnsupportedContent { reason: String },

	#[error("Unsafe filesystem path {path:?}: {reason}")]
	UnsafePath { path: PathBuf, reason: String },
}

/// A validated local Codama IDL document.
#[derive(Debug, Clone)]
pub struct LocalIdl {
	pub path: PathBuf,
	pub program_id: String,
	pub value: Value,
}

/// Result of semantically comparing two JSON IDLs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdlDiff {
	pub equal: bool,
	pub program_id: String,
	pub local: Value,
	pub on_chain: Value,
}

/// Options for the pinned official Program Metadata client.
#[derive(Clone)]
pub struct ClientOptions {
	pub npx: String,
	pub cluster: String,
}

impl fmt::Debug for ClientOptions {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter
			.debug_struct("ClientOptions")
			.field("npx", &self.npx)
			.field("cluster", &"<redacted>")
			.finish()
	}
}

/// Options for a canonical IDL publication.
#[derive(Debug)]
pub struct PublishOptions<'a> {
	pub local: &'a LocalIdl,
	pub authority: Option<&'a Path>,
	pub payer: Option<&'a Path>,
	pub priority_fee: u64,
	pub export: bool,
	pub export_authority: Option<&'a str>,
	pub export_encoding: &'a str,
}

/// Load a generated or file-backed IDL and enforce the Codama root contract.
///
/// # Errors
///
/// Returns an error when project discovery, generation, file access, JSON
/// parsing, or Codama root validation fails.
pub fn load_local_idl(
	project_dir: &Path,
	file: Option<&Path>,
) -> Result<LocalIdl, IdlMetadataError> {
	let (path, value) = if let Some(path) = file {
		const MAX_LOCAL_IDL_BYTES: u64 = 16 * 1024 * 1024;
		let metadata = std::fs::metadata(path).map_err(read_error(path))?;

		if !metadata.is_file() {
			return Err(IdlMetadataError::InvalidIdl {
				path: path.to_path_buf(),
				reason: "IDL path is not a regular file".to_owned(),
			});
		}

		if metadata.len() > MAX_LOCAL_IDL_BYTES {
			return Err(IdlMetadataError::InvalidIdl {
				path: path.to_path_buf(),
				reason: format!("IDL exceeds the {MAX_LOCAL_IDL_BYTES}-byte safety limit"),
			});
		}

		ensure_safe_path(path, true)?;
		let bytes = std::fs::read(path).map_err(read_error(path))?;
		let value = serde_json::from_slice(&bytes).map_err(json_error(path))?;

		(path.to_path_buf(), value)
	} else {
		let project = Project::discover(project_dir)?;
		let root = generate_idl(&project.program_dir, Some(&project.library_name))?;
		let value = serde_json::to_value(root)
			.unwrap_or_else(|error| panic!("Codama RootNode serialization failed: {error}"));

		(
			project
				.idl_dir
				.join(format!("{}.json", project.library_name)),
			value,
		)
	};
	let program_id = idl_program_id(&value, &path)?;

	Ok(LocalIdl {
		path,
		program_id,
		value,
	})
}

/// Resolve and validate a requested program ID against the IDL when available.
///
/// # Errors
///
/// Returns an error for malformed addresses or an IDL/target mismatch.
pub fn resolve_program_id(
	requested: Option<&str>,
	local: Option<&LocalIdl>,
) -> Result<String, IdlMetadataError> {
	let value = requested
		.or_else(|| local.map(|idl| idl.program_id.as_str()))
		.ok_or_else(|| {
			IdlMetadataError::InvalidIdl {
				path: PathBuf::from("<local project>"),
				reason: "program ID is unavailable; pass --program-id".to_owned(),
			}
		})?;
	let normalized = parse_address(value)?;

	if let Some(local) = local
		&& local.program_id != normalized
	{
		return Err(IdlMetadataError::ProgramMismatch {
			idl: local.program_id.clone(),
			requested: normalized,
		});
	}

	Ok(normalized)
}

/// Convert an explicit cluster moniker or safe RPC URL to the official client input.
///
/// # Errors
///
/// Returns an error for URLs containing credentials, query parameters, or
/// fragments, which are rejected to keep secrets out of subprocess arguments.
pub fn rpc_url(cluster: &str) -> Result<String, IdlMetadataError> {
	let known = match cluster {
		"mainnet" | "mainnet-beta" => Some("https://api.mainnet-beta.solana.com"),
		"devnet" => Some("https://api.devnet.solana.com"),
		"testnet" => Some("https://api.testnet.solana.com"),
		"localnet" | "localhost" => Some("http://127.0.0.1:8899"),
		_ => None,
	};

	if let Some(url) = known {
		return Ok(url.to_owned());
	}

	if cluster.chars().any(char::is_control) {
		return Err(IdlMetadataError::InvalidCluster {
			cluster: "<redacted>".to_owned(),
			reason: "RPC URLs must not contain control characters".to_owned(),
		});
	}

	let url = Url::parse(cluster).map_err(|_| {
		IdlMetadataError::InvalidCluster {
			cluster: "<redacted>".to_owned(),
			reason: "use mainnet-beta, devnet, testnet, localnet, or a valid HTTP(S) URL"
				.to_owned(),
		}
	})?;
	let is_http = url.scheme() == "http";
	let is_https = url.scheme() == "https";
	let is_loopback = match url.host() {
		Some(Host::Domain(domain)) => domain == "localhost",
		Some(Host::Ipv4(address)) => address.is_loopback(),
		Some(Host::Ipv6(address)) => address.is_loopback(),
		None => false,
	};
	let invalid_component = !url.username().is_empty()
		|| url.password().is_some()
		|| url.query().is_some()
		|| url.fragment().is_some()
		|| url.path() != "/";

	if (!is_http && !is_https) || invalid_component || (is_http && !is_loopback) {
		return Err(IdlMetadataError::InvalidCluster {
			cluster: "<redacted>".to_owned(),
			reason: "RPC URLs require HTTP(S), no credentials, query, fragment, or path, and \
			         plaintext HTTP only for an exact loopback host"
				.to_owned(),
		});
	}

	Ok(cluster.to_owned())
}

/// Return a credential-free cluster label suitable for terminal or JSON output.
#[must_use]
pub fn cluster_display(cluster: &str) -> String {
	match cluster {
		"mainnet" | "mainnet-beta" | "devnet" | "testnet" | "localnet" | "localhost" => {
			cluster.to_owned()
		}
		_ => {
			Url::parse(cluster).ok().map_or_else(
				|| "<redacted>".to_owned(),
				|url| url.origin().ascii_serialization(),
			)
		}
	}
}

/// Validate a Solana JSON keypair without printing its contents.
///
/// # Errors
///
/// Returns an error unless the file is a regular file containing exactly 64 bytes.
pub fn validate_keypair(path: &Path) -> Result<String, IdlMetadataError> {
	validated_keypair(path).map(|(_, address)| address)
}

fn validated_keypair(path: &Path) -> Result<([u8; 64], String), IdlMetadataError> {
	let metadata = std::fs::metadata(path).map_err(read_error(path))?;

	if !metadata.is_file() {
		return Err(IdlMetadataError::InvalidKeypair {
			path: path.to_path_buf(),
			reason: "path is not a regular file".to_owned(),
		});
	}

	if metadata.len() > 4_096 {
		return Err(IdlMetadataError::InvalidKeypair {
			path: path.to_path_buf(),
			reason: "keypair file exceeds the 4 KiB safety limit".to_owned(),
		});
	}

	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;

		if metadata.permissions().mode() & 0o077 != 0 {
			return Err(IdlMetadataError::InvalidKeypair {
				path: path.to_path_buf(),
				reason: "keypair file must not be readable or writable by group or other users"
					.to_owned(),
			});
		}
	}

	ensure_safe_path(path, true)?;
	let source = std::fs::read(path).map_err(read_error(path))?;
	let bytes: Vec<u8> = serde_json::from_slice(&source).map_err(|_| {
		IdlMetadataError::InvalidKeypair {
			path: path.to_path_buf(),
			reason: "expected a JSON array of 64 byte values".to_owned(),
		}
	})?;

	if bytes.len() != 64 {
		return Err(IdlMetadataError::InvalidKeypair {
			path: path.to_path_buf(),
			reason: format!("expected 64 bytes, found {}", bytes.len()),
		});
	}

	let mut keypair = [0; 64];
	keypair.copy_from_slice(&bytes);
	let signing_key = SigningKey::from_keypair_bytes(&keypair).map_err(|_| {
		IdlMetadataError::InvalidKeypair {
			path: path.to_path_buf(),
			reason: "secret and public key bytes do not form a valid Ed25519 keypair".to_owned(),
		}
	})?;

	let address = Address::from(signing_key.verifying_key().to_bytes()).to_string();
	Ok((keypair, address))
}

/// Fetch and parse the canonical on-chain IDL.
///
/// # Errors
///
/// Returns an error when RPC validation, the official client, or JSON parsing fails.
pub fn fetch_idl(client: &ClientOptions, program_id: &str) -> Result<Value, IdlMetadataError> {
	let rpc = rpc_url(&client.cluster)?;
	let args = vec![
		OsString::from("--yes"),
		OsString::from(PROGRAM_METADATA_PACKAGE),
		OsString::from("fetch"),
		OsString::from(IDL_SEED),
		OsString::from(parse_address(program_id)?),
		OsString::from("--rpc"),
		OsString::from(rpc),
		OsString::from("--raw"),
	];

	let output = run_official_client(client, &args)?;
	let compressed = extract_raw_hex(&output)?;
	const MAX_IDL_BYTES: u64 = 16 * 1024 * 1024;
	let decoder = ZlibDecoder::new(compressed.as_slice());
	let mut bytes = Vec::new();
	Read::read_to_end(&mut decoder.take(MAX_IDL_BYTES + 1), &mut bytes).map_err(|error| {
		IdlMetadataError::UnsupportedContent {
			reason: format!("zlib decompression failed: {error}"),
		}
	})?;

	if bytes.len() as u64 > MAX_IDL_BYTES {
		return Err(IdlMetadataError::UnsupportedContent {
			reason: format!("decompressed IDL exceeds the {MAX_IDL_BYTES}-byte safety limit"),
		});
	}
	let value = serde_json::from_slice(&bytes).map_err(|source| {
		IdlMetadataError::Json {
			path: PathBuf::from("<on-chain IDL>"),
			source,
		}
	})?;
	let fetched_program = idl_program_id(&value, Path::new("<on-chain IDL>"))?;

	if fetched_program != program_id {
		return Err(IdlMetadataError::ProgramMismatch {
			idl: fetched_program,
			requested: program_id.to_owned(),
		});
	}

	Ok(value)
}

/// Compare a validated local IDL to canonical on-chain JSON semantically.
#[must_use]
pub fn compare_idls(local: &LocalIdl, on_chain: Value) -> IdlDiff {
	IdlDiff {
		equal: local.value == on_chain,
		program_id: local.program_id.clone(),
		local: local.value.clone(),
		on_chain,
	}
}

/// Publish or export a canonical IDL transaction plan.
///
/// # Errors
///
/// Returns an error when validation or the pinned official client fails.
pub fn publish_idl(
	client: &ClientOptions,
	options: &PublishOptions<'_>,
) -> Result<Option<String>, IdlMetadataError> {
	validate_publish_signers(options)?;
	let validated_program_id = idl_program_id(&options.local.value, &options.local.path)?;

	if validated_program_id != options.local.program_id {
		return Err(IdlMetadataError::ProgramMismatch {
			idl: validated_program_id,
			requested: options.local.program_id.clone(),
		});
	}

	let rpc = rpc_url(&client.cluster)?;
	let program_id = parse_address(&options.local.program_id)?;
	let mut args = vec![
		OsString::from("--yes"),
		OsString::from(PROGRAM_METADATA_PACKAGE),
		OsString::from("write"),
		OsString::from(IDL_SEED),
		OsString::from(program_id),
	];
	let serialized = serde_json::to_vec_pretty(&options.local.value)
		.unwrap_or_else(|error| panic!("serde_json::Value serialization failed: {error}"));
	let temp = tempfile::NamedTempFile::new().map_err(write_error(Path::new("<temporary IDL>")))?;
	std::fs::write(temp.path(), serialized).map_err(write_error(temp.path()))?;
	args.push(temp.path().as_os_str().to_owned());
	args.extend([
		OsString::from("--compression"),
		OsString::from("zlib"),
		OsString::from("--encoding"),
		OsString::from("utf8"),
		OsString::from("--format"),
		OsString::from("json"),
	]);

	args.extend([
		OsString::from("--rpc"),
		OsString::from(rpc),
		OsString::from("--priority-fees"),
		OsString::from(options.priority_fee.to_string()),
	]);
	let authority_copy = options.authority.map(prepare_keypair).transpose()?;
	let payer_copy = options.payer.map(prepare_keypair).transpose()?;

	if let Some(authority) = &authority_copy {
		args.push(OsString::from("--keypair"));
		args.push(authority.path.as_os_str().to_owned());

		if let Some(payer) = &payer_copy {
			args.push(OsString::from("--payer"));
			args.push(payer.path.as_os_str().to_owned());
		}
	}

	if options.export {
		args.push(OsString::from("--export"));

		if let Some(authority) = options.export_authority {
			args.push(OsString::from(parse_address(authority)?));
		}

		args.extend([
			OsString::from("--export-encoding"),
			OsString::from(options.export_encoding),
		]);
	}

	let output = run_official_client(client, &args)?;

	if !options.export {
		return Ok(None);
	}

	let output = String::from_utf8(output).map_err(|_| IdlMetadataError::NonUtf8)?;

	if !output.contains("Transaction #") {
		return Err(IdlMetadataError::MissingExport);
	}

	Ok(Some(output))
}

struct PreparedKeypair {
	_directory: tempfile::TempDir,
	path: PathBuf,
}

fn prepare_keypair(path: &Path) -> Result<PreparedKeypair, IdlMetadataError> {
	let (bytes, _) = validated_keypair(path)?;
	let directory = tempfile::tempdir().map_err(write_error(Path::new("<temporary keypair>")))?;

	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;

		let mut permissions = std::fs::metadata(directory.path())
			.map_err(read_error(directory.path()))?
			.permissions();
		permissions.set_mode(0o700);
		std::fs::set_permissions(directory.path(), permissions)
			.map_err(write_error(directory.path()))?;
	}

	let stable_path = directory.path().join("keypair.json");
	let serialized = serde_json::to_vec(bytes.as_slice())
		.unwrap_or_else(|error| panic!("fixed byte-array serialization failed: {error}"));
	let mut options = std::fs::OpenOptions::new();
	options.write(true).create_new(true);

	#[cfg(unix)]
	{
		use std::os::unix::fs::OpenOptionsExt;

		options.mode(0o600);
	}

	let mut file = options
		.open(&stable_path)
		.map_err(write_error(&stable_path))?;
	file.write_all(&serialized)
		.and_then(|()| file.flush())
		.map_err(write_error(&stable_path))?;

	Ok(PreparedKeypair {
		_directory: directory,
		path: stable_path,
	})
}

fn validate_publish_signers(options: &PublishOptions<'_>) -> Result<(), IdlMetadataError> {
	let local_authority = options.authority.is_some();
	let exported_authority = options.export_authority.is_some();

	if options.payer.is_some() && !local_authority {
		return Err(IdlMetadataError::InvalidSignerConfiguration {
			reason: "a payer keypair requires a local authority keypair",
		});
	}

	if exported_authority && (local_authority || options.payer.is_some()) {
		return Err(IdlMetadataError::InvalidSignerConfiguration {
			reason: "an exported authority cannot be combined with local authority or payer \
			         keypairs",
		});
	}

	if !options.export && !local_authority {
		return Err(IdlMetadataError::InvalidSignerConfiguration {
			reason: "direct publication requires a local authority keypair",
		});
	}

	if options.export && !exported_authority && !local_authority {
		return Err(IdlMetadataError::InvalidSignerConfiguration {
			reason: "local transaction export requires a local authority keypair",
		});
	}

	Ok(())
}

/// Atomically write user-visible IDL or export output.
///
/// # Errors
///
/// Returns an error when the destination cannot be opened, written, or committed.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), IdlMetadataError> {
	let parent = path.parent().unwrap_or_else(|| Path::new("."));
	ensure_safe_path(path, false)?;
	std::fs::create_dir_all(parent).map_err(write_error(parent))?;
	let mut file = AtomicWriteFile::open(path).map_err(write_error(path))?;
	file.write_all(contents)
		.and_then(|()| file.commit())
		.map_err(write_error(path))
}

fn idl_program_id(value: &Value, path: &Path) -> Result<String, IdlMetadataError> {
	let root: RootNode = serde_json::from_value(value.clone()).map_err(|error| {
		IdlMetadataError::InvalidIdl {
			path: path.to_path_buf(),
			reason: error.to_string(),
		}
	})?;

	if root.standard != "codama" {
		return Err(IdlMetadataError::InvalidIdl {
			path: path.to_path_buf(),
			reason: "expected the codama standard".to_owned(),
		});
	}

	parse_address(&root.program.public_key)
}

fn parse_address(value: &str) -> Result<String, IdlMetadataError> {
	Address::from_str(value)
		.map(|address| address.to_string())
		.map_err(|_| {
			IdlMetadataError::InvalidAddress {
				value: value.to_owned(),
			}
		})
}

fn run_official_client(
	client: &ClientOptions,
	args: &[OsString],
) -> Result<Vec<u8>, IdlMetadataError> {
	const MAX_STDOUT_BYTES: usize = 32 * 1024 * 1024;
	const MAX_STDERR_BYTES: usize = 64 * 1024;
	run_official_client_with_limits(client, args, MAX_STDOUT_BYTES, MAX_STDERR_BYTES)
}

fn run_official_client_with_limits(
	client: &ClientOptions,
	args: &[OsString],
	max_stdout_bytes: usize,
	max_stderr_bytes: usize,
) -> Result<Vec<u8>, IdlMetadataError> {
	let mut child = Command::new(&client.npx)
		.args(args)
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.map_err(run_client_error(client))?;
	let stdout = child
		.stdout
		.take()
		.ok_or_else(|| std::io::Error::other("Program Metadata stdout pipe was not created"))
		.map_err(run_client_error(client))?;
	let stderr = child
		.stderr
		.take()
		.ok_or_else(|| std::io::Error::other("Program Metadata stderr pipe was not created"))
		.map_err(run_client_error(client))?;
	let (status, stdout, stderr) = std::thread::scope(|scope| {
		let stdout = scope.spawn(|| read_bounded(stdout, max_stdout_bytes));
		let stderr = scope.spawn(|| read_bounded(stderr, max_stderr_bytes));
		let status = child.wait()?;
		let stdout = join_output_reader(stdout)?;
		let stderr = join_output_reader(stderr)?;

		Ok::<_, std::io::Error>((status, stdout, stderr))
	})
	.map_err(run_client_error(client))?;

	if stdout.overflowed {
		return Err(IdlMetadataError::ClientOutputTooLarge {
			stream: "stdout",
			limit: max_stdout_bytes,
		});
	}

	if stderr.overflowed {
		return Err(IdlMetadataError::ClientOutputTooLarge {
			stream: "stderr",
			limit: max_stderr_bytes,
		});
	}

	if status.success() {
		return Ok(stdout.bytes);
	}

	let details = sanitize_diagnostic(&String::from_utf8_lossy(&stderr.bytes));

	Err(IdlMetadataError::ClientFailed {
		status: status.code().unwrap_or(-1),
		details,
	})
}

struct BoundedRead {
	bytes: Vec<u8>,
	overflowed: bool,
}

fn read_bounded(reader: impl Read, limit: usize) -> std::io::Result<BoundedRead> {
	let mut reader = std::io::BufReader::new(reader);
	let mut chunk = [0; 8 * 1024];
	let mut bytes = Vec::with_capacity(limit.min(chunk.len()));
	let mut overflowed = false;

	loop {
		let read = reader.read(&mut chunk)?;

		if read == 0 {
			return Ok(BoundedRead { bytes, overflowed });
		}

		let remaining = limit.saturating_sub(bytes.len());
		let retained = remaining.min(read);
		bytes.extend_from_slice(&chunk[..retained]);
		overflowed |= retained < read;
	}
}

fn join_output_reader(
	handle: std::thread::ScopedJoinHandle<'_, std::io::Result<BoundedRead>>,
) -> std::io::Result<BoundedRead> {
	handle
		.join()
		.map_err(|_| std::io::Error::other("Program Metadata output reader panicked"))?
}

fn run_client_error(
	client: &ClientOptions,
) -> impl FnOnce(std::io::Error) -> IdlMetadataError + '_ {
	move |source| {
		IdlMetadataError::RunClient {
			command: client.npx.clone(),
			source,
		}
	}
}

fn extract_raw_hex(output: &[u8]) -> Result<Vec<u8>, IdlMetadataError> {
	let output = std::str::from_utf8(output).map_err(|_| IdlMetadataError::NonUtf8)?;
	let line = output
		.lines()
		.rev()
		.map(str::trim)
		.find(|line| !line.is_empty())
		.ok_or_else(|| {
			IdlMetadataError::UnsupportedContent {
				reason: "the official client returned no raw account data".to_owned(),
			}
		})?;

	if line.len() > 8 * 1024 * 1024 {
		return Err(IdlMetadataError::UnsupportedContent {
			reason: "raw compressed IDL exceeds the 4 MiB safety limit".to_owned(),
		});
	}

	if !line.len().is_multiple_of(2) || !line.bytes().all(|byte| byte.is_ascii_hexdigit()) {
		return Err(IdlMetadataError::UnsupportedContent {
			reason: "expected direct raw hexadecimal data; URL and external-account sources are \
			         not supported"
				.to_owned(),
		});
	}

	line.as_bytes()
		.chunks_exact(2)
		.map(|pair| {
			let text = std::str::from_utf8(pair)
				.unwrap_or_else(|error| panic!("validated hexadecimal was not UTF-8: {error}"));
			Ok(u8::from_str_radix(text, 16)
				.unwrap_or_else(|error| panic!("validated hexadecimal did not decode: {error}")))
		})
		.collect()
}

fn read_error(path: &Path) -> impl FnOnce(std::io::Error) -> IdlMetadataError + '_ {
	move |source| {
		IdlMetadataError::Read {
			path: path.to_path_buf(),
			source,
		}
	}
}

fn write_error(path: &Path) -> impl FnOnce(std::io::Error) -> IdlMetadataError + '_ {
	move |source| {
		IdlMetadataError::Write {
			path: path.to_path_buf(),
			source,
		}
	}
}

fn json_error(path: &Path) -> impl FnOnce(serde_json::Error) -> IdlMetadataError + '_ {
	move |source| {
		IdlMetadataError::Json {
			path: path.to_path_buf(),
			source,
		}
	}
}

fn sanitize_diagnostic(value: &str) -> String {
	let sanitized = value
		.chars()
		.filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
		.take(4_096)
		.collect::<String>();
	let sanitized = sanitized.trim();

	if sanitized.is_empty() {
		"no diagnostic output".to_owned()
	} else {
		sanitized.to_owned()
	}
}

fn ensure_safe_path(path: &Path, include_self: bool) -> Result<(), IdlMetadataError> {
	if include_self || path.exists() {
		let metadata = std::fs::symlink_metadata(path).map_err(read_error(path))?;

		if is_link_or_reparse(&metadata) {
			return Err(IdlMetadataError::UnsafePath {
				path: path.to_path_buf(),
				reason: "the final path is a symbolic link or reparse point".to_owned(),
			});
		}
	}

	Ok(())
}

fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
	if metadata.file_type().is_symlink() {
		return true;
	}

	#[cfg(windows)]
	{
		use std::os::windows::fs::MetadataExt;

		return metadata.file_attributes() & 0x0400 != 0;
	}

	#[cfg(not(windows))]
	false
}

#[cfg(test)]
mod tests {
	#[cfg(unix)]
	use std::fmt::Write as _;
	use std::io::Cursor;

	#[cfg(unix)]
	use flate2::Compression;
	#[cfg(unix)]
	use flate2::write::ZlibEncoder;

	use super::*;

	fn idl(program_id: &str) -> Value {
		serde_json::json!({
			"kind": "rootNode",
			"standard": "codama",
			"version": "1.8.0",
			"program": {
				"kind": "programNode",
				"name": "fixture",
				"publicKey": program_id,
				"version": "0.0.0",
				"instructions": []
			},
			"additionalPrograms": []
		})
	}

	#[test]
	fn cluster_aliases_and_safe_urls_are_accepted() {
		assert_eq!(
			rpc_url("devnet").unwrap_or_else(|error| panic!("devnet failed: {error}")),
			"https://api.devnet.solana.com"
		);
		assert_eq!(
			rpc_url("https://rpc.example.test")
				.unwrap_or_else(|error| panic!("URL failed: {error}")),
			"https://rpc.example.test"
		);
		assert!(rpc_url("http://localhost.attacker.test").is_err());
		assert!(rpc_url("http://127.attacker.test").is_err());
		assert!(rpc_url("http://127.0.0.1:8899").is_ok());
		assert!(rpc_url("http://[::1]:8899").is_ok());
		assert!(rpc_url("file:///tmp/rpc").is_err());
	}

	#[test]
	fn credential_bearing_rpc_urls_are_rejected_and_redacted() {
		for cluster in [
			"https://user:secret@rpc.example.test",
			"https://rpc.example.test?token=secret",
			"https://rpc.example.test/private",
			"https://rpc.example.test:invalid",
			"https://rpc.example.test\n.evil.test",
			"ftp://rpc.example.test",
		] {
			let error = rpc_url(cluster)
				.expect_err("unsafe URL must fail")
				.to_string();
			assert!(!error.contains("secret"));
		}
	}

	#[test]
	fn client_debug_and_cluster_display_do_not_expose_rpc_details() {
		let client = ClientOptions {
			npx: "npx".to_owned(),
			cluster: "https://rpc.example.test".to_owned(),
		};
		let debug = format!("{client:?}");
		assert!(debug.contains("<redacted>"));
		assert!(!debug.contains("rpc.example.test"));
		assert_eq!(
			cluster_display("https://rpc.example.test:9443"),
			"https://rpc.example.test:9443"
		);
		assert_eq!(cluster_display("not a URL"), "<redacted>");
	}

	#[test]
	fn semantic_comparison_ignores_object_order_but_preserves_array_order() {
		let program_id = "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS";
		let local = LocalIdl {
			path: PathBuf::from("idl.json"),
			program_id: program_id.to_owned(),
			value: idl(program_id),
		};
		let reordered: Value = serde_json::from_str(
			&serde_json::to_string(&local.value)
				.unwrap_or_else(|error| panic!("fixture must serialize: {error}")),
		)
		.unwrap_or_else(|error| panic!("fixture must parse: {error}"));
		assert!(compare_idls(&local, reordered).equal);

		let mut changed = local.value.clone();
		changed["program"]["instructions"] = serde_json::json!([2, 1]);
		assert!(!compare_idls(&local, changed).equal);
	}

	#[test]
	fn target_program_must_match_the_idl() {
		let local = LocalIdl {
			path: PathBuf::from("idl.json"),
			program_id: "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS".to_owned(),
			value: Value::Null,
		};
		let error = resolve_program_id(
			Some("ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S"),
			Some(&local),
		)
		.expect_err("mismatch must fail");
		assert!(matches!(error, IdlMetadataError::ProgramMismatch { .. }));
	}

	#[test]
	fn malformed_codama_documents_are_rejected() {
		let malformed = serde_json::json!({
			"kind": "rootNode",
			"standard": "codama",
			"version": "1.8.0",
			"program": { "publicKey": "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS" }
		});
		let error = idl_program_id(&malformed, Path::new("bad.json"))
			.expect_err("incomplete root must fail");
		assert!(matches!(error, IdlMetadataError::InvalidIdl { .. }));
	}

	#[test]
	fn non_codama_roots_are_rejected_after_full_deserialization() {
		let mut value = idl("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");
		value["standard"] = Value::String("other".to_owned());
		assert!(matches!(
			idl_program_id(&value, Path::new("other.json")),
			Err(IdlMetadataError::InvalidIdl { .. })
		));
	}

	#[test]
	fn local_idl_loading_covers_files_generation_and_input_failures() {
		let directory = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
		let missing = directory.path().join("missing.json");
		assert!(matches!(
			load_local_idl(directory.path(), Some(&missing)),
			Err(IdlMetadataError::Read { .. })
		));
		assert!(matches!(
			load_local_idl(directory.path(), Some(directory.path())),
			Err(IdlMetadataError::InvalidIdl { .. })
		));

		let oversized = directory.path().join("oversized.json");
		let oversized_file = std::fs::File::create(&oversized)
			.unwrap_or_else(|error| panic!("oversized fixture create failed: {error}"));
		oversized_file
			.set_len(16 * 1024 * 1024 + 1)
			.unwrap_or_else(|error| panic!("oversized fixture resize failed: {error}"));
		assert!(matches!(
			load_local_idl(directory.path(), Some(&oversized)),
			Err(IdlMetadataError::InvalidIdl { .. })
		));

		let malformed = directory.path().join("malformed.json");
		std::fs::write(&malformed, b"{")
			.unwrap_or_else(|error| panic!("malformed fixture write failed: {error}"));
		assert!(matches!(
			load_local_idl(directory.path(), Some(&malformed)),
			Err(IdlMetadataError::Json { .. })
		));

		let generated_program = directory.path().join("generated-program");
		std::fs::create_dir_all(generated_program.join("src"))
			.unwrap_or_else(|error| panic!("generated fixture directory failed: {error}"));
		std::fs::write(
			generated_program.join("Cargo.toml"),
			"[package]\nname = \"anchor_declare_id\"\nversion = \"0.0.0\"\nedition = \
			 \"2024\"\n\n[lib]\ncrate-type = [\"cdylib\", \"lib\"]\n",
		)
		.unwrap_or_else(|error| panic!("generated fixture manifest failed: {error}"));
		std::fs::write(
			generated_program.join("src/lib.rs"),
			include_str!("../../../examples/anchor_declare_id/src/lib.rs"),
		)
		.unwrap_or_else(|error| panic!("generated fixture source failed: {error}"));
		let generated = load_local_idl(&generated_program, None)
			.unwrap_or_else(|error| panic!("generated IDL failed: {error}"));
		assert_eq!(
			generated.program_id,
			"Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS"
		);

		#[cfg(unix)]
		{
			use std::os::unix::fs::symlink;

			let link = directory.path().join("linked.json");
			symlink(&malformed, &link)
				.unwrap_or_else(|error| panic!("IDL symlink failed: {error}"));
			assert!(matches!(
				load_local_idl(directory.path(), Some(&link)),
				Err(IdlMetadataError::UnsafePath { .. })
			));
		}
	}

	#[test]
	fn program_resolution_requires_a_valid_available_address() {
		assert!(matches!(
			resolve_program_id(None, None),
			Err(IdlMetadataError::InvalidIdl { .. })
		));
		assert!(matches!(
			resolve_program_id(Some("not-an-address"), None),
			Err(IdlMetadataError::InvalidAddress { .. })
		));
	}

	#[test]
	fn official_package_policy_is_exactly_pinned() {
		assert_eq!(
			PROGRAM_METADATA_PACKAGE,
			"@solana-program/program-metadata@0.9.0"
		);
	}

	#[test]
	fn valid_keypairs_are_verified_cryptographically() {
		let directory = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
		let path = directory.path().join("authority.json");
		let keypair = SigningKey::from_bytes(&[7; 32]).to_keypair_bytes();
		std::fs::write(
			&path,
			serde_json::to_vec(keypair.as_slice())
				.unwrap_or_else(|error| panic!("keypair serialization failed: {error}")),
		)
		.unwrap_or_else(|error| panic!("keypair write failed: {error}"));
		set_private_permissions(&path);

		let address = validate_keypair(&path)
			.unwrap_or_else(|error| panic!("valid keypair was rejected: {error}"));
		assert_eq!(
			address,
			Address::from(keypair[32..].try_into().unwrap_or([0; 32])).to_string()
		);

		let mut mismatched = keypair;
		mismatched[63] ^= 1;
		std::fs::write(
			&path,
			serde_json::to_vec(mismatched.as_slice())
				.unwrap_or_else(|error| panic!("keypair serialization failed: {error}")),
		)
		.unwrap_or_else(|error| panic!("keypair write failed: {error}"));
		assert!(validate_keypair(&path).is_err());
	}

	#[test]
	fn malformed_and_non_file_keypairs_are_rejected() {
		let directory = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
		assert!(matches!(
			validate_keypair(directory.path()),
			Err(IdlMetadataError::InvalidKeypair { .. })
		));

		let malformed = directory.path().join("malformed.json");
		std::fs::write(&malformed, b"not json")
			.unwrap_or_else(|error| panic!("malformed fixture write failed: {error}"));
		set_private_permissions(&malformed);
		assert!(matches!(
			validate_keypair(&malformed),
			Err(IdlMetadataError::InvalidKeypair { .. })
		));

		let short = directory.path().join("short.json");
		std::fs::write(&short, b"[]")
			.unwrap_or_else(|error| panic!("short fixture write failed: {error}"));
		set_private_permissions(&short);
		assert!(validate_keypair(&short).is_err());
	}

	#[test]
	fn oversized_keypair_files_are_rejected_before_reading() {
		let directory = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
		let path = directory.path().join("oversized.json");
		std::fs::write(&path, vec![b'0'; 4_097])
			.unwrap_or_else(|error| panic!("oversized fixture write failed: {error}"));
		set_private_permissions(&path);
		let error = validate_keypair(&path).expect_err("oversized keypair must fail");
		assert!(error.to_string().contains("4 KiB"));
	}

	#[cfg(unix)]
	#[test]
	fn shared_unix_keypair_files_are_rejected() {
		use std::os::unix::fs::PermissionsExt;

		let directory = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
		let path = directory.path().join("shared.json");
		std::fs::write(&path, b"[]")
			.unwrap_or_else(|error| panic!("shared fixture write failed: {error}"));
		let mut permissions = std::fs::metadata(&path)
			.unwrap_or_else(|error| panic!("keypair metadata failed: {error}"))
			.permissions();
		permissions.set_mode(0o640);
		std::fs::set_permissions(&path, permissions)
			.unwrap_or_else(|error| panic!("keypair permissions failed: {error}"));
		let error = validate_keypair(&path).expect_err("shared keypair must fail");
		assert!(error.to_string().contains("group or other users"));
	}

	#[cfg(unix)]
	fn set_private_permissions(path: &Path) {
		use std::os::unix::fs::PermissionsExt;

		let mut permissions = std::fs::metadata(path)
			.unwrap_or_else(|error| panic!("keypair metadata failed: {error}"))
			.permissions();
		permissions.set_mode(0o600);
		std::fs::set_permissions(path, permissions)
			.unwrap_or_else(|error| panic!("keypair permissions failed: {error}"));
	}

	#[cfg(not(unix))]
	fn set_private_permissions(_path: &Path) {}

	#[cfg(unix)]
	#[test]
	fn fetch_uses_raw_mode_and_decodes_direct_zlib_json() {
		let directory = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
		let program_id = "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS";
		let value = idl(program_id);
		let source = serde_json::to_vec(&value)
			.unwrap_or_else(|error| panic!("IDL serialization failed: {error}"));
		let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
		encoder
			.write_all(&source)
			.unwrap_or_else(|error| panic!("compression failed: {error}"));
		let compressed = encoder
			.finish()
			.unwrap_or_else(|error| panic!("compression finish failed: {error}"));
		let hex = compressed.iter().fold(String::new(), |mut output, byte| {
			write!(output, "{byte:02x}")
				.unwrap_or_else(|error| panic!("hex formatting failed: {error}"));
			output
		});
		let capture = directory.path().join("args.txt");
		let runner = fake_runner(directory.path(), &capture, &format!("heading\n{hex}"));
		let client = ClientOptions {
			npx: runner.display().to_string(),
			cluster: "devnet".to_owned(),
		};

		let fetched =
			fetch_idl(&client, program_id).unwrap_or_else(|error| panic!("fetch failed: {error}"));
		assert_eq!(fetched, value);
		let args = std::fs::read_to_string(capture)
			.unwrap_or_else(|error| panic!("capture read failed: {error}"));
		assert!(args.contains("@solana-program/program-metadata@0.9.0"));
		assert!(args.contains("--raw"));
		assert!(!args.contains("latest"));
	}

	#[cfg(unix)]
	#[test]
	fn publish_forwards_structured_arguments_without_secret_contents() {
		let directory = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
		let capture = directory.path().join("args.txt");
		let runner = fake_runner(directory.path(), &capture, "planned");
		let keypair = SigningKey::from_bytes(&[19; 32]).to_keypair_bytes();
		let authority = directory.path().join("authority.json");
		std::fs::write(
			&authority,
			serde_json::to_vec(keypair.as_slice())
				.unwrap_or_else(|error| panic!("keypair serialization failed: {error}")),
		)
		.unwrap_or_else(|error| panic!("keypair write failed: {error}"));
		set_private_permissions(&authority);
		let program_id = "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS";
		let local = LocalIdl {
			path: PathBuf::from("idl.json"),
			program_id: program_id.to_owned(),
			value: idl(program_id),
		};
		let client = ClientOptions {
			npx: runner.display().to_string(),
			cluster: "devnet".to_owned(),
		};
		let options = PublishOptions {
			local: &local,
			authority: Some(&authority),
			payer: None,
			priority_fee: 42,
			export: false,
			export_authority: None,
			export_encoding: "base64",
		};

		assert!(
			publish_idl(&client, &options)
				.unwrap_or_else(|error| panic!("publish failed: {error}"))
				.is_none()
		);
		let args = std::fs::read_to_string(capture)
			.unwrap_or_else(|error| panic!("capture read failed: {error}"));
		assert!(args.contains("write\nidl\n"));
		assert!(args.contains("--compression\nzlib\n--encoding\nutf8\n--format\njson"));
		assert!(args.contains("--priority-fees\n42"));
		assert!(!args.contains(&authority.display().to_string()));
		assert!(!args.contains(&serde_json::to_string(keypair.as_slice()).unwrap_or_default()));
	}

	#[cfg(unix)]
	#[test]
	fn publication_signs_from_a_stable_private_keypair_copy() {
		let directory = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
		let keypair = SigningKey::from_bytes(&[29; 32]).to_keypair_bytes();
		let authority = directory.path().join("authority.json");
		let captured = directory.path().join("captured.json");
		let serialized = serde_json::to_vec(keypair.as_slice()).unwrap_or_default();
		std::fs::write(&authority, &serialized)
			.unwrap_or_else(|error| panic!("keypair write failed: {error}"));
		set_private_permissions(&authority);
		let body = format!(
			"rm -f '{}'\nprevious=''\nfor argument in \"$@\"; do\n  if [ \"$previous\" = \
			 \"--keypair\" ]; then cp \"$argument\" '{}'; fi\n  \
			 previous=\"$argument\"\ndone\nprintf 'published'",
			authority.display(),
			captured.display(),
		);
		let runner = fake_runner_script(directory.path(), &body);
		let program_id = "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS";
		let local = LocalIdl {
			path: PathBuf::from("idl.json"),
			program_id: program_id.to_owned(),
			value: idl(program_id),
		};
		let client = ClientOptions {
			npx: runner.display().to_string(),
			cluster: "devnet".to_owned(),
		};
		let options = PublishOptions {
			local: &local,
			authority: Some(&authority),
			payer: None,
			priority_fee: 0,
			export: false,
			export_authority: None,
			export_encoding: "base64",
		};

		publish_idl(&client, &options)
			.unwrap_or_else(|error| panic!("publication failed: {error}"));
		assert!(!authority.exists());
		assert_eq!(std::fs::read(captured).unwrap_or_default(), serialized);
	}

	#[cfg(unix)]
	#[test]
	fn publish_supports_separate_payer_and_local_export() {
		let directory = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
		let capture = directory.path().join("args.txt");
		let runner = fake_runner(
			directory.path(),
			&capture,
			"Exporting 1 transaction:\n[Transaction #1]\nAAAA\n",
		);
		let keypair = SigningKey::from_bytes(&[23; 32]).to_keypair_bytes();
		let authority = directory.path().join("authority.json");
		let payer = directory.path().join("payer.json");
		for path in [&authority, &payer] {
			std::fs::write(
				path,
				serde_json::to_vec(keypair.as_slice()).unwrap_or_default(),
			)
			.unwrap_or_else(|error| panic!("keypair write failed: {error}"));
			set_private_permissions(path);
		}
		let program_id = "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS";
		let local = LocalIdl {
			path: PathBuf::from("idl.json"),
			program_id: program_id.to_owned(),
			value: idl(program_id),
		};
		let client = ClientOptions {
			npx: runner.display().to_string(),
			cluster: "devnet".to_owned(),
		};
		let options = PublishOptions {
			local: &local,
			authority: Some(&authority),
			payer: Some(&payer),
			priority_fee: 0,
			export: true,
			export_authority: None,
			export_encoding: "base58",
		};
		let exported = publish_idl(&client, &options)
			.unwrap_or_else(|error| panic!("export failed: {error}"))
			.unwrap_or_default();
		assert!(exported.contains("[Transaction #1]"));
		let args = std::fs::read_to_string(capture)
			.unwrap_or_else(|error| panic!("capture read failed: {error}"));
		assert!(args.contains("--payer"));
		assert!(args.contains("--export\n--export-encoding\nbase58"));
	}

	#[test]
	fn library_publication_rejects_invalid_signer_combinations_before_spawning() {
		let program_id = "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS";
		let local = LocalIdl {
			path: PathBuf::from("idl.json"),
			program_id: program_id.to_owned(),
			value: idl(program_id),
		};
		let client = ClientOptions {
			npx: "a runner that must never start".to_owned(),
			cluster: "devnet".to_owned(),
		};
		let keypair = Path::new("authority.json");
		let multisig = "ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S";
		let cases = [
			PublishOptions {
				local: &local,
				authority: None,
				payer: Some(keypair),
				priority_fee: 0,
				export: false,
				export_authority: None,
				export_encoding: "base64",
			},
			PublishOptions {
				local: &local,
				authority: Some(keypair),
				payer: None,
				priority_fee: 0,
				export: true,
				export_authority: Some(multisig),
				export_encoding: "base64",
			},
			PublishOptions {
				local: &local,
				authority: None,
				payer: None,
				priority_fee: 0,
				export: false,
				export_authority: None,
				export_encoding: "base64",
			},
			PublishOptions {
				local: &local,
				authority: None,
				payer: None,
				priority_fee: 0,
				export: true,
				export_authority: None,
				export_encoding: "base64",
			},
		];

		for options in cases {
			assert!(matches!(
				publish_idl(&client, &options),
				Err(IdlMetadataError::InvalidSignerConfiguration { .. })
			));
		}
	}

	#[test]
	fn library_publication_revalidates_forged_local_idl_values_before_spawning() {
		let client = ClientOptions {
			npx: "a runner that must never start".to_owned(),
			cluster: "devnet".to_owned(),
		};
		let victim = "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS";
		let other = "ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S";
		let local = LocalIdl {
			path: PathBuf::from("forged.json"),
			program_id: victim.to_owned(),
			value: idl(other),
		};
		let options = PublishOptions {
			local: &local,
			authority: None,
			payer: None,
			priority_fee: 0,
			export: true,
			export_authority: Some(other),
			export_encoding: "base64",
		};
		assert!(matches!(
			publish_idl(&client, &options),
			Err(IdlMetadataError::ProgramMismatch { .. })
		));
	}

	#[cfg(unix)]
	#[test]
	fn export_rejects_missing_or_non_utf8_transaction_framing() {
		let directory = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
		let program_id = "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS";
		let local = LocalIdl {
			path: PathBuf::from("idl.json"),
			program_id: program_id.to_owned(),
			value: idl(program_id),
		};
		let options = PublishOptions {
			local: &local,
			authority: None,
			payer: None,
			priority_fee: 0,
			export: true,
			export_authority: Some("ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S"),
			export_encoding: "base64",
		};

		let capture = directory.path().join("args-empty.txt");
		let runner = fake_runner(directory.path(), &capture, "no plan");
		let client = ClientOptions {
			npx: runner.display().to_string(),
			cluster: "devnet".to_owned(),
		};
		assert!(matches!(
			publish_idl(&client, &options),
			Err(IdlMetadataError::MissingExport)
		));

		let runner = fake_runner_script(directory.path(), "printf '\\377'");
		let client = ClientOptions {
			npx: runner.display().to_string(),
			cluster: "devnet".to_owned(),
		};
		assert!(matches!(
			publish_idl(&client, &options),
			Err(IdlMetadataError::NonUtf8)
		));
	}

	#[cfg(unix)]
	#[test]
	fn official_client_failures_are_sanitized_and_bounded() {
		let directory = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
		let runner =
			fake_runner_script(directory.path(), "printf 'bad\\001diagnostic' >&2\nexit 7");
		let client = ClientOptions {
			npx: runner.display().to_string(),
			cluster: "devnet".to_owned(),
		};
		let error = run_official_client(&client, &[]).expect_err("client failure must propagate");
		assert!(error.to_string().contains("baddiagnostic"));
		assert!(!error.to_string().contains('\u{1}'));
	}

	#[test]
	fn missing_official_client_executable_is_reported_without_a_shell_fallback() {
		let directory = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
		let client = ClientOptions {
			npx: directory
				.path()
				.join("missing & untrusted npx")
				.display()
				.to_string(),
			cluster: "devnet".to_owned(),
		};
		assert!(matches!(
			run_official_client(&client, &[OsString::from("; touch never")]),
			Err(IdlMetadataError::RunClient { .. })
		));
		assert!(!directory.path().join("never").exists());
	}

	#[test]
	fn bounded_output_reader_drains_without_retaining_overflow() {
		let output = read_bounded(Cursor::new(b"abcdef"), 4)
			.unwrap_or_else(|error| panic!("bounded read failed: {error}"));
		assert_eq!(output.bytes, b"abcd");
		assert!(output.overflowed);

		let exact = read_bounded(Cursor::new(b"abcd"), 4)
			.unwrap_or_else(|error| panic!("bounded read failed: {error}"));
		assert_eq!(exact.bytes, b"abcd");
		assert!(!exact.overflowed);
	}

	#[cfg(unix)]
	#[test]
	fn official_client_rejects_output_overflow_and_preserves_signal_status() {
		let directory = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
		for (body, stream) in [
			("printf '12345'", "stdout"),
			("printf '12345' >&2", "stderr"),
		] {
			let runner = fake_runner_script(directory.path(), body);
			let client = ClientOptions {
				npx: runner.display().to_string(),
				cluster: "devnet".to_owned(),
			};
			let result = if stream == "stdout" {
				run_official_client_with_limits(&client, &[], 4, 64)
			} else {
				run_official_client_with_limits(&client, &[], 64, 4)
			};
			assert!(matches!(
				result,
				Err(IdlMetadataError::ClientOutputTooLarge { stream: actual, .. }) if actual == stream
			));
		}

		let runner = fake_runner_script(directory.path(), "kill -TERM $$");
		let client = ClientOptions {
			npx: runner.display().to_string(),
			cluster: "devnet".to_owned(),
		};
		assert!(matches!(
			run_official_client(&client, &[]),
			Err(IdlMetadataError::ClientFailed { status: -1, .. })
		));
	}

	#[test]
	fn path_error_helpers_preserve_context() {
		let io = || std::io::Error::other("fixture");
		assert!(matches!(
			read_error(Path::new("read"))(io()),
			IdlMetadataError::Read { path, .. } if path == Path::new("read")
		));
		assert!(matches!(
			write_error(Path::new("write"))(io()),
			IdlMetadataError::Write { path, .. } if path == Path::new("write")
		));
		let json = serde_json::from_str::<Value>("{").expect_err("malformed JSON must fail");
		assert!(matches!(
			json_error(Path::new("json"))(json),
			IdlMetadataError::Json { path, .. } if path == Path::new("json")
		));
	}

	#[test]
	fn safe_path_check_reports_a_missing_required_entry() {
		let directory = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
		assert!(matches!(
			ensure_safe_path(&directory.path().join("missing"), true),
			Err(IdlMetadataError::Read { .. })
		));
	}

	#[test]
	fn raw_metadata_parsing_fails_closed() {
		assert!(matches!(
			extract_raw_hex(b""),
			Err(IdlMetadataError::UnsupportedContent { .. })
		));
		assert!(matches!(
			extract_raw_hex(&[0xff]),
			Err(IdlMetadataError::NonUtf8)
		));
		assert!(extract_raw_hex(b"abc\n").is_err());
		assert!(extract_raw_hex(b"zz\n").is_err());
		assert!(extract_raw_hex(&vec![b'a'; 8 * 1024 * 1024 + 1]).is_err());
	}

	#[cfg(unix)]
	#[test]
	fn fetch_rejects_invalid_compression_json_and_program_identity() {
		let directory = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
		let program_id = "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS";
		for (index, raw) in [
			"00".to_owned(),
			"789cff".to_owned(),
			compressed_hex(b"not json"),
		]
		.into_iter()
		.enumerate()
		{
			let capture = directory.path().join(format!("args-{index}.txt"));
			let runner = fake_runner(directory.path(), &capture, &raw);
			let client = ClientOptions {
				npx: runner.display().to_string(),
				cluster: "devnet".to_owned(),
			};
			assert!(fetch_idl(&client, program_id).is_err());
		}

		let other = idl("ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S");
		let source = serde_json::to_vec(&other).unwrap_or_default();
		let capture = directory.path().join("args-mismatch.txt");
		let runner = fake_runner(directory.path(), &capture, &compressed_hex(&source));
		let client = ClientOptions {
			npx: runner.display().to_string(),
			cluster: "devnet".to_owned(),
		};
		assert!(matches!(
			fetch_idl(&client, program_id),
			Err(IdlMetadataError::ProgramMismatch { .. })
		));

		let oversized = vec![0; 16 * 1024 * 1024 + 1];
		let capture = directory.path().join("args-oversized.txt");
		let runner = fake_runner(directory.path(), &capture, &compressed_hex(&oversized));
		let client = ClientOptions {
			npx: runner.display().to_string(),
			cluster: "devnet".to_owned(),
		};
		let error = fetch_idl(&client, program_id).expect_err("oversized IDL must fail");
		assert!(error.to_string().contains("safety limit"));
	}

	#[test]
	fn atomic_output_rejects_links_and_writes_regular_files() {
		let directory = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
		let output = directory.path().join("nested/output.json");
		write_atomic(&output, b"first")
			.unwrap_or_else(|error| panic!("atomic write failed: {error}"));
		write_atomic(&output, b"second")
			.unwrap_or_else(|error| panic!("atomic replace failed: {error}"));
		assert_eq!(std::fs::read(&output).unwrap_or_default(), b"second");

		#[cfg(unix)]
		{
			use std::os::unix::fs::symlink;

			let target = directory.path().join("target.json");
			std::fs::write(&target, b"target")
				.unwrap_or_else(|error| panic!("target write failed: {error}"));
			let link = directory.path().join("link.json");
			symlink(&target, &link).unwrap_or_else(|error| panic!("symlink failed: {error}"));
			assert!(matches!(
				write_atomic(&link, b"replacement"),
				Err(IdlMetadataError::UnsafePath { .. })
			));
		}
	}

	#[cfg(unix)]
	fn compressed_hex(source: &[u8]) -> String {
		let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
		encoder.write_all(source).unwrap_or_default();
		encoder
			.finish()
			.unwrap_or_default()
			.iter()
			.fold(String::new(), |mut output, byte| {
				write!(output, "{byte:02x}").unwrap_or_default();
				output
			})
	}

	#[cfg(unix)]
	fn fake_runner(directory: &Path, capture: &Path, stdout: &str) -> PathBuf {
		use std::os::unix::fs::PermissionsExt;

		let runner = directory.join("fake-npx.sh");
		let script = format!(
			"#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nprintf '{}'\n",
			capture.display(),
			stdout.replace('\\', "\\\\").replace('\'', "'\\''")
		);
		std::fs::write(&runner, script)
			.unwrap_or_else(|error| panic!("runner write failed: {error}"));
		let mut permissions = std::fs::metadata(&runner)
			.unwrap_or_else(|error| panic!("runner metadata failed: {error}"))
			.permissions();
		permissions.set_mode(0o755);
		std::fs::set_permissions(&runner, permissions)
			.unwrap_or_else(|error| panic!("runner permissions failed: {error}"));

		runner
	}

	#[cfg(unix)]
	fn fake_runner_script(directory: &Path, body: &str) -> PathBuf {
		use std::os::unix::fs::PermissionsExt;

		let runner = directory.join("fake-script.sh");
		std::fs::write(&runner, format!("#!/bin/sh\n{body}\n"))
			.unwrap_or_else(|error| panic!("runner write failed: {error}"));
		let mut permissions = std::fs::metadata(&runner)
			.unwrap_or_else(|error| panic!("runner metadata failed: {error}"))
			.permissions();
		permissions.set_mode(0o755);
		std::fs::set_permissions(&runner, permissions)
			.unwrap_or_else(|error| panic!("runner permissions failed: {error}"));

		runner
	}

	#[test]
	fn transaction_export_preserves_official_multi_transaction_framing() {
		let output = b"Writing metadata account...\nExporting 2 transactions:\n[Transaction #1]\n3mJr7AoUXx2WqdTBLBvMtttP7aB9o8X\n[Transaction #2]\n4SUZkH3cK1vPabJv9RkWmN2CdEfGhL\n";
		let exported = String::from_utf8(output.to_vec())
			.unwrap_or_else(|error| panic!("fixture must be UTF-8: {error}"));
		assert!(exported.contains("[Transaction #1]"));
		assert!(exported.contains("[Transaction #2]"));
	}
}
