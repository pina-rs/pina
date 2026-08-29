//! Deployed-program verification through the official `solana-verify` CLI.

use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;
use std::str::FromStr;

use atomic_write_file::AtomicWriteFile;
use base64::Engine;
use ed25519_dalek::SigningKey;
use solana_address::Address;
use url::Host;
use url::Url;

use crate::project::Project;
use crate::project::ProjectError;
use crate::verifiable::VerifyBuildError;

/// The supported upstream verification tool release.
pub const SOLANA_VERIFY_VERSION: &str = "0.5.1";

const MAINNET_RPC: &str = "https://api.mainnet-beta.solana.com";
const DEVNET_RPC: &str = "https://api.devnet.solana.com";
const TESTNET_RPC: &str = "https://api.testnet.solana.com";
const LOCALNET_RPC: &str = "http://localhost:8899";
const MAX_CAPTURED_OUTPUT: usize = 1_048_576;
const MAX_KEYPAIR_BYTES: usize = 4_096;

/// A validated Solana cluster alias or RPC URL.
#[derive(Clone, PartialEq, Eq)]
pub struct Cluster {
	rpc: String,
	name: String,
	mainnet_risk: bool,
}

impl Cluster {
	/// Return the RPC value passed to `solana-verify`.
	#[must_use]
	pub fn rpc(&self) -> &str {
		&self.rpc
	}

	/// Return a safe display name that excludes URL credentials and queries.
	#[must_use]
	pub fn display_name(&self) -> &str {
		&self.name
	}

	/// Whether this is the canonical mainnet endpoint.
	#[must_use]
	pub const fn requires_mainnet_acknowledgement(&self) -> bool {
		self.mainnet_risk
	}
}

impl fmt::Debug for Cluster {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter
			.debug_struct("Cluster")
			.field("endpoint", &self.name)
			.field("mainnet_risk", &self.mainnet_risk)
			.finish_non_exhaustive()
	}
}

impl FromStr for Cluster {
	type Err = VerifyError;

	fn from_str(value: &str) -> Result<Self, Self::Err> {
		let (rpc, name, mainnet_risk) = match value {
			"mainnet" | "mainnet-beta" => (MAINNET_RPC, "mainnet-beta", true),
			"devnet" => (DEVNET_RPC, "devnet", false),
			"testnet" => (TESTNET_RPC, "testnet", false),
			"localnet" | "localhost" => (LOCALNET_RPC, "localnet", false),
			_ => return parse_custom_rpc(value),
		};

		Ok(Self {
			rpc: rpc.to_owned(),
			name: name.to_owned(),
			mainnet_risk,
		})
	}
}

fn parse_custom_rpc(value: &str) -> Result<Cluster, VerifyError> {
	let url = Url::parse(value).map_err(|_| VerifyError::InvalidCluster)?;
	let loopback = matches!(url.host(), Some(Host::Domain("localhost")))
		|| matches!(url.host(), Some(Host::Ipv4(address)) if address.is_loopback())
		|| matches!(url.host(), Some(Host::Ipv6(address)) if address.is_loopback());

	if !matches!(url.scheme(), "http" | "https")
		|| (url.scheme() != "https" && !loopback)
		|| url.host().is_none()
		|| !url.username().is_empty()
		|| url.password().is_some()
		|| url.query().is_some()
		|| url.fragment().is_some()
		|| !matches!(url.path(), "" | "/")
	{
		return Err(VerifyError::InvalidCluster);
	}

	let host = url.host_str().ok_or(VerifyError::InvalidCluster)?;
	let name = match url.port() {
		Some(port) => format!("{}://{host}:{port}", url.scheme()),
		None => format!("{}://{host}", url.scheme()),
	};
	Ok(Cluster {
		rpc: value.to_owned(),
		name,
		mainnet_risk: !loopback,
	})
}

/// A validated public HTTPS Git repository URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryUrl(String);

impl RepositoryUrl {
	/// Return the URL passed to the verifier.
	#[must_use]
	pub fn as_str(&self) -> &str {
		&self.0
	}
}

impl FromStr for RepositoryUrl {
	type Err = VerifyError;

	fn from_str(value: &str) -> Result<Self, Self::Err> {
		let url = Url::parse(value).map_err(|_| VerifyError::InvalidRepository)?;

		if url.scheme() != "https"
			|| !url.username().is_empty()
			|| url.password().is_some()
			|| url.query().is_some()
			|| url.fragment().is_some()
			|| !is_public_host(url.host())
		{
			return Err(VerifyError::InvalidRepository);
		}

		Ok(Self(value.to_owned()))
	}
}

#[allow(clippy::needless_pass_by_value)]
fn is_public_host(host: Option<Host<&str>>) -> bool {
	match host {
		Some(Host::Domain(domain)) => is_public_domain(domain),
		Some(Host::Ipv4(address)) => {
			!(address.is_private()
				|| address.is_loopback()
				|| address.is_link_local()
				|| address.is_broadcast()
				|| address.is_documentation()
				|| address.is_multicast()
				|| address.is_unspecified())
		}
		Some(Host::Ipv6(address)) => {
			if let Some(mapped) = address.to_ipv4_mapped() {
				return is_public_host(Some(Host::Ipv4(mapped)));
			}
			let segments = address.segments();
			let documentation = segments[0] == 0x2001 && segments[1] == 0x0db8;

			!(address.is_loopback()
				|| address.is_unspecified()
				|| address.is_unique_local()
				|| address.is_unicast_link_local()
				|| address.is_multicast()
				|| documentation)
		}
		None => false,
	}
}

fn is_public_domain(value: &str) -> bool {
	let domain = value.trim_end_matches('.').to_ascii_lowercase();
	let reserved = [
		"localhost",
		"local",
		"internal",
		"test",
		"invalid",
		"example",
	];
	let labels_valid = domain.len() <= 253
		&& domain.split('.').all(|label| {
			let bytes = label.as_bytes();
			!bytes.is_empty()
				&& bytes.len() <= 63
				&& bytes.first().is_some_and(u8::is_ascii_alphanumeric)
				&& bytes.last().is_some_and(u8::is_ascii_alphanumeric)
				&& bytes
					.iter()
					.all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
		});

	labels_valid
		&& domain.contains('.')
		&& !reserved
			.iter()
			.any(|suffix| domain == *suffix || domain.ends_with(&format!(".{suffix}")))
}

/// A full SHA-1 Git revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revision(String);

impl Revision {
	/// Return the validated revision.
	#[must_use]
	pub fn as_str(&self) -> &str {
		&self.0
	}
}

impl FromStr for Revision {
	type Err = VerifyError;

	fn from_str(value: &str) -> Result<Self, Self::Err> {
		if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
			return Err(VerifyError::InvalidRevision);
		}

		Ok(Self(value.to_ascii_lowercase()))
	}
}

/// Encoding used for an exported verification transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportEncoding {
	Base64,
	Base58,
}

impl ExportEncoding {
	fn as_str(self) -> &'static str {
		match self {
			Self::Base64 => "base64",
			Self::Base58 => "base58",
		}
	}
}

/// Result of comparing a local executable with a deployed program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckResult {
	Match { hash: String },
	Mismatch { local: String, deployed: String },
}

/// Inputs for a read-only executable comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOptions {
	pub program_id: String,
	pub cluster: Cluster,
	pub program: Option<PathBuf>,
	pub project_dir: PathBuf,
}

/// Inputs shared by recording and exporting verification metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordOptions {
	pub program_id: String,
	pub cluster: Cluster,
	pub build_record: PathBuf,
	pub authority: Option<PathBuf>,
	pub export_authority: Option<String>,
	pub export_output: Option<PathBuf>,
	pub export_encoding: ExportEncoding,
	pub confirmed: bool,
	pub mainnet_acknowledged: bool,
}

/// Fully validated values shown before an interactive record submission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordReview {
	pub build_record: PathBuf,
	pub program_id: String,
	pub cluster: String,
	pub repository: String,
	pub revision: String,
	pub mount_path: String,
	pub workspace_path: String,
	pub library_name: String,
	pub features: Vec<String>,
	pub default_features: bool,
	pub executable_hash: String,
	pub authority_path: PathBuf,
	pub authority: String,
}

/// An immutable, validated recording plan.
///
/// For direct submissions this owns a private temporary copy of the authority keypair, so the
/// file reviewed by the user cannot be replaced between confirmation and process execution.
pub struct RecordPlan {
	options: RecordOptions,
	program_id: Address,
	context: ProjectContext,
	record_hash: String,
	authority: Address,
	staged_authority: Option<StagedAuthority>,
}

impl RecordPlan {
	/// Return the non-secret values that must be reviewed before submission.
	#[must_use]
	pub fn review(&self) -> RecordReview {
		RecordReview {
			build_record: self.options.build_record.clone(),
			program_id: self.program_id.to_string(),
			cluster: self.options.cluster.display_name().to_owned(),
			repository: self.context.repository.as_str().to_owned(),
			revision: self.context.revision.as_str().to_owned(),
			mount_path: self.context.mount_path.clone(),
			workspace_path: self.context.workspace_path.clone(),
			library_name: self.context.library_name.clone(),
			features: self.context.features.clone(),
			default_features: self.context.default_features,
			executable_hash: self.record_hash.clone(),
			authority_path: self.options.authority.clone().unwrap_or_default(),
			authority: self.authority.to_string(),
		}
	}

	/// Mark this exact prepared plan as interactively confirmed.
	pub const fn confirm(&mut self) {
		self.options.confirmed = true;
	}
}

struct StagedAuthority {
	_directory: tempfile::TempDir,
	path: PathBuf,
}

/// Process result returned by a verification executor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
	pub status: ProcessStatus,
	pub stdout: Vec<u8>,
	pub stderr: Vec<u8>,
}

/// Portable child-process termination information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessStatus {
	Code(i32),
	Signal,
}

impl ProcessStatus {
	fn success(self) -> bool {
		matches!(self, Self::Code(0))
	}
}

impl fmt::Display for ProcessStatus {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Code(code) => write!(formatter, "exit code {code}"),
			Self::Signal => formatter.write_str("signal"),
		}
	}
}

/// Executes the official verification tool.
pub trait VerifyExecutor {
	/// Run `solana-verify` with structured arguments.
	fn run(&self, arguments: &[OsString]) -> Result<ProcessOutput, std::io::Error>;

	/// Run a potentially long command while streaming its output.
	fn run_streaming(&self, arguments: &[OsString]) -> Result<ProcessOutput, std::io::Error> {
		self.run(arguments)
	}
}

/// Production process executor.
#[derive(Debug, Clone)]
pub struct SystemVerifyExecutor {
	executable: OsString,
}

impl Default for SystemVerifyExecutor {
	fn default() -> Self {
		Self {
			executable: OsString::from("solana-verify"),
		}
	}
}

impl SystemVerifyExecutor {
	/// Select an explicit `solana-verify` executable.
	#[must_use]
	pub fn new(executable: OsString) -> Self {
		Self { executable }
	}
}

impl VerifyExecutor for SystemVerifyExecutor {
	fn run(&self, arguments: &[OsString]) -> Result<ProcessOutput, std::io::Error> {
		let mut child = Command::new(&self.executable)
			.args(arguments)
			.stdin(Stdio::null())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.spawn()?;
		let stdout = child
			.stdout
			.take()
			.expect("piped solana-verify stdout must be available");
		let stderr = child
			.stderr
			.take()
			.expect("piped solana-verify stderr must be available");
		let stdout_thread = std::thread::spawn(move || read_bounded(stdout));
		let stderr_thread = std::thread::spawn(move || read_bounded(stderr));
		let status = child.wait()?;
		let stdout = join_reader(stdout_thread)?;
		let stderr = join_reader(stderr_thread)?;

		Ok(ProcessOutput {
			status: process_status(status),
			stdout,
			stderr,
		})
	}

	fn run_streaming(&self, arguments: &[OsString]) -> Result<ProcessOutput, std::io::Error> {
		let mut child = Command::new(&self.executable)
			.args(arguments)
			.stdin(Stdio::null())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.spawn()?;
		let stdout = child
			.stdout
			.take()
			.expect("piped solana-verify stdout must be available");
		let stderr = child
			.stderr
			.take()
			.expect("piped solana-verify stderr must be available");
		let stdout_thread = std::thread::spawn(move || read_streaming(stdout, false));
		let stderr_thread = std::thread::spawn(move || read_streaming(stderr, true));
		let status = child.wait()?;
		let stdout = join_reader(stdout_thread)?;
		let stderr = join_reader(stderr_thread)?;

		Ok(ProcessOutput {
			status: process_status(status),
			stdout,
			stderr,
		})
	}
}

fn read_bounded(mut reader: impl Read) -> Result<Vec<u8>, std::io::Error> {
	let mut captured = Vec::new();
	let mut chunk = [0_u8; 8_192];

	loop {
		let read = reader.read(&mut chunk)?;

		if read == 0 {
			break;
		}

		let remaining = MAX_CAPTURED_OUTPUT.saturating_sub(captured.len());
		captured.extend_from_slice(&chunk[..read.min(remaining)]);
	}

	Ok(captured)
}

fn read_streaming(mut reader: impl Read, stderr: bool) -> Result<Vec<u8>, std::io::Error> {
	if stderr {
		return read_streaming_to(&mut reader, std::io::stderr().lock());
	}

	read_streaming_to(&mut reader, std::io::stdout().lock())
}

fn read_streaming_to(
	mut reader: impl Read,
	mut writer: impl Write,
) -> Result<Vec<u8>, std::io::Error> {
	let mut captured = Vec::new();
	let mut chunk = [0_u8; 8_192];
	let mut write_error = None;

	loop {
		let read = reader.read(&mut chunk)?;

		if read == 0 {
			break;
		}

		if write_error.is_none()
			&& let Err(error) = writer.write_all(&chunk[..read])
		{
			write_error = Some(error);
		}

		append_tail(&mut captured, &chunk[..read]);
	}

	write_error.map_or(Ok(captured), Err)
}

fn append_tail(captured: &mut Vec<u8>, chunk: &[u8]) {
	if chunk.len() >= MAX_CAPTURED_OUTPUT {
		captured.clear();
		captured.extend_from_slice(&chunk[chunk.len() - MAX_CAPTURED_OUTPUT..]);
		return;
	}

	let overflow = captured
		.len()
		.saturating_add(chunk.len())
		.saturating_sub(MAX_CAPTURED_OUTPUT);

	if overflow > 0 {
		captured.drain(..overflow);
	}

	captured.extend_from_slice(chunk);
}

fn join_reader(
	thread: std::thread::JoinHandle<Result<Vec<u8>, std::io::Error>>,
) -> Result<Vec<u8>, std::io::Error> {
	flatten_reader(thread.join())
}

fn flatten_reader(
	result: std::thread::Result<Result<Vec<u8>, std::io::Error>>,
) -> Result<Vec<u8>, std::io::Error> {
	result.map_err(|_| std::io::Error::other("solana-verify output reader failed"))?
}

fn process_status(status: ExitStatus) -> ProcessStatus {
	status
		.code()
		.map_or(ProcessStatus::Signal, ProcessStatus::Code)
}

/// Errors produced by deployed-program verification workflows.
#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
	#[error("invalid Solana program address")]
	InvalidAddress,

	#[error(
		"cluster must be a known alias or an HTTP(S) origin without credentials, path, query, or \
		 fragment"
	)]
	InvalidCluster,

	#[error("repository must be a public HTTPS URL without credentials, query, or fragment")]
	InvalidRepository,

	#[error("revision must be a full 40-character hexadecimal Git SHA")]
	InvalidRevision,

	#[error("could not run `solana-verify`: {0}")]
	MissingTool(std::io::Error),

	#[error("unsupported solana-verify version; expected {SOLANA_VERIFY_VERSION}, found {found}")]
	WrongVersion { found: String },

	#[error("`solana-verify {command}` failed with {status}: {details}")]
	CommandFailed {
		command: &'static str,
		status: ProcessStatus,
		details: String,
	},

	#[error("`solana-verify {command}` returned an invalid executable hash")]
	InvalidHash { command: &'static str },

	#[error(transparent)]
	Project(#[from] ProjectError),

	#[error(transparent)]
	VerifyBuild(#[from] VerifyBuildError),

	#[error("program executable does not exist: {0}")]
	MissingProgram(PathBuf),

	#[error("could not read keypair {path}: {source}")]
	ReadKeypair {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("could not create a private authority copy for solana-verify: {0}")]
	StageKeypair(std::io::Error),

	#[error("keypair at {0} is not a valid 64-byte Solana keypair")]
	InvalidKeypair(PathBuf),

	#[error("--authority is required unless --export provides an authority address")]
	MissingAuthority,

	#[error("--authority cannot be combined with an authority address supplied to --export")]
	ConflictingAuthority,

	#[error("recording verification metadata requires confirmation; pass --yes in automation")]
	ConfirmationRequired,

	#[error("mainnet recording requires --acknowledge-mainnet")]
	MainnetAcknowledgementRequired,

	#[error("--output is required with --export")]
	MissingExportOutput,

	#[error("could not write exported transaction to {path}: {source}")]
	WriteExport {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("keypair path must be a private regular file and may not be a symbolic link: {0}")]
	UnsafeKeypair(PathBuf),

	#[error("export output did not end with a valid {encoding} transaction payload")]
	InvalidExport { encoding: &'static str },

	#[error("verified build record does not contain a public repository and full revision")]
	MissingBuildProvenance,

	#[error("verified build record contains an unsafe source path")]
	UnsafeBuildPath,

	#[error("verified build hash {local} does not match deployed program hash {deployed}")]
	RecordMismatch { local: String, deployed: String },

	#[error(
		"solana-verify {SOLANA_VERIFY_VERSION} repository recording is unsupported on {host}; use \
		 Linux or macOS"
	)]
	UnsupportedRecordHost { host: String },
}

impl VerifyError {
	/// Return the documented process exit code for this failure.
	#[must_use]
	pub const fn exit_code(&self) -> i32 {
		if matches!(self, Self::RecordMismatch { .. }) {
			2
		} else {
			1
		}
	}
}

/// Compare an SBF executable with the deployed program through the official hash commands.
///
/// `solana-verify` removes trailing zero bytes before hashing both executables. Delegating both
/// hashes preserves that official behavior rather than reproducing it independently.
///
/// # Errors
///
/// Returns an error for invalid input, project discovery failure, missing tools, unsupported tool
/// versions, malformed hashes, or unsuccessful child processes.
pub fn check_program(
	executor: &impl VerifyExecutor,
	options: &CheckOptions,
) -> Result<CheckResult, VerifyError> {
	let program_id = parse_address(&options.program_id)?;
	let executable = resolve_executable(options)?;

	check_tool_version(executor)?;
	let local = run_hash(
		executor,
		"get-executable-hash",
		[
			OsString::from("get-executable-hash"),
			executable.into_os_string(),
		],
	)?;
	let deployed = run_hash(
		executor,
		"get-program-hash",
		[
			OsString::from("-u"),
			OsString::from(options.cluster.rpc()),
			OsString::from("get-program-hash"),
			OsString::from(program_id.to_string()),
		],
	)?;

	if local == deployed {
		return Ok(CheckResult::Match { hash: local });
	}

	Ok(CheckResult::Mismatch { local, deployed })
}

/// Record or export repository verification metadata through `solana-verify`.
///
/// # Errors
///
/// Returns an error when inputs or keypairs are invalid, safety acknowledgement is absent, project
/// paths are ambiguous, export cannot be written, or `solana-verify` fails.
pub fn record_program(
	executor: &impl VerifyExecutor,
	options: &RecordOptions,
) -> Result<ProcessOutput, VerifyError> {
	record_program_for_host(executor, options, std::env::consts::OS)
}

fn record_program_for_host(
	executor: &impl VerifyExecutor,
	options: &RecordOptions,
	host: &str,
) -> Result<ProcessOutput, VerifyError> {
	let exporting = options.export_authority.is_some() || options.export_output.is_some();

	if !exporting && !options.confirmed {
		return Err(VerifyError::ConfirmationRequired);
	}

	let plan = prepare_record_for_host(options, host)?;
	execute_record(executor, &plan)
}

/// Validate all local inputs and freeze them into one confirmation and execution plan.
///
/// This performs no network calls and does not invoke `solana-verify`.
///
/// # Errors
///
/// Returns an error when the host, address, build record, provenance, paths, export output, or
/// authority keypair is invalid.
pub fn prepare_record(options: &RecordOptions) -> Result<RecordPlan, VerifyError> {
	prepare_record_for_host(options, std::env::consts::OS)
}

fn prepare_record_for_host(options: &RecordOptions, host: &str) -> Result<RecordPlan, VerifyError> {
	ensure_record_host(host)?;
	let program_id = parse_address(&options.program_id)?;
	let exporting = options.export_authority.is_some() || options.export_output.is_some();

	let record = crate::build::read_verified_build_record(&options.build_record)?;
	let context = project_context(&record)?;
	let (authority, staged_authority) = if exporting {
		let authority = export_authority(options)?;
		(authority, None)
	} else {
		let authority_path = options
			.authority
			.as_deref()
			.ok_or(VerifyError::MissingAuthority)?;
		let keypair = read_keypair(authority_path)?;
		let authority = keypair.address;
		let staged = stage_keypair(&keypair.contents)?;
		(authority, Some(staged))
	};

	Ok(RecordPlan {
		options: options.clone(),
		program_id,
		context,
		record_hash: record.executable_hash().to_owned(),
		authority,
		staged_authority,
	})
}

/// Execute one previously prepared and, when mutating, confirmed record plan.
///
/// # Errors
///
/// Returns an error when confirmation or network safety is absent, hashes differ, the official
/// tool fails, or an exported transaction cannot be validated and written.
pub fn execute_record(
	executor: &impl VerifyExecutor,
	plan: &RecordPlan,
) -> Result<ProcessOutput, VerifyError> {
	let exporting = plan.options.export_authority.is_some() || plan.options.export_output.is_some();

	if exporting {
		return export_record(executor, plan);
	}

	if !plan.options.confirmed {
		return Err(VerifyError::ConfirmationRequired);
	}

	if plan.options.cluster.requires_mainnet_acknowledgement() && !plan.options.mainnet_acknowledged
	{
		return Err(VerifyError::MainnetAcknowledgementRequired);
	}

	let authority_path = &plan
		.staged_authority
		.as_ref()
		.ok_or(VerifyError::MissingAuthority)?
		.path;

	check_tool_version(executor)?;
	let deployed = run_hash(
		executor,
		"get-program-hash",
		[
			OsString::from("-u"),
			OsString::from(plan.options.cluster.rpc()),
			OsString::from("get-program-hash"),
			OsString::from(plan.program_id.to_string()),
		],
	)?;

	if deployed != plan.record_hash {
		return Err(VerifyError::RecordMismatch {
			local: plan.record_hash.clone(),
			deployed,
		});
	}

	let arguments = record_arguments(
		&plan.options,
		&plan.context,
		authority_path,
		&plan.program_id,
	);
	let output = run_streaming_checked(executor, "verify-from-repo", &arguments)?;
	guard_record_transcript(&output.stdout)?;

	Ok(ProcessOutput {
		stdout: Vec::new(),
		stderr: Vec::new(),
		..output
	})
}

/// Validate and resolve every value shown by the interactive recording prompt.
///
/// This performs no network calls and does not invoke `solana-verify`.
///
/// # Errors
///
/// Returns an error when the host, program address, build record, provenance, paths, or authority
/// keypair is invalid.
pub fn review_record(options: &RecordOptions) -> Result<RecordReview, VerifyError> {
	Ok(prepare_record(options)?.review())
}

fn ensure_record_host(host: &str) -> Result<(), VerifyError> {
	if matches!(host, "linux" | "macos") {
		return Ok(());
	}

	Err(VerifyError::UnsupportedRecordHost {
		host: host.to_owned(),
	})
}

/// Submit a mainnet remote-verifier job.
///
/// # Errors
///
/// Returns an error for invalid addresses, unavailable tools, unsupported versions, or a failed
/// remote submission.
pub fn submit_program(
	executor: &impl VerifyExecutor,
	program_id: &str,
	uploader: &str,
) -> Result<ProcessOutput, VerifyError> {
	let program_id = parse_address(program_id)?;
	let uploader = parse_address(uploader)?;

	check_tool_version(executor)?;
	run_checked(
		executor,
		"remote submit-job",
		&[
			OsString::from("-u"),
			OsString::from(MAINNET_RPC),
			OsString::from("remote"),
			OsString::from("submit-job"),
			OsString::from("--program-id"),
			OsString::from(program_id.to_string()),
			OsString::from("--uploader"),
			OsString::from(uploader.to_string()),
		],
	)
}

/// Fetch remote verification status for a program.
///
/// # Errors
///
/// Returns an error for invalid addresses, unavailable tools, unsupported versions, or a failed
/// status command.
pub fn status_program(
	executor: &impl VerifyExecutor,
	program_id: &str,
) -> Result<ProcessOutput, VerifyError> {
	let program_id = parse_address(program_id)?;

	check_tool_version(executor)?;
	run_checked(
		executor,
		"remote get-status",
		&[
			OsString::from("-u"),
			OsString::from(MAINNET_RPC),
			OsString::from("remote"),
			OsString::from("get-status"),
			OsString::from("--program-id"),
			OsString::from(program_id.to_string()),
		],
	)
}

fn parse_address(value: &str) -> Result<Address, VerifyError> {
	Address::from_str(value).map_err(|_| VerifyError::InvalidAddress)
}

fn resolve_executable(options: &CheckOptions) -> Result<PathBuf, VerifyError> {
	let path = if let Some(path) = &options.program {
		path.clone()
	} else {
		let project = Project::discover(&options.project_dir)?;
		project
			.target_dir
			.join("deploy")
			.join(format!("{}.so", project.library_name))
	};

	if !path.is_file() {
		return Err(VerifyError::MissingProgram(path));
	}

	Ok(path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectContext {
	repository: RepositoryUrl,
	revision: Revision,
	mount_path: String,
	workspace_path: String,
	library_name: String,
	features: Vec<String>,
	default_features: bool,
}

fn project_context(
	record: &crate::build::VerifiedBuildRecord,
) -> Result<ProjectContext, VerifyError> {
	let repository = record
		.repository()
		.ok_or(VerifyError::MissingBuildProvenance)?
		.parse()?;
	let revision = record
		.revision()
		.ok_or(VerifyError::MissingBuildProvenance)?
		.parse()?;
	let mount_path = verified_relative_path(record.mount_path())?;
	let workspace_path = verified_relative_path(record.workspace_path())?;

	Ok(ProjectContext {
		repository,
		revision,
		mount_path,
		workspace_path,
		library_name: record.library_name().to_owned(),
		features: record.features().to_vec(),
		default_features: record.default_features(),
	})
}

fn verified_relative_path(value: &str) -> Result<String, VerifyError> {
	let path = Path::new(value);

	if path.is_absolute()
		|| path
			.components()
			.any(|component| matches!(component, std::path::Component::ParentDir))
	{
		return Err(VerifyError::UnsafeBuildPath);
	}

	Ok(value.replace('\\', "/"))
}

struct ValidatedKeypair {
	address: Address,
	contents: Vec<u8>,
}

fn read_keypair(path: &Path) -> Result<ValidatedKeypair, VerifyError> {
	let initial_metadata = map_keypair_io(path, fs::symlink_metadata(path))?;

	if !initial_metadata.file_type().is_file() {
		return Err(VerifyError::UnsafeKeypair(path.to_path_buf()));
	}

	#[cfg(windows)]
	{
		use std::os::windows::fs::MetadataExt;

		const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

		if initial_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
			return Err(VerifyError::UnsafeKeypair(path.to_path_buf()));
		}
	}

	let mut file = map_keypair_io(path, fs::File::open(path))?;
	let opened_metadata = map_keypair_io(path, file.metadata())?;
	validate_keypair_metadata(path, &initial_metadata, &opened_metadata)?;
	let contents = read_keypair_contents(path, &mut file)?;
	let bytes = serde_json::from_slice::<Vec<u8>>(&contents)
		.map_err(|_| VerifyError::InvalidKeypair(path.to_path_buf()))?;
	let keypair: [u8; 64] = bytes
		.try_into()
		.map_err(|_| VerifyError::InvalidKeypair(path.to_path_buf()))?;
	let secret: [u8; 32] = keypair[..32]
		.try_into()
		.map_err(|_| VerifyError::InvalidKeypair(path.to_path_buf()))?;
	let signing_key = SigningKey::from_bytes(&secret);
	let expected_public = signing_key.verifying_key().to_bytes();

	if keypair[32..] != expected_public {
		return Err(VerifyError::InvalidKeypair(path.to_path_buf()));
	}

	Ok(ValidatedKeypair {
		address: Address::new_from_array(expected_public),
		contents,
	})
}

fn validate_keypair_metadata(
	path: &Path,
	initial: &fs::Metadata,
	opened: &fs::Metadata,
) -> Result<(), VerifyError> {
	if !opened.file_type().is_file() {
		return Err(VerifyError::UnsafeKeypair(path.to_path_buf()));
	}

	#[cfg(unix)]
	{
		use std::os::unix::fs::MetadataExt;
		use std::os::unix::fs::PermissionsExt;

		if initial.dev() != opened.dev()
			|| initial.ino() != opened.ino()
			|| opened.permissions().mode() & 0o077 != 0
		{
			return Err(VerifyError::UnsafeKeypair(path.to_path_buf()));
		}
	}
	#[cfg(not(unix))]
	let _ = initial;

	if opened.len() > MAX_KEYPAIR_BYTES as u64 {
		return Err(VerifyError::UnsafeKeypair(path.to_path_buf()));
	}

	Ok(())
}

fn read_keypair_contents(path: &Path, reader: &mut impl Read) -> Result<Vec<u8>, VerifyError> {
	let mut contents = Vec::new();
	let read = reader
		.take((MAX_KEYPAIR_BYTES + 1) as u64)
		.read_to_end(&mut contents);
	map_keypair_io(path, read)?;

	if contents.len() > MAX_KEYPAIR_BYTES {
		return Err(VerifyError::UnsafeKeypair(path.to_path_buf()));
	}

	Ok(contents)
}

fn map_keypair_io<T>(path: &Path, result: Result<T, std::io::Error>) -> Result<T, VerifyError> {
	result.map_err(|source| {
		VerifyError::ReadKeypair {
			path: path.to_path_buf(),
			source,
		}
	})
}

fn stage_keypair(contents: &[u8]) -> Result<StagedAuthority, VerifyError> {
	let directory = tempfile::Builder::new()
		.prefix("pina-verify-authority-")
		.tempdir()
		.map_err(VerifyError::StageKeypair)?;
	let path = directory.path().join("authority.json");
	let mut options = OpenOptions::new();
	options.write(true).create_new(true);
	#[cfg(unix)]
	{
		use std::os::unix::fs::OpenOptionsExt;

		options.mode(0o600);
	}
	let mut file = options.open(&path).map_err(VerifyError::StageKeypair)?;
	file.write_all(contents)
		.and_then(|()| file.sync_all())
		.map_err(VerifyError::StageKeypair)?;

	Ok(StagedAuthority {
		_directory: directory,
		path,
	})
}

fn export_authority(options: &RecordOptions) -> Result<Address, VerifyError> {
	if options.export_output.is_none() {
		return Err(VerifyError::MissingExportOutput);
	}

	match options.export_authority.as_deref() {
		Some(value) if !value.is_empty() => {
			if options.authority.is_some() {
				return Err(VerifyError::ConflictingAuthority);
			}

			parse_address(value)
		}
		_ => {
			let authority_path = options
				.authority
				.as_deref()
				.ok_or(VerifyError::MissingAuthority)?;
			Ok(read_keypair(authority_path)?.address)
		}
	}
}

fn check_tool_version(executor: &impl VerifyExecutor) -> Result<(), VerifyError> {
	let output = executor
		.run(&[OsString::from("--version")])
		.map_err(VerifyError::MissingTool)?;

	if !output.status.success() {
		return Err(command_failed("--version", &output));
	}

	let found = String::from_utf8_lossy(&output.stdout).trim().to_owned();
	let accepted = format!("solana-verify {SOLANA_VERIFY_VERSION}");

	if found != accepted {
		return Err(VerifyError::WrongVersion { found });
	}

	Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn run_hash<const N: usize>(
	executor: &impl VerifyExecutor,
	command: &'static str,
	arguments: [OsString; N],
) -> Result<String, VerifyError> {
	let output = run_checked(executor, command, &arguments)?;
	let hash = String::from_utf8_lossy(&output.stdout)
		.trim()
		.to_ascii_lowercase();

	if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
		return Err(VerifyError::InvalidHash { command });
	}

	Ok(hash)
}

fn run_checked(
	executor: &impl VerifyExecutor,
	command: &'static str,
	arguments: &[OsString],
) -> Result<ProcessOutput, VerifyError> {
	let output = executor.run(arguments).map_err(VerifyError::MissingTool)?;

	if !output.status.success() {
		return Err(command_failed(command, &output));
	}

	Ok(output)
}

fn run_streaming_checked(
	executor: &impl VerifyExecutor,
	command: &'static str,
	arguments: &[OsString],
) -> Result<ProcessOutput, VerifyError> {
	let output = executor
		.run_streaming(arguments)
		.map_err(VerifyError::MissingTool)?;

	if !output.status.success() {
		return Err(command_failed(command, &output));
	}

	Ok(output)
}

fn command_failed(command: &'static str, output: &ProcessOutput) -> VerifyError {
	let details = sanitize_diagnostics(&String::from_utf8_lossy(&output.stderr));
	let details = details.trim().to_owned();
	let details = if details.is_empty() {
		"no diagnostic output".to_owned()
	} else {
		details
	};

	VerifyError::CommandFailed {
		command,
		status: output.status,
		details,
	}
}

fn sanitize_diagnostics(value: &str) -> String {
	let redacted: String = value
		.split_inclusive(char::is_whitespace)
		.map(|token| {
			let bare = token.trim_end_matches(char::is_whitespace);
			let whitespace = &token[bare.len()..];
			let suffix_index = bare
				.trim_end_matches(['.', ',', ':', ';', ')', ']', '}'])
				.len();
			let (candidate, suffix) = bare.split_at(suffix_index);
			let redacted = Url::parse(candidate).ok().and_then(|url| {
				let host = url.host_str()?;
				Some(match url.port() {
					Some(port) => format!("{}://{host}:{port}", url.scheme()),
					None => format!("{}://{host}", url.scheme()),
				})
			});

			match redacted {
				Some(redacted) => format!("{redacted}{suffix}{whitespace}"),
				None => format!("{bare}{whitespace}"),
			}
		})
		.collect();
	let mut escaped = String::with_capacity(redacted.len());

	for character in redacted.chars() {
		if character.is_control() && !matches!(character, '\n' | '\t') {
			escaped.extend(character.escape_default());
		} else {
			escaped.push(character);
		}
	}

	escaped
}

fn record_arguments(
	options: &RecordOptions,
	context: &ProjectContext,
	authority: &Path,
	program_id: &Address,
) -> Vec<OsString> {
	let mut arguments = vec![
		OsString::from("-u"),
		OsString::from(options.cluster.rpc()),
		OsString::from("verify-from-repo"),
		OsString::from(context.repository.as_str()),
		OsString::from("--commit-hash"),
		OsString::from(context.revision.as_str()),
		OsString::from("--program-id"),
		OsString::from(program_id.to_string()),
		OsString::from("--mount-path"),
		OsString::from(&context.mount_path),
		OsString::from("--workspace-path"),
		OsString::from(&context.workspace_path),
		OsString::from("--library-name"),
		OsString::from(&context.library_name),
		OsString::from("--skip-prompt"),
		OsString::from("--keypair"),
		authority.as_os_str().to_owned(),
	];
	append_cargo_arguments(&mut arguments, context);
	arguments
}

fn append_cargo_arguments(arguments: &mut Vec<OsString>, context: &ProjectContext) {
	if context.features.is_empty() && context.default_features {
		return;
	}

	arguments.push(OsString::from("--"));

	if !context.features.is_empty() {
		arguments.push(OsString::from("--features"));
		arguments.push(OsString::from(context.features.join(",")));
	}

	if !context.default_features {
		arguments.push(OsString::from("--no-default-features"));
	}
}

fn guard_record_transcript(stdout: &[u8]) -> Result<(), VerifyError> {
	let transcript = String::from_utf8_lossy(stdout);

	if !transcript.contains("Program hashes do not match") {
		return Ok(());
	}

	let local = transcript_hash(&transcript, "Executable Program Hash from repo:")
		.unwrap_or_else(|| "unknown".to_owned());
	let deployed = transcript_hash(&transcript, "On-chain Program Hash:")
		.unwrap_or_else(|| "unknown".to_owned());

	Err(VerifyError::RecordMismatch { local, deployed })
}

fn transcript_hash(transcript: &str, prefix: &str) -> Option<String> {
	transcript.lines().rev().find_map(|line| {
		let value = line.strip_prefix(prefix)?.trim();

		(value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
			.then(|| value.to_ascii_lowercase())
	})
}

fn export_record(
	executor: &impl VerifyExecutor,
	plan: &RecordPlan,
) -> Result<ProcessOutput, VerifyError> {
	let output_path = plan
		.options
		.export_output
		.as_deref()
		.ok_or(VerifyError::MissingExportOutput)?;

	check_tool_version(executor)?;
	let deployed = run_hash(
		executor,
		"get-program-hash",
		[
			OsString::from("-u"),
			OsString::from(plan.options.cluster.rpc()),
			OsString::from("get-program-hash"),
			OsString::from(plan.program_id.to_string()),
		],
	)?;

	if deployed != plan.record_hash {
		return Err(VerifyError::RecordMismatch {
			local: plan.record_hash.clone(),
			deployed,
		});
	}

	let mut arguments = vec![
		OsString::from("-u"),
		OsString::from(plan.options.cluster.rpc()),
		OsString::from("export-pda-tx"),
		OsString::from("--uploader"),
		OsString::from(plan.authority.to_string()),
		OsString::from("--encoding"),
		OsString::from(plan.options.export_encoding.as_str()),
		OsString::from("--mount-path"),
		OsString::from(&plan.context.mount_path),
		OsString::from("--workspace-path"),
		OsString::from(&plan.context.workspace_path),
		OsString::from("--library-name"),
		OsString::from(&plan.context.library_name),
		OsString::from(plan.context.repository.as_str()),
		OsString::from("--commit-hash"),
		OsString::from(plan.context.revision.as_str()),
		OsString::from("--program-id"),
		OsString::from(plan.program_id.to_string()),
	];
	append_cargo_arguments(&mut arguments, &plan.context);
	let output = run_checked(executor, "export-pda-tx", &arguments)?;
	let (diagnostics, payload) = split_export_output(&output.stdout, plan.options.export_encoding)?;

	write_export(output_path, payload.as_bytes())?;
	Ok(ProcessOutput {
		stdout: diagnostics,
		..output
	})
}

fn split_export_output(
	stdout: &[u8],
	encoding: ExportEncoding,
) -> Result<(Vec<u8>, String), VerifyError> {
	let text = std::str::from_utf8(stdout).map_err(|_| {
		VerifyError::InvalidExport {
			encoding: encoding.as_str(),
		}
	})?;
	let trimmed = text.trim_end_matches(['\r', '\n']);
	let split = trimmed.rfind('\n');
	let (diagnostics, payload) = split.map_or(("", trimmed), |index| {
		(&trimmed[..=index], &trimmed[index + 1..])
	});
	let decoded = match encoding {
		ExportEncoding::Base64 => {
			base64::engine::general_purpose::STANDARD
				.decode(payload)
				.map_err(|_| ())
		}
		ExportEncoding::Base58 => bs58::decode(payload).into_vec().map_err(|_| ()),
	}
	.map_err(|()| {
		VerifyError::InvalidExport {
			encoding: encoding.as_str(),
		}
	})?;

	if decoded.len() < 64 {
		return Err(VerifyError::InvalidExport {
			encoding: encoding.as_str(),
		});
	}

	Ok((diagnostics.as_bytes().to_vec(), payload.to_owned()))
}

fn write_export(path: &Path, contents: &[u8]) -> Result<(), VerifyError> {
	let mut file = AtomicWriteFile::open(path).map_err(|source| {
		VerifyError::WriteExport {
			path: path.to_path_buf(),
			source,
		}
	})?;
	let result = file.write_all(contents).and_then(|()| file.commit());
	map_export_write_result(path, result)
}

fn map_export_write_result(
	path: &Path,
	result: Result<(), std::io::Error>,
) -> Result<(), VerifyError> {
	result.map_err(|source| {
		VerifyError::WriteExport {
			path: path.to_path_buf(),
			source,
		}
	})
}

#[cfg(test)]
mod tests {
	use std::cell::RefCell;
	use std::collections::VecDeque;

	use sha2::Digest;
	use sha2::Sha256;
	use tempfile::TempDir;

	use super::*;

	const PROGRAM_ID: &str = "11111111111111111111111111111111";
	const UPLOADER: &str = "SysvarRent111111111111111111111111111111111";
	const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";

	#[derive(Default)]
	struct FakeExecutor {
		outputs: RefCell<VecDeque<Result<ProcessOutput, std::io::Error>>>,
		calls: RefCell<Vec<Vec<OsString>>>,
	}

	impl FakeExecutor {
		fn with(outputs: impl IntoIterator<Item = ProcessOutput>) -> Self {
			Self {
				outputs: RefCell::new(outputs.into_iter().map(Ok).collect()),
				calls: RefCell::default(),
			}
		}
	}

	impl VerifyExecutor for FakeExecutor {
		fn run(&self, arguments: &[OsString]) -> Result<ProcessOutput, std::io::Error> {
			self.calls.borrow_mut().push(arguments.to_vec());
			self.outputs
				.borrow_mut()
				.pop_front()
				.unwrap_or_else(|| Err(std::io::Error::other("missing fake output")))
		}
	}

	fn output(code: i32, stdout: impl AsRef<[u8]>, stderr: impl AsRef<[u8]>) -> ProcessOutput {
		ProcessOutput {
			status: ProcessStatus::Code(code),
			stdout: stdout.as_ref().to_vec(),
			stderr: stderr.as_ref().to_vec(),
		}
	}

	fn version() -> ProcessOutput {
		output(0, "solana-verify 0.5.1\n", "")
	}

	fn record_hash() -> String {
		Sha256::digest([7_u8; 128]).iter().map(|byte| format!("{byte:02x}")).collect::<String>()
	}

	fn record_program_for_test(
		executor: &impl VerifyExecutor,
		options: &RecordOptions,
	) -> Result<ProcessOutput, VerifyError> {
		record_program_for_host(executor, options, "linux")
	}

	fn prepare_record_for_test(options: &RecordOptions) -> Result<RecordPlan, VerifyError> {
		prepare_record_for_host(options, "linux")
	}

	fn review_record_for_test(options: &RecordOptions) -> Result<RecordReview, VerifyError> {
		Ok(prepare_record_for_test(options)?.review())
	}

	#[test]
	fn validates_cluster_aliases_and_redacts_custom_rpc() {
		assert!(
			Cluster::from_str("mainnet-beta")
				.unwrap()
				.requires_mainnet_acknowledgement()
		);
		assert_eq!(Cluster::from_str("devnet").unwrap().rpc(), DEVNET_RPC);
		assert_eq!(Cluster::from_str("testnet").unwrap().rpc(), TESTNET_RPC);
		assert_eq!(Cluster::from_str("localhost").unwrap().rpc(), LOCALNET_RPC);
		let custom = Cluster::from_str("https://rpc.example.com").unwrap();
		assert_eq!(custom.display_name(), "https://rpc.example.com");
		assert!(custom.requires_mainnet_acknowledgement());
		assert!(!format!("{custom:?}").contains("token"));
		assert!(Cluster::from_str("https://user:pass@example.com/rpc?token=secret").is_err());
		assert!(Cluster::from_str("http://rpc.example.com").is_err());
		assert!(Cluster::from_str("file:///tmp/rpc").is_err());
		assert_eq!(
			Cluster::from_str("http://[::1]:8899")
				.unwrap()
				.display_name(),
			"http://[::1]:8899"
		);
	}

	#[test]
	fn validates_public_repository_and_full_revision() {
		assert!(RepositoryUrl::from_str("https://github.com/pina-rs/pina").is_ok());
		for value in [
			"http://github.com/pina-rs/pina",
			"https://user@github.com/pina-rs/pina",
			"https://localhost/repo",
			"https://127.0.0.1/repo",
			"https://example.com/repo?token=x",
			"https://repo.local/project",
			"https://repo.internal/project",
			"https://repo.test/project",
			"https://[fc00::1]/project",
			"https://[fe80::1]/project",
			"https://[2001:db8::1]/project",
			"https://[::ffff:127.0.0.1]/project",
			"https://[::ffff:10.0.0.1]/project",
		] {
			assert!(RepositoryUrl::from_str(value).is_err(), "accepted {value}");
		}
		for domain in [
			"bad..example.com",
			"-bad.example.com",
			"bad-.example.com",
			"bad_label.example.com",
			"localhost",
		] {
			assert!(!is_public_domain(domain));
		}
		assert!(is_public_domain("github.com."));
		assert_eq!(
			Revision::from_str(&REVISION.to_uppercase())
				.unwrap()
				.as_str(),
			REVISION
		);
		assert!(Revision::from_str("abc123").is_err());
		assert!(is_public_host(Some(Host::Ipv4("8.8.8.8".parse().unwrap()))));
		for address in [
			"10.0.0.1",
			"127.0.0.1",
			"169.254.1.1",
			"255.255.255.255",
			"192.0.2.1",
			"224.0.0.1",
			"0.0.0.0",
		] {
			assert!(!is_public_host(Some(Host::Ipv4(address.parse().unwrap()))));
		}
		assert!(is_public_host(Some(Host::Ipv6(
			"2001:4860:4860::8888".parse().unwrap()
		))));
		assert!(!is_public_host(Some(Host::Ipv6("::".parse().unwrap()))));
		for address in [
			"fc00::1",
			"fe80::1",
			"ff02::1",
			"2001:db8::1",
			"::ffff:10.0.0.1",
		] {
			assert!(!is_public_host(Some(Host::Ipv6(address.parse().unwrap()))));
		}
		assert!(!is_public_host(None));
	}

	#[test]
	fn covers_small_helpers_and_bounded_reader_errors() {
		assert_eq!(ExportEncoding::Base58.as_str(), "base58");
		assert_eq!(ProcessStatus::Code(3).to_string(), "exit code 3");
		assert!(format!("{:?}", SystemVerifyExecutor::default()).contains("solana-verify"));

		let mut tail = vec![1_u8; 12];
		append_tail(&mut tail, &vec![2_u8; MAX_CAPTURED_OUTPUT + 1]);
		assert_eq!(tail.len(), MAX_CAPTURED_OUTPUT);
		assert!(tail.iter().all(|byte| *byte == 2));

		struct ErrorReader;
		impl Read for ErrorReader {
			fn read(&mut self, _buffer: &mut [u8]) -> Result<usize, std::io::Error> {
				Err(std::io::Error::other("read failed"))
			}
		}
		assert!(read_bounded(ErrorReader).is_err());
		let panic_result: std::thread::Result<Result<Vec<u8>, std::io::Error>> =
			Err(Box::new("fixture panic"));
		assert!(flatten_reader(panic_result).is_err());

		let mut arguments = Vec::new();
		append_cargo_arguments(
			&mut arguments,
			&ProjectContext {
				repository: RepositoryUrl::from_str("https://github.com/pina-rs/pina").unwrap(),
				revision: Revision::from_str(REVISION).unwrap(),
				mount_path: ".".to_owned(),
				workspace_path: ".".to_owned(),
				library_name: "fixture".to_owned(),
				features: Vec::new(),
				default_features: true,
			},
		);
		assert!(arguments.is_empty());
	}

	#[test]
	fn compares_official_hashes_and_preserves_paths_with_spaces() {
		let temp = TempDir::new().unwrap();
		let program = temp.path().join("program with spaces.so");
		fs::write(&program, b"binary\0\0").unwrap();
		let hash = "a".repeat(64);
		let executor = FakeExecutor::with([
			version(),
			output(0, format!("{hash}\n"), ""),
			output(0, format!("{hash}\n"), ""),
		]);
		let result = check_program(
			&executor,
			&CheckOptions {
				program_id: PROGRAM_ID.to_owned(),
				cluster: Cluster::from_str("devnet").unwrap(),
				program: Some(program.clone()),
				project_dir: PathBuf::from("unused"),
			},
		)
		.unwrap();

		assert_eq!(result, CheckResult::Match { hash });
		assert_eq!(executor.calls.borrow()[1][1], program.as_os_str());
	}

	#[test]
	fn check_discovers_the_project_executable() {
		let temp = TempDir::new().unwrap();
		fs::create_dir_all(temp.path().join("src")).unwrap();
		fs::create_dir_all(temp.path().join("target/deploy")).unwrap();
		fs::write(
			temp.path().join("Cargo.toml"),
			"[package]\nname = \"fixture-program\"\nversion = \"0.1.0\"\nedition = \
			 \"2024\"\n\n[lib]\ncrate-type = [\"cdylib\"]\n",
		)
		.unwrap();
		fs::write(temp.path().join("src/lib.rs"), "").unwrap();
		fs::write(temp.path().join("target/deploy/fixture_program.so"), b"sbf").unwrap();
		let hash = "a".repeat(64);
		let executor = FakeExecutor::with([
			version(),
			output(0, format!("{hash}\n"), ""),
			output(0, format!("{hash}\n"), ""),
		]);
		let result = check_program(
			&executor,
			&CheckOptions {
				program_id: PROGRAM_ID.to_owned(),
				cluster: Cluster::from_str("devnet").unwrap(),
				program: None,
				project_dir: temp.path().to_path_buf(),
			},
		)
		.unwrap();
		assert!(matches!(result, CheckResult::Match { .. }));
	}

	#[test]
	fn reports_hash_mismatch_without_error() {
		let temp = TempDir::new().unwrap();
		let program = temp.path().join("program.so");
		fs::write(&program, b"binary").unwrap();
		let executor = FakeExecutor::with([
			version(),
			output(0, format!("{}\n", "a".repeat(64)), ""),
			output(0, format!("{}\n", "b".repeat(64)), ""),
		]);
		let result = check_program(
			&executor,
			&CheckOptions {
				program_id: PROGRAM_ID.to_owned(),
				cluster: Cluster::from_str("devnet").unwrap(),
				program: Some(program),
				project_dir: PathBuf::new(),
			},
		)
		.unwrap();

		assert!(matches!(result, CheckResult::Mismatch { .. }));
	}

	#[test]
	fn reports_missing_tool_version_hash_status_and_signal() {
		let missing = FakeExecutor::default();
		let error = check_tool_version(&missing).unwrap_err();
		assert!(matches!(error, VerifyError::MissingTool(_)));

		let wrong = FakeExecutor::with([output(0, "solana-verify 0.6.0", "")]);
		assert!(matches!(
			check_tool_version(&wrong),
			Err(VerifyError::WrongVersion { .. })
		));

		let failed = FakeExecutor::with([output(7, "", "network failed")]);
		assert!(matches!(
			check_tool_version(&failed),
			Err(VerifyError::CommandFailed {
				status: ProcessStatus::Code(7),
				..
			})
		));

		let signaled = command_failed(
			"remote get-status",
			&ProcessOutput {
				status: ProcessStatus::Signal,
				stdout: Vec::new(),
				stderr: Vec::new(),
			},
		);
		assert!(signaled.to_string().contains("signal"));
		assert!(signaled.to_string().contains("no diagnostic output"));

		let malformed = FakeExecutor::with([version(), output(0, "not-a-hash", "")]);
		let temp = TempDir::new().unwrap();
		let program = temp.path().join("program.so");
		fs::write(&program, b"binary").unwrap();
		let result = check_program(
			&malformed,
			&CheckOptions {
				program_id: PROGRAM_ID.to_owned(),
				cluster: Cluster::from_str("devnet").unwrap(),
				program: Some(program),
				project_dir: PathBuf::new(),
			},
		);
		assert!(matches!(result, Err(VerifyError::InvalidHash { .. })));
	}

	#[test]
	fn rejects_invalid_addresses_and_missing_programs_before_execution() {
		let executor = FakeExecutor::default();
		let invalid = check_program(
			&executor,
			&CheckOptions {
				program_id: "invalid".to_owned(),
				cluster: Cluster::from_str("devnet").unwrap(),
				program: Some(PathBuf::from("missing.so")),
				project_dir: PathBuf::new(),
			},
		);
		assert!(matches!(invalid, Err(VerifyError::InvalidAddress)));

		let missing = check_program(
			&executor,
			&CheckOptions {
				program_id: PROGRAM_ID.to_owned(),
				cluster: Cluster::from_str("devnet").unwrap(),
				program: Some(PathBuf::from("missing.so")),
				project_dir: PathBuf::new(),
			},
		);
		assert!(matches!(missing, Err(VerifyError::MissingProgram(_))));
	}

	#[test]
	fn submit_and_status_use_mainnet_and_validate_uploader() {
		let executor = FakeExecutor::with([version(), output(0, "job", "")]);
		let submitted = submit_program(&executor, PROGRAM_ID, UPLOADER).unwrap();
		assert_eq!(submitted.stdout, b"job");
		assert!(executor.calls.borrow()[1].contains(&OsString::from(MAINNET_RPC)));

		let executor = FakeExecutor::with([version(), output(0, "verified\n", "diagnostic\n")]);
		let status = status_program(&executor, PROGRAM_ID).unwrap();
		assert_eq!(status.stdout, b"verified\n");
		assert_eq!(status.stderr, b"diagnostic\n");
		assert!(submit_program(&FakeExecutor::default(), PROGRAM_ID, "invalid").is_err());
	}

	#[test]
	fn export_writes_only_validated_payload_and_preserves_diagnostics() {
		let temp = TempDir::new().unwrap();
		let build_record = create_build_record(&temp);
		let output_path = temp.path().join("transaction with spaces.txt");
		let payload = base64::engine::general_purpose::STANDARD.encode([7_u8; 128]);
		let upstream_output = format!("Cloning repository\nBuilding program\n{payload}\n");
		let executor = FakeExecutor::with([
			version(),
			output(0, format!("{}\n", record_hash()), ""),
			output(0, &upstream_output, "diagnostic\n"),
		]);
		let result = record_program_for_test(
			&executor,
			&RecordOptions {
				program_id: PROGRAM_ID.to_owned(),
				cluster: Cluster::from_str("devnet").unwrap(),
				build_record,
				authority: None,
				export_authority: Some(UPLOADER.to_owned()),
				export_output: Some(output_path.clone()),
				export_encoding: ExportEncoding::Base64,
				confirmed: false,
				mainnet_acknowledged: false,
			},
		)
		.unwrap();

		assert_eq!(result.stdout, b"Cloning repository\nBuilding program\n");
		assert_eq!(fs::read(output_path).unwrap(), payload.as_bytes());
		let arguments = &executor.calls.borrow()[2];
		assert!(arguments.contains(&OsString::from("base64")));
	}

	fn create_build_record(temp: &TempDir) -> PathBuf {
		create_build_record_with(temp, ".", "workspace", &["bpf-entrypoint"], true)
	}

	fn create_build_record_with(
		temp: &TempDir,
		mount_path: &str,
		workspace_path: &str,
		features: &[&str],
		default_features: bool,
	) -> PathBuf {
		let bytes = vec![7_u8; 128];
		let hash = Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect::<String>();
		let record = temp.path().join(format!("verify_fixture-{hash}.json"));
		let artifact = record.with_extension("so");
		let json = serde_json::json!({
			"schemaVersion": 1,
			"packageName": "verify_fixture",
			"libraryName": "verify_fixture",
			"executableHash": hash,
			"solanaVerifyVersion": "0.5.1",
			"build": {
				"mountPath": mount_path,
				"workspacePath": workspace_path,
				"programPath": "workspace/programs/verify fixture",
				"libraryName": "verify_fixture",
				"features": features,
				"defaultFeatures": default_features,
				"cargoLockSha256": "a".repeat(64),
			},
			"source": {
				"repository": "https://github.com/pina-rs/pina",
				"revision": REVISION,
				"dirty": false,
			},
			"diagnostics": [],
		});
		fs::write(&artifact, bytes).unwrap();
		fs::write(&record, serde_json::to_vec_pretty(&json).unwrap()).unwrap();
		record
	}

	fn create_keypair(temp: &TempDir, name: &str) -> (PathBuf, Address) {
		let path = temp.path().join(name);
		let signing_key = SigningKey::from_bytes(&[3_u8; 32]);
		let public = signing_key.verifying_key().to_bytes();
		let bytes = [signing_key.to_bytes().as_slice(), public.as_slice()].concat();
		fs::write(&path, serde_json::to_vec(&bytes).unwrap()).unwrap();
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;

			fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
		}
		(path, Address::new_from_array(public))
	}

	#[test]
	fn record_requires_safety_acknowledgements_and_valid_keypairs() {
		let temp = TempDir::new().unwrap();
		let build_record = create_build_record(&temp);
		let base = RecordOptions {
			program_id: PROGRAM_ID.to_owned(),
			cluster: Cluster::from_str("mainnet-beta").unwrap(),
			build_record,
			authority: None,
			export_authority: None,
			export_output: None,
			export_encoding: ExportEncoding::Base58,
			confirmed: false,
			mainnet_acknowledged: false,
		};
		let executor = FakeExecutor::with([version()]);
		assert!(matches!(
			record_program_for_test(&executor, &base),
			Err(VerifyError::ConfirmationRequired)
		));

		let mut mainnet = base.clone();
		mainnet.confirmed = true;
		let (authority, _) = create_keypair(&temp, "authority.json");
		mainnet.authority = Some(authority);
		let executor = FakeExecutor::with([version()]);
		assert!(matches!(
			record_program_for_test(&executor, &mainnet),
			Err(VerifyError::MainnetAcknowledgementRequired)
		));

		let invalid_keypair = temp.path().join("invalid.json");
		fs::write(&invalid_keypair, "[1, 2, 3]").unwrap();
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;

			fs::set_permissions(&invalid_keypair, fs::Permissions::from_mode(0o600)).unwrap();
		}
		mainnet.authority = Some(invalid_keypair);
		mainnet.mainnet_acknowledged = true;
		let executor = FakeExecutor::with([version()]);
		assert!(matches!(
			record_program_for_test(&executor, &mainnet),
			Err(VerifyError::InvalidKeypair(_))
		));
	}

	#[test]
	fn export_requires_output_and_authority() {
		let temp = TempDir::new().unwrap();
		let build_record = create_build_record(&temp);
		let mut options = RecordOptions {
			program_id: PROGRAM_ID.to_owned(),
			cluster: Cluster::from_str("devnet").unwrap(),
			build_record,
			authority: None,
			export_authority: Some(String::new()),
			export_output: None,
			export_encoding: ExportEncoding::Base58,
			confirmed: false,
			mainnet_acknowledged: false,
		};
		let executor = FakeExecutor::with([version()]);
		assert!(matches!(
			record_program_for_test(&executor, &options),
			Err(VerifyError::MissingExportOutput)
		));

		options.export_output = Some(temp.path().join("tx.txt"));
		let executor = FakeExecutor::with([version()]);
		assert!(matches!(
			record_program_for_test(&executor, &options),
			Err(VerifyError::MissingAuthority)
		));

		let (authority, _) = create_keypair(&temp, "conflicting.json");
		options.authority = Some(authority);
		options.export_authority = Some(UPLOADER.to_owned());
		assert!(matches!(
			prepare_record_for_test(&options),
			Err(VerifyError::ConflictingAuthority)
		));

		options.export_authority = Some(String::new());
		let payload = bs58::encode([8_u8; 128]).into_string();
		let executor = FakeExecutor::with([
			version(),
			output(0, format!("{}\n", record_hash()), ""),
			output(0, format!("{payload}\n"), ""),
		]);
		record_program_for_test(&executor, &options).unwrap();
	}

	#[test]
	fn record_uses_exact_build_record_provenance_and_features() {
		let temp = TempDir::new().unwrap();
		let build_record = create_build_record_with(
			&temp,
			".",
			"crates/workspace with spaces",
			&["bpf-entrypoint", "logs"],
			false,
		);
		let (authority, _) = create_keypair(&temp, "authority with spaces.json");
		let hash = record_hash();
		let executor = FakeExecutor::with([
			version(),
			output(0, format!("{hash}\n"), ""),
			output(
				0,
				format!(
					"Program hash matches ✅\nExecutable Program Hash from repo: {hash}\nOn-chain \
					 Program Hash: {hash}\nProgram uploaded successfully.\n"
				),
				"",
			),
		]);
		let result = record_program_for_test(
			&executor,
			&RecordOptions {
				program_id: PROGRAM_ID.to_owned(),
				cluster: Cluster::from_str("devnet").unwrap(),
				build_record,
				authority: Some(authority.clone()),
				export_authority: None,
				export_output: None,
				export_encoding: ExportEncoding::Base58,
				confirmed: true,
				mainnet_acknowledged: false,
			},
		)
		.unwrap();

		assert!(result.stdout.is_empty());
		let arguments = &executor.calls.borrow()[2];
		assert!(arguments.contains(&OsString::from("https://github.com/pina-rs/pina")));
		assert!(arguments.contains(&OsString::from(REVISION)));
		assert!(arguments.contains(&OsString::from("crates/workspace with spaces")));
		assert!(arguments.contains(&OsString::from("bpf-entrypoint,logs")));
		assert!(arguments.contains(&OsString::from("--no-default-features")));
		let keypair_index = arguments
			.iter()
			.position(|argument| argument == "--keypair")
			.unwrap();
		assert_ne!(arguments[keypair_index + 1], authority.into_os_string());
		assert!(
			arguments[keypair_index + 1]
				.to_string_lossy()
				.contains("pina-verify-authority-")
		);
	}

	#[test]
	fn prepared_record_uses_immutable_authority_and_provenance() {
		let temp = TempDir::new().unwrap();
		let build_record = create_build_record(&temp);
		let (authority, expected_address) = create_keypair(&temp, "authority.json");
		let options = RecordOptions {
			program_id: PROGRAM_ID.to_owned(),
			cluster: Cluster::from_str("devnet").unwrap(),
			build_record: build_record.clone(),
			authority: Some(authority.clone()),
			export_authority: None,
			export_output: None,
			export_encoding: ExportEncoding::Base58,
			confirmed: false,
			mainnet_acknowledged: false,
		};
		let mut plan = prepare_record_for_test(&options).unwrap();
		let review = plan.review();
		assert_eq!(review.authority, expected_address.to_string());
		assert_eq!(review.build_record, build_record);
		assert_eq!(review.features, ["bpf-entrypoint"]);
		assert!(review.default_features);
		assert_eq!(
			review_record_for_test(&options).unwrap().authority,
			expected_address.to_string()
		);

		fs::write(&authority, "[]").unwrap();
		fs::write(&build_record, "{}").unwrap();
		plan.confirm();
		let hash = record_hash();
		let executor = FakeExecutor::with([
			version(),
			output(0, format!("{hash}\n"), ""),
			output(0, "Program uploaded successfully.\n", ""),
		]);
		execute_record(&executor, &plan).unwrap();

		let arguments = &executor.calls.borrow()[2];
		assert!(arguments.contains(&OsString::from(REVISION)));
		let keypair_index = arguments
			.iter()
			.position(|argument| argument == "--keypair")
			.unwrap();
		assert_ne!(arguments[keypair_index + 1], authority.into_os_string());
	}

	#[test]
	fn prepared_record_still_requires_confirmation() {
		let temp = TempDir::new().unwrap();
		let (authority, _) = create_keypair(&temp, "authority.json");
		let plan = prepare_record_for_test(&RecordOptions {
			program_id: PROGRAM_ID.to_owned(),
			cluster: Cluster::from_str("devnet").unwrap(),
			build_record: create_build_record(&temp),
			authority: Some(authority),
			export_authority: None,
			export_output: None,
			export_encoding: ExportEncoding::Base58,
			confirmed: false,
			mainnet_acknowledged: false,
		})
		.unwrap();
		assert!(matches!(
			execute_record(&FakeExecutor::default(), &plan),
			Err(VerifyError::ConfirmationRequired)
		));
	}

	#[test]
	fn keypair_and_build_paths_fail_closed() {
		let temp = TempDir::new().unwrap();
		assert!(matches!(
			read_keypair(&temp.path().join("missing")),
			Err(VerifyError::ReadKeypair { .. })
		));

		struct ErrorReader;

		impl Read for ErrorReader {
			fn read(&mut self, _buffer: &mut [u8]) -> Result<usize, std::io::Error> {
				Err(std::io::Error::other("fixture read failure"))
			}
		}

		let fixture_path = temp.path().join("fixture.json");
		assert!(matches!(
			read_keypair_contents(&fixture_path, &mut ErrorReader),
			Err(VerifyError::ReadKeypair { .. })
		));
		assert!(matches!(
			read_keypair_contents(
				&fixture_path,
				&mut std::io::Cursor::new(vec![0_u8; MAX_KEYPAIR_BYTES + 1])
			),
			Err(VerifyError::UnsafeKeypair(_))
		));
		assert!(matches!(
			read_keypair(temp.path()),
			Err(VerifyError::UnsafeKeypair(_))
		));

		let too_large = temp.path().join("large.json");
		fs::write(&too_large, vec![0_u8; 4_097]).unwrap();
		#[cfg(unix)]
		fs::set_permissions(&too_large, {
			use std::os::unix::fs::PermissionsExt;
			fs::Permissions::from_mode(0o600)
		})
		.unwrap();
		assert!(matches!(
			read_keypair(&too_large),
			Err(VerifyError::UnsafeKeypair(_))
		));

		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;

			let public = temp.path().join("public.json");
			fs::write(&public, "[]").unwrap();
			fs::set_permissions(&public, fs::Permissions::from_mode(0o644)).unwrap();
			assert!(matches!(
				read_keypair(&public),
				Err(VerifyError::UnsafeKeypair(_))
			));

			let replacement = temp.path().join("replacement.json");
			fs::write(&replacement, "[]").unwrap();
			fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
			assert!(matches!(
				validate_keypair_metadata(
					&public,
					&fs::metadata(&public).unwrap(),
					&fs::metadata(&replacement).unwrap(),
				),
				Err(VerifyError::UnsafeKeypair(_))
			));
			assert!(matches!(
				validate_keypair_metadata(
					&public,
					&fs::metadata(&public).unwrap(),
					&fs::metadata(temp.path()).unwrap(),
				),
				Err(VerifyError::UnsafeKeypair(_))
			));
		}

		let bad_public = temp.path().join("bad-public.json");
		fs::write(&bad_public, serde_json::to_vec(&vec![1_u8; 64]).unwrap()).unwrap();
		#[cfg(unix)]
		fs::set_permissions(&bad_public, {
			use std::os::unix::fs::PermissionsExt;
			fs::Permissions::from_mode(0o600)
		})
		.unwrap();
		assert!(matches!(
			read_keypair(&bad_public),
			Err(VerifyError::InvalidKeypair(_))
		));

		assert!(verified_relative_path("../program").is_err());
		assert_eq!(
			verified_relative_path("programs\\fixture").unwrap(),
			"programs/fixture"
		);
	}

	#[test]
	fn export_payload_validation_covers_both_encodings_and_write_errors() {
		let base58 = bs58::encode([5_u8; 64]).into_string();
		assert_eq!(
			split_export_output(base58.as_bytes(), ExportEncoding::Base58)
				.unwrap()
				.1,
			base58
		);
		for (bytes, encoding) in [
			(vec![0xff], ExportEncoding::Base64),
			(b"not-base64".to_vec(), ExportEncoding::Base64),
			(
				bs58::encode([1_u8; 2]).into_string().into_bytes(),
				ExportEncoding::Base58,
			),
		] {
			assert!(split_export_output(&bytes, encoding).is_err());
		}
		let temp = TempDir::new().unwrap();
		assert!(write_export(&temp.path().join("missing/tx"), b"payload").is_err());
		assert!(
			map_export_write_result(
				&temp.path().join("tx"),
				Err(std::io::Error::other("commit failed"))
			)
			.is_err()
		);
	}

	#[test]
	fn checked_process_helpers_reject_nonzero_and_streaming_failures() {
		let failed = FakeExecutor::with([output(4, "", "failed")]);
		assert!(matches!(
			run_checked(&failed, "status", &[]),
			Err(VerifyError::CommandFailed { .. })
		));
		let failed = FakeExecutor::with([output(5, "", "failed")]);
		assert!(matches!(
			run_streaming_checked(&failed, "record", &[]),
			Err(VerifyError::CommandFailed { .. })
		));
	}

	#[test]
	fn record_rejects_preflight_and_upstream_mismatches_with_exit_two() {
		let temp = TempDir::new().unwrap();
		let build_record = create_build_record(&temp);
		let (authority, _) = create_keypair(&temp, "authority.json");
		let options = RecordOptions {
			program_id: PROGRAM_ID.to_owned(),
			cluster: Cluster::from_str("devnet").unwrap(),
			build_record,
			authority: Some(authority),
			export_authority: None,
			export_output: None,
			export_encoding: ExportEncoding::Base58,
			confirmed: true,
			mainnet_acknowledged: false,
		};
		let executor =
			FakeExecutor::with([version(), output(0, format!("{}\n", "b".repeat(64)), "")]);
		let error = record_program_for_test(&executor, &options).unwrap_err();
		assert_eq!(error.exit_code(), 2);
		assert!(matches!(error, VerifyError::RecordMismatch { .. }));
		assert_eq!(executor.calls.borrow().len(), 2);

		let hash = record_hash();
		let executor = FakeExecutor::with([
			version(),
			output(0, format!("{hash}\n"), ""),
			output(
				0,
				format!(
					"Program hashes do not match ❌\nExecutable Program Hash from repo: \
					 {}\nOn-chain Program Hash: {}\n",
					"c".repeat(64),
					"d".repeat(64)
				),
				"",
			),
		]);
		let error = record_program_for_test(&executor, &options).unwrap_err();
		assert!(matches!(error, VerifyError::RecordMismatch { .. }));

		let invalid_hash = FakeExecutor::with([version(), output(0, "invalid", "")]);
		assert!(matches!(
			record_program_for_test(&invalid_hash, &options),
			Err(VerifyError::InvalidHash { .. })
		));
	}

	#[test]
	fn export_preflight_rejects_mismatch_and_never_exports() {
		let temp = TempDir::new().unwrap();
		let executor =
			FakeExecutor::with([version(), output(0, format!("{}\n", "b".repeat(64)), "")]);
		let error = record_program_for_test(
			&executor,
			&RecordOptions {
				program_id: PROGRAM_ID.to_owned(),
				cluster: Cluster::from_str("mainnet-beta").unwrap(),
				build_record: create_build_record(&temp),
				authority: None,
				export_authority: Some(UPLOADER.to_owned()),
				export_output: Some(temp.path().join("tx.txt")),
				export_encoding: ExportEncoding::Base58,
				confirmed: false,
				mainnet_acknowledged: false,
			},
		)
		.unwrap_err();

		assert!(matches!(error, VerifyError::RecordMismatch { .. }));
		assert_eq!(executor.calls.borrow().len(), 2);

		let invalid_hash = FakeExecutor::with([version(), output(0, "invalid", "")]);
		let invalid = record_program_for_test(
			&invalid_hash,
			&RecordOptions {
				program_id: PROGRAM_ID.to_owned(),
				cluster: Cluster::from_str("devnet").unwrap(),
				build_record: create_build_record(&temp),
				authority: None,
				export_authority: Some(UPLOADER.to_owned()),
				export_output: Some(temp.path().join("invalid.tx")),
				export_encoding: ExportEncoding::Base58,
				confirmed: false,
				mainnet_acknowledged: false,
			},
		);
		assert!(matches!(invalid, Err(VerifyError::InvalidHash { .. })));
	}

	#[test]
	fn record_host_and_diagnostic_safety_are_explicit() {
		assert!(ensure_record_host("linux").is_ok());
		assert!(ensure_record_host("macos").is_ok());
		assert!(matches!(
			ensure_record_host("windows"),
			Err(VerifyError::UnsupportedRecordHost { .. })
		));
		assert_eq!(
			sanitize_diagnostics("RPC https://user:secret@example.com/path?token=x, failed\n"),
			"RPC https://example.com, failed\n"
		);
		assert_eq!(
			sanitize_diagnostics("RPC https://user:secret@example.com:8899/path failed"),
			"RPC https://example.com:8899 failed"
		);
		assert_eq!(
			sanitize_diagnostics("failure\u{1b}[31m\r\nnext\titem"),
			"failure\\u{1b}[31m\\r\nnext\titem"
		);

		let temp = TempDir::new().unwrap();
		let options = RecordOptions {
			program_id: PROGRAM_ID.to_owned(),
			cluster: Cluster::from_str("devnet").unwrap(),
			build_record: temp.path().join("missing.json"),
			authority: None,
			export_authority: None,
			export_output: None,
			export_encoding: ExportEncoding::Base64,
			confirmed: true,
			mainnet_acknowledged: false,
		};
		assert!(record_program(&FakeExecutor::default(), &options).is_err());
		assert!(prepare_record(&options).is_err());
		assert!(review_record(&options).is_err());
		assert_eq!(VerifyError::InvalidAddress.exit_code(), 1);
	}

	#[test]
	fn streaming_capture_keeps_tail_and_drains_after_writer_failure() {
		struct BrokenWriter;

		impl Write for BrokenWriter {
			fn write(&mut self, _buffer: &[u8]) -> Result<usize, std::io::Error> {
				Err(std::io::Error::new(
					std::io::ErrorKind::BrokenPipe,
					"closed",
				))
			}

			fn flush(&mut self) -> Result<(), std::io::Error> {
				Ok(())
			}
		}
		BrokenWriter.flush().unwrap();

		let mut input = vec![b'x'; MAX_CAPTURED_OUTPUT + 128];
		input.extend_from_slice(b"Program hashes do not match\n");
		let mut mirrored = Vec::new();
		let captured = read_streaming_to(input.as_slice(), &mut mirrored).unwrap();
		assert_eq!(mirrored.len(), input.len());
		assert!(captured.ends_with(b"Program hashes do not match\n"));
		assert_eq!(captured.len(), MAX_CAPTURED_OUTPUT);

		let error = read_streaming_to(input.as_slice(), BrokenWriter).unwrap_err();
		assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
	}
}
