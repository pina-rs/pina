//! Fetching a foreign program's IDL and importing it as a CPI crate.
//!
//! Import is the supported entry point for calling someone else's program.
//! It resolves an IDL from a file, a URL, or a cluster's canonical program
//! metadata, renders a standalone Pina CPI crate, and records where the IDL
//! came from so a reviewer can tell what a committed crate was built against.
//!
//! The provenance block matters because a CPI crate is a copy of another
//! program's interface: without a recorded digest, a reviewed crate can be
//! regenerated from a different IDL without the change being visible.

use std::ffi::OsStr;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use pina_cpi_renderer::RenderConfig;
use pina_cpi_renderer::RenderMode;
use serde_json::Value;

use crate::error::CodamaError;
use crate::project::GenerationMode;

/// Largest IDL this command will read, in bytes.
///
/// Bounding the input keeps a hostile or accidental URL from exhausting memory
/// before the renderer ever sees the document.
const MAX_IDL_BYTES: u64 = 16 * 1024 * 1024;

/// Pinned Anchor-to-Codama converter, matching the `pina cpi` pipeline.
const NODES_FROM_ANCHOR_PACKAGE: &str = "@codama/nodes-from-anchor@1.5.5";

const ANCHOR_CONVERT_SCRIPT: &str = r#"
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { delimiter, dirname, join } from "node:path";
import { pathToFileURL } from "node:url";

const packageName = "@codama/nodes-from-anchor";
const idlPath = process.argv[1];
const pathRoots = (process.env.PATH ?? "").split(delimiter).filter(Boolean).map(dirname);
const searchRoots = [process.cwd(), ...pathRoots];
let rootNodeFromAnchor;

for (const root of searchRoots) {
	try {
		const require = createRequire(join(root, "package.json"));
		const entry = require.resolve(packageName);
		({ rootNodeFromAnchor } = await import(pathToFileURL(entry)));
		break;
	} catch {}
}

if (!rootNodeFromAnchor) {
	throw new Error(`could not resolve ${packageName}`);
}

const idl = JSON.parse(readFileSync(idlPath, "utf8"));
process.stdout.write(JSON.stringify(rootNodeFromAnchor(idl)));
"#;

/// Where an import's IDL came from.
#[derive(Clone, Debug)]
pub enum ImportSource {
	/// A file on disk.
	File(PathBuf),
	/// An HTTP(S) URL.
	Url(String),
	/// The canonical program metadata published for a cluster.
	Cluster { cluster: String, program_id: String },
}

impl ImportSource {
	/// A short, human-readable description for the provenance block.
	fn describe(&self) -> String {
		match self {
			Self::File(path) => format!("file `{}`", path.display()),
			Self::Url(url) => format!("url `{url}`"),
			Self::Cluster {
				cluster,
				program_id,
			} => format!("canonical program metadata for `{program_id}` on `{cluster}`"),
		}
	}

	/// Resolves the `--idl` / `--url` / `--cluster` triangle into one source.
	///
	/// # Errors
	///
	/// Returns an error when more than one of `--idl` and `--url` is given.
	pub fn select(
		idl: Option<PathBuf>,
		url: Option<String>,
		cluster: &str,
		program_id: &str,
	) -> Result<Self, ImportError> {
		match (idl, url) {
			(Some(path), None) => Ok(Self::File(path)),
			(None, Some(url)) => Ok(Self::Url(url)),
			(None, None) => {
				Ok(Self::Cluster {
					cluster: cluster.to_string(),
					program_id: program_id.to_string(),
				})
			}
			(Some(_), Some(_)) => Err(ImportError::ConflictingSources),
		}
	}
}

/// Options for importing one foreign program.
#[derive(Clone, Debug)]
pub struct ImportOptions {
	pub name: String,
	pub program_id: String,
	pub source: ImportSource,
	pub output: PathBuf,
	pub mode: GenerationMode,
	pub npx: String,
	/// Skip instructions the renderer cannot express instead of failing the
	/// import.
	pub skip_unsupported_instructions: bool,
}

/// Errors raised while importing a foreign IDL.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
	#[error("`{name}` is not a usable crate name: {reason}")]
	InvalidName { name: String, reason: String },

	#[error("`{program_id}` is not a valid program address: {reason}")]
	InvalidProgramId { program_id: String, reason: String },

	#[error("could not read the IDL at {path}: {source}")]
	ReadIdl {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("the IDL exceeds the {MAX_IDL_BYTES}-byte safety limit")]
	IdlTooLarge,

	#[error("the IDL is not valid JSON: {source}")]
	InvalidJson { source: serde_json::Error },

	#[error("could not fetch the IDL: {reason}")]
	Fetch { reason: String },

	#[error("--idl and --url cannot be used together")]
	ConflictingSources,

	#[error(transparent)]
	UnsafeOutput(#[from] CodamaError),

	#[error("CPI crate rendering failed: {source}")]
	Render {
		source: pina_cpi_renderer::RenderError,
	},

	#[error("could not write `{path}`: {source}")]
	WriteFile {
		path: PathBuf,
		source: std::io::Error,
	},
}

/// One imported crate's outcome.
#[derive(Clone, Debug)]
pub struct ImportOutcome {
	/// Directory the crate was written to.
	pub crate_dir: PathBuf,
	/// Hex SHA-256 of the IDL the crate was rendered from.
	pub idl_sha256: String,
	/// Where the IDL came from.
	pub source: String,
	/// Whether the rendered crate differed from what was already on disk.
	pub changed: bool,
}

/// Imports a foreign program's IDL as a standalone CPI crate.
///
/// # Errors
///
/// Returns an error when the name or program ID is unusable, the IDL cannot be
/// fetched or parsed, or the crate cannot be rendered or written.
pub fn import_idl(options: &ImportOptions) -> Result<ImportOutcome, ImportError> {
	validate_name(&options.name)?;
	let program_id = validate_program_id(&options.program_id)?;
	let idl_bytes = fetch_idl(&options.source, &options.npx)?;
	let digest = sha256_hex(&idl_bytes);

	let crate_dir = options.output.join(&options.name);
	let normalized = normalize_to_codama(&idl_bytes, &options.npx)?;
	let root = apply_program_id(&normalized, &program_id)?;

	let config = RenderConfig {
		mode: render_mode(options.mode),
		package_name: Some(options.name.clone()),
		skip_unsupported_instructions: options.skip_unsupported_instructions,
		..RenderConfig::default()
	};

	// The renderer validates every page before writing, so a failure here leaves
	// any existing crate untouched.
	pina_cpi_renderer::render_root_node(&root, &crate_dir, &config)
		.map_err(|source| ImportError::Render { source })?;

	let provenance = Provenance {
		name: &options.name,
		program_id: &program_id,
		source: options.source.describe(),
		sha256: &digest,
	};
	let changed = write_provenance(&crate_dir, &provenance)?;

	Ok(ImportOutcome {
		crate_dir,
		idl_sha256: digest,
		source: options.source.describe(),
		changed,
	})
}

/// Facts recorded in a generated crate's README.
struct Provenance<'a> {
	name: &'a str,
	program_id: &'a str,
	source: String,
	sha256: &'a str,
}

impl Provenance<'_> {
	fn render(&self) -> String {
		format!(
			"# {name}_cpi\n\nGenerated CPI client for `{program_id}`.\n\nThis crate was produced \
			 by `pina import`. Do not edit it by hand: re-run the\nimport instead so the \
			 provenance below stays accurate.\n\n<!-- pina-import-provenance:start -->\n## \
			 Provenance\n\n| Field | Value |\n| --- | --- |\n| Program ID | `{program_id}` |\n| \
			 IDL source | {source} |\n| IDL SHA-256 | `{sha256}` |\n| Generator | \
			 `pina_cpi_renderer` {version} |\n\nA CPI crate is a copy of another program's \
			 interface, so this digest is the\ncontract a reviewer checks: the same digest means \
			 the same bytes were rendered.\n<!-- pina-import-provenance:end -->\n",
			name = self.name,
			program_id = self.program_id,
			source = self.source,
			sha256 = self.sha256,
			version = env!("CARGO_PKG_VERSION"),
		)
	}
}

/// The `RenderMode` a generation mode maps to.
const fn render_mode(mode: GenerationMode) -> RenderMode {
	match mode {
		GenerationMode::Auto => RenderMode::Auto,
		GenerationMode::Create => RenderMode::Create,
		GenerationMode::Update => RenderMode::Update,
		GenerationMode::Overwrite => RenderMode::Overwrite,
	}
}

/// Rejects a name that cannot become a directory and crate name.
fn validate_name(name: &str) -> Result<(), ImportError> {
	let invalid = name.is_empty()
		|| name.starts_with('.')
		|| !name
			.chars()
			.all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));

	if invalid {
		return Err(ImportError::InvalidName {
			name: name.to_string(),
			reason: "use ASCII letters, digits, `-`, or `_`, and do not start with `.`".to_string(),
		});
	}

	Ok(())
}

/// Canonicalises a program address, returning the base58 form.
fn validate_program_id(program_id: &str) -> Result<String, ImportError> {
	use std::str::FromStr;

	solana_address::Address::from_str(program_id)
		.map(|address| address.to_string())
		.map_err(|error| {
			ImportError::InvalidProgramId {
				program_id: program_id.to_string(),
				reason: error.to_string(),
			}
		})
}

/// Reads the IDL bytes from whichever source the caller selected.
fn fetch_idl(source: &ImportSource, npx: &str) -> Result<Vec<u8>, ImportError> {
	match source {
		ImportSource::File(path) => read_bounded(path),
		ImportSource::Url(url) => fetch_url(url),
		ImportSource::Cluster {
			cluster,
			program_id,
		} => fetch_cluster(cluster, program_id, npx),
	}
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, ImportError> {
	let file = std::fs::File::open(path).map_err(|source| {
		ImportError::ReadIdl {
			path: path.to_path_buf(),
			source,
		}
	})?;

	read_bounded_reader(file)
}

fn read_bounded_reader(mut reader: impl Read) -> Result<Vec<u8>, ImportError> {
	let mut bytes = Vec::new();
	reader
		.by_ref()
		.take(MAX_IDL_BYTES + 1)
		.read_to_end(&mut bytes)
		.map_err(|source| {
			ImportError::ReadIdl {
				path: PathBuf::from("<input>"),
				source,
			}
		})?;

	if bytes.len() as u64 > MAX_IDL_BYTES {
		return Err(ImportError::IdlTooLarge);
	}

	Ok(bytes)
}

fn fetch_url(url: &str) -> Result<Vec<u8>, ImportError> {
	let parsed = url::Url::parse(url).map_err(|error| {
		ImportError::Fetch {
			reason: format!("`{url}` is not a valid URL: {error}"),
		}
	})?;

	if !matches!(parsed.scheme(), "http" | "https") {
		return Err(ImportError::Fetch {
			reason: format!(
				"`{}` uses the unsupported scheme `{}`; only http and https are accepted",
				parsed.scheme(),
				parsed.scheme()
			),
		});
	}

	let response = ureq::get(url)
		.config()
		.timeout_global(Some(std::time::Duration::from_secs(30)))
		.build()
		.call()
		.map_err(|error| {
			ImportError::Fetch {
				reason: error.to_string(),
			}
		})?;

	read_bounded_reader(response.into_body().into_reader())
}

fn fetch_cluster(cluster: &str, program_id: &str, npx: &str) -> Result<Vec<u8>, ImportError> {
	let client = crate::idl_metadata::ClientOptions {
		npx: npx.to_string(),
		cluster: cluster.to_string(),
	};
	let value = crate::idl_metadata::fetch_idl(&client, program_id).map_err(|error| {
		ImportError::Fetch {
			reason: error.to_string(),
		}
	})?;

	serde_json::to_vec(&value).map_err(|source| ImportError::InvalidJson { source })
}

/// Builds the read error for the converter's scratch directory.
///
/// The scratch directory is freshly created, so its reads only fail when the
/// environment is broken; the constructor exists to keep that mapping out of
/// the happy path.
fn scratch_read_error(source: std::io::Error) -> ImportError {
	ImportError::ReadIdl {
		path: PathBuf::from("<temp>"),
		source,
	}
}

/// Builds the write error for the converter's scratch copy of the IDL.
fn scratch_write_error(path: PathBuf) -> impl FnOnce(std::io::Error) -> ImportError {
	move |source| ImportError::WriteFile { path, source }
}

/// Normalizes any accepted IDL into a Codama root node.
///
/// A Codama document passes through untouched; anything else is treated as an
/// Anchor IDL and converted with the pinned converter, matching `pina cpi`.
fn normalize_to_codama(bytes: &[u8], npx: &str) -> Result<Value, ImportError> {
	let value: Value =
		serde_json::from_slice(bytes).map_err(|source| ImportError::InvalidJson { source })?;

	if serde_json::from_value::<codama_nodes::RootNode>(value.clone()).is_ok()
		|| value.get("standard").and_then(Value::as_str) == Some("codama")
	{
		return Ok(value);
	}

	// Write the raw IDL to a scratch file for the converter, which reads a path.
	let temp = tempfile::tempdir().map_err(scratch_read_error)?;
	let idl_path = temp.path().join("idl.json");
	std::fs::write(&idl_path, bytes).map_err(scratch_write_error(idl_path.clone()))?;

	convert_anchor(&idl_path, npx)
}

fn convert_anchor(path: &Path, npx: &str) -> Result<Value, ImportError> {
	let mut command = if Path::new(npx).file_stem() == Some(OsStr::new("node")) {
		Command::new(npx)
	} else {
		let mut command = Command::new(npx);
		command.args(["-y", "-p", NODES_FROM_ANCHOR_PACKAGE, "node"]);
		command
	};
	command
		.args(["--input-type=module", "--eval", ANCHOR_CONVERT_SCRIPT])
		.arg(path);

	let output = command.output().map_err(|error| {
		ImportError::Fetch {
			reason: format!("could not run `{npx}` to normalize the Anchor IDL: {error}"),
		}
	})?;

	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr);
		return Err(ImportError::Fetch {
			reason: format!(
				"Anchor IDL conversion failed with status {}: {}",
				output.status.code().unwrap_or(-1),
				stderr.trim()
			),
		});
	}

	serde_json::from_slice(&output.stdout).map_err(|source| ImportError::InvalidJson { source })
}

/// Overrides a converted root's program ID with the address the caller passed.
fn apply_program_id(root: &Value, program_id: &str) -> Result<codama_nodes::RootNode, ImportError> {
	let mut root = root.clone();
	let Some(program) = root.get_mut("program").and_then(Value::as_object_mut) else {
		return Err(ImportError::Fetch {
			reason: "the IDL has no `program` node to bind the program ID to".to_string(),
		});
	};
	program.insert(
		"publicKey".to_string(),
		Value::String(program_id.to_string()),
	);

	serde_json::from_value(root).map_err(|source| ImportError::InvalidJson { source })
}

/// Writes the provenance README into a rendered crate.
///
/// Returns whether the file changed, so a re-import of an unchanged IDL reports
/// a no-op instead of a rewrite.
fn write_provenance(crate_dir: &Path, provenance: &Provenance<'_>) -> Result<bool, ImportError> {
	let readme_path = crate_dir.join("README.md");
	let readme = provenance.render();
	let changed = std::fs::read_to_string(&readme_path).ok().as_deref() != Some(readme.as_str());
	write_if_changed(&readme_path, &readme)?;

	Ok(changed)
}

fn write_if_changed(path: &Path, contents: &str) -> Result<(), ImportError> {
	if std::fs::read_to_string(path).ok().as_deref() == Some(contents) {
		return Ok(());
	}

	std::fs::write(path, contents).map_err(|source| {
		ImportError::WriteFile {
			path: path.to_path_buf(),
			source,
		}
	})
}

/// Hex-encoded SHA-256 of `bytes`.
fn sha256_hex(bytes: &[u8]) -> String {
	use sha2::Digest;

	let digest = sha2::Sha256::digest(bytes);

	digest.iter().fold(String::new(), |mut hex, byte| {
		use std::fmt::Write as _;
		let _ = write!(hex, "{byte:02x}");
		hex
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	fn fixture_path() -> PathBuf {
		Path::new(env!("CARGO_MANIFEST_DIR"))
			.parent()
			.and_then(Path::parent)
			.expect("the CLI manifest lives inside the workspace")
			.join("crates/pina_cpi_renderer/fixtures/switchboard_on_demand.json")
	}

	fn options(output: PathBuf, source: ImportSource) -> ImportOptions {
		ImportOptions {
			name: "switchboard".to_string(),
			program_id: "SBondMDrcV3K4kxZR1HNVT7osZxAHVHgYXL5Ze1oMUv".to_string(),
			source,
			output,
			mode: GenerationMode::Auto,
			npx: "must-not-run".to_string(),
			skip_unsupported_instructions: false,
		}
	}

	#[test]
	fn rejects_names_that_cannot_become_crates() {
		for name in ["", ".hidden", "has space", "slash/ed", "dot.ted"] {
			let error = validate_name(name).expect_err("unsafe names must be rejected");
			assert!(error.to_string().contains("not a usable crate name"));
		}

		for name in ["switchboard", "metaplex-token-metadata", "squads_v4"] {
			assert!(validate_name(name).is_ok(), "`{name}` should be allowed");
		}
	}

	#[test]
	fn canonicalises_and_rejects_program_ids() {
		assert_eq!(
			validate_program_id("SBondMDrcV3K4kxZR1HNVT7osZxAHVHgYXL5Ze1oMUv")
				.expect("valid key rejected"),
			"SBondMDrcV3K4kxZR1HNVT7osZxAHVHgYXL5Ze1oMUv"
		);
		assert!(validate_program_id("not-a-key").is_err());
		assert!(validate_program_id("").is_err());
	}

	#[test]
	fn hashes_the_exact_idl_bytes() {
		// The digest is the provenance contract, so it must be stable and
		// whitespace-sensitive.
		assert_eq!(
			sha256_hex(b""),
			"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
		);
		assert_ne!(sha256_hex(b"{}"), sha256_hex(b"{ }"));
	}

	#[test]
	fn rejects_non_http_url_sources() {
		for url in ["file:///etc/passwd", "ftp://example.com/idl.json"] {
			let error = fetch_url(url).expect_err("non-http schemes must be rejected");
			assert!(error.to_string().contains("only http and https"));
		}
		assert!(fetch_url("not a url").is_err());
	}

	#[test]
	fn imports_a_file_idl_with_provenance() {
		let temp = tempfile::TempDir::new().expect("temp");
		let outcome = import_idl(&options(
			temp.path().to_path_buf(),
			ImportSource::File(fixture_path()),
		))
		.expect("import should succeed");

		assert!(outcome.changed);
		assert_eq!(outcome.idl_sha256.len(), 64);
		assert!(outcome.crate_dir.join("src/generated/mod.rs").is_file());
		assert!(
			outcome
				.crate_dir
				.join("src/generated/accounts/randomness_account_data.rs")
				.is_file()
		);

		let readme = std::fs::read_to_string(outcome.crate_dir.join("README.md")).expect("readme");
		assert!(readme.contains(&outcome.idl_sha256));
		assert!(readme.contains("SBondMDrcV3K4kxZR1HNVT7osZxAHVHgYXL5Ze1oMUv"));
		assert!(readme.contains("pina-import-provenance:start"));
	}

	#[test]
	fn re_importing_an_unchanged_idl_reports_no_change() {
		let temp = tempfile::TempDir::new().expect("temp");
		let options = options(
			temp.path().to_path_buf(),
			ImportSource::File(fixture_path()),
		);

		let first = import_idl(&options).expect("first");
		assert!(first.changed);

		let second = import_idl(&options).expect("second");
		assert!(
			!second.changed,
			"an unchanged IDL must not rewrite the crate"
		);
		assert_eq!(first.idl_sha256, second.idl_sha256);
	}

	#[test]
	fn binds_the_caller_program_id_into_the_crate() {
		let temp = tempfile::TempDir::new().expect("temp");
		let mut options = options(
			temp.path().to_path_buf(),
			ImportSource::File(fixture_path()),
		);
		// A devnet deployment of the same interface, as the issue's lootbox
		// integration distinguishes mainnet from devnet.
		options.program_id = "Aio4gaXjXzJNVLtzwtNVmSqGKpANtXhybbkhtAC94ji2".to_string();

		let outcome = import_idl(&options).expect("import");
		let programs = std::fs::read_to_string(outcome.crate_dir.join("src/generated/programs.rs"))
			.expect("programs");

		assert!(programs.contains("Aio4gaXjXzJNVLtzwtNVmSqGKpANtXhybbkhtAC94ji2"));
		assert!(!programs.contains("SBondMDrcV3K4kxZR1HNVT7osZxAHVHgYXL5Ze1oMUv"));
	}

	#[test]
	fn reports_render_failures_from_the_renderer() {
		let temp = tempfile::TempDir::new().expect("temp");
		// The output path is a file, so the crate directory cannot be created.
		let blocker = temp.path().join("blocker");
		std::fs::write(&blocker, b"occupied").expect("write blocker");

		let error = import_idl(&options(blocker, ImportSource::File(fixture_path())))
			.err()
			.expect("an unusable output path must fail the render");
		assert!(matches!(error, ImportError::Render { .. }));
	}
}

#[cfg(test)]
mod coverage {
	use super::*;

	#[test]
	fn describes_every_source_shape() {
		assert!(
			ImportSource::File(PathBuf::from("./x.json"))
				.describe()
				.contains("file")
		);
		assert!(
			ImportSource::Url("https://example.com/x.json".to_string())
				.describe()
				.contains("url")
		);
		assert!(
			ImportSource::Cluster {
				cluster: "mainnet-beta".to_string(),
				program_id: "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf".to_string(),
			}
			.describe()
			.contains("canonical program metadata")
		);
	}

	#[test]
	fn converts_every_generation_mode() {
		use pina_cpi_renderer::RenderMode;

		assert!(matches!(
			render_mode(GenerationMode::Auto),
			RenderMode::Auto
		));
		assert!(matches!(
			render_mode(GenerationMode::Create),
			RenderMode::Create
		));
		assert!(matches!(
			render_mode(GenerationMode::Update),
			RenderMode::Update
		));
		assert!(matches!(
			render_mode(GenerationMode::Overwrite),
			RenderMode::Overwrite
		));
	}

	#[test]
	fn rejects_readers_over_the_size_limit() {
		let oversized = std::io::Cursor::new(vec![0u8; MAX_IDL_BYTES as usize + 1]);
		let error = read_bounded_reader(oversized).expect_err("oversize input must be rejected");
		assert!(error.to_string().contains("safety limit"));

		let exact = std::io::Cursor::new(vec![0u8; MAX_IDL_BYTES as usize]);
		assert!(read_bounded_reader(exact).is_ok());
	}

	#[test]
	fn reports_unreadable_idl_files() {
		let error = read_bounded(Path::new("./definitely-missing.json"))
			.expect_err("missing files must be rejected");
		assert!(error.to_string().contains("could not read the IDL"));
	}

	#[test]
	fn passes_codama_roots_through_untouched() {
		let root = serde_json::json!({
			"kind": "rootNode",
			"standard": "codama",
			"version": "1.0.0",
			"program": {
				"kind": "programNode",
				"name": "counter",
				"publicKey": "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS",
				"version": "0.0.0"
			}
		});
		let bytes = serde_json::to_vec(&root).expect("json");
		let normalized = normalize_to_codama(&bytes, "must-not-run").expect("passthrough");
		assert!(normalized.get("program").is_some());
	}

	#[test]
	fn applies_the_caller_program_id_over_the_idl_value() {
		let root = serde_json::json!({
			"kind": "rootNode",
			"standard": "codama",
			"version": "1.0.0",
			"program": {
				"kind": "programNode",
				"name": "counter",
				"publicKey": "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS",
				"version": "0.0.0"
			}
		});
		let applied = apply_program_id(&root, "11111111111111111111111111111111").expect("apply");
		let key = applied.program.public_key.as_str();
		assert_eq!(key, "11111111111111111111111111111111");

		let missing = serde_json::json!({ "kind": "rootNode" });
		let error = apply_program_id(&missing, "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS")
			.expect_err("a root without a program must be rejected");
		assert!(error.to_string().contains("no `program` node"));
	}

	#[test]
	fn rejects_clusters_that_are_not_rpc_targets() {
		let error = fetch_cluster(
			"not-an-rpc",
			"GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS",
			"must-not-run",
		)
		.expect_err("invalid clusters must be rejected");
		assert!(error.to_string().contains("could not fetch") || error.to_string().contains("RPC"));
	}

	#[test]
	fn fetch_errors_name_connection_failures() {
		// Port 1 on loopback is closed, so the request fails without network.
		let error = fetch_url("http://127.0.0.1:1/x.json").expect_err("closed ports must fail");
		assert!(error.to_string().contains("could not fetch"));
	}

	#[test]
	fn routes_fetching_through_the_selected_source() {
		let error = fetch_idl(
			&ImportSource::Url("http://127.0.0.1:1/x.json".to_string()),
			"must-not-run",
		)
		.expect_err("closed ports must fail");
		assert!(error.to_string().contains("could not fetch"));

		let error = fetch_idl(
			&ImportSource::Cluster {
				cluster: "localnet".to_string(),
				program_id: "not-a-key".to_string(),
			},
			"must-not-run",
		)
		.expect_err("unusable program ids must fail before any subprocess runs");
		assert!(error.to_string().contains("could not fetch"));
	}

	#[test]
	fn reads_idl_bodies_from_http_sources() {
		let body = br#"{"kind":"rootNode","standard":"codama"}"#;
		let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
		let port = listener.local_addr().expect("addr").port();
		let server = std::thread::spawn(move || {
			let (mut stream, _) = listener.accept().expect("accept");
			let head = format!(
				"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: \
				 {}\r\nconnection: close\r\n\r\n",
				body.len()
			);
			use std::io::Write as _;
			stream.write_all(head.as_bytes()).expect("write head");
			stream.write_all(body).expect("write body");
			let _ = stream.shutdown(std::net::Shutdown::Write);
			let mut sink = [0_u8; 512];
			while matches!(stream.read(&mut sink), Ok(read) if read > 0) {}
		});

		let bytes = fetch_url(&format!("http://127.0.0.1:{port}/idl.json")).expect("fetch");
		server.join().expect("server");
		assert_eq!(bytes, body.to_vec());
	}

	#[test]
	fn reports_io_failures_from_idl_readers() {
		struct Broken;

		impl std::io::Read for Broken {
			fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
				Err(std::io::Error::other("cable unplugged"))
			}
		}

		let error = read_bounded_reader(Broken)
			.err()
			.expect("io failures must surface");
		assert!(error.to_string().contains("could not read the IDL"));
	}

	#[test]
	fn passes_loose_codama_documents_through() {
		// The document carries the Codama standard marker even though it is too
		// loose to parse as a root node, so no converter runs.
		let bytes = br#"{"kind":"rootNode","standard":"codama"}"#;
		let normalized = normalize_to_codama(bytes, "must-not-run").expect("passthrough");
		assert_eq!(
			normalized.get("standard").and_then(Value::as_str),
			Some("codama")
		);
	}

	#[test]
	fn selects_exactly_one_idl_source() {
		let file = ImportSource::select(
			Some(PathBuf::from("./counter.json")),
			None,
			"mainnet-beta",
			"GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS",
		)
		.expect("a file source is enough");
		assert!(matches!(file, ImportSource::File(_)));

		let url = ImportSource::select(
			None,
			Some("https://example.com/counter.json".to_string()),
			"mainnet-beta",
			"GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS",
		)
		.expect("a url source is enough");
		assert!(matches!(url, ImportSource::Url(_)));

		let cluster = ImportSource::select(
			None,
			None,
			"mainnet-beta",
			"GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS",
		)
		.expect("without either, the cluster metadata is the source");
		assert!(matches!(cluster, ImportSource::Cluster { .. }));

		let error = ImportSource::select(
			Some(PathBuf::from("./counter.json")),
			Some("https://example.com/counter.json".to_string()),
			"mainnet-beta",
			"GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS",
		)
		.err()
		.expect("naming two sources must be rejected");
		assert!(error.to_string().contains("cannot be used together"));
	}

	#[test]
	fn reports_provenance_write_failures() {
		let temp = tempfile::TempDir::new().expect("temp");
		// The parent directory does not exist, so the write fails.
		let readme = temp.path().join("missing").join("README.md");
		let error = write_if_changed(&readme, "provenance")
			.err()
			.expect("a failed write must be reported");
		assert!(error.to_string().contains("could not write"));
	}

	#[test]
	fn describes_scratch_errors_with_their_context() {
		let read = scratch_read_error(std::io::Error::other("broken"));
		assert!(read.to_string().contains("<temp>"));

		let write = scratch_write_error(PathBuf::from("/tmp/scratch/idl.json"))(
			std::io::Error::other("full"),
		);
		assert!(write.to_string().contains("idl.json"));
	}

	/// Writes an executable stand-in for `npx` that ignores its arguments and
	/// prints `output`.
	#[cfg(unix)]
	fn fake_executable(dir: &Path, name: &str, output: &str) -> PathBuf {
		fake_script(dir, name, &["printf '", output, "'"].concat())
	}

	/// Writes an executable stand-in for `npx` running an arbitrary shell body.
	#[cfg(unix)]
	fn fake_script(dir: &Path, name: &str, body: &str) -> PathBuf {
		use std::os::unix::fs::PermissionsExt as _;

		let path = dir.join(name);
		std::fs::write(&path, format!("#!/bin/sh\n{}\n", body)).expect("write script");
		std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
			.expect("make executable");
		path
	}

	fn codama_root_json() -> String {
		serde_json::json!({
			"kind": "rootNode",
			"standard": "codama",
			"version": "1.0.0",
			"program": {
				"kind": "programNode",
				"name": "counter",
				"publicKey": "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS",
				"version": "0.0.0"
			}
		})
		.to_string()
	}

	#[cfg(unix)]
	#[test]
	fn converts_anchor_idls_with_the_configured_converter() {
		let anchor_idl = br#"{"name":"counter","version":"0.1.0","instructions":[]}"#;
		let temp = tempfile::TempDir::new().expect("temp");
		// The `node` file stem takes the direct-node invocation path; any other
		// name goes through `npx -y -p <package> node`.
		let npx = fake_executable(temp.path(), "node", &codama_root_json());
		let normalized =
			normalize_to_codama(anchor_idl, &npx.to_string_lossy()).expect("conversion");
		assert!(normalized.get("program").is_some());

		let temp = tempfile::TempDir::new().expect("temp");
		let npx = fake_executable(temp.path(), "fake-npx", &codama_root_json());
		let normalized =
			normalize_to_codama(anchor_idl, &npx.to_string_lossy()).expect("conversion");
		assert!(normalized.get("program").is_some());
	}

	#[cfg(unix)]
	#[test]
	fn reports_anchor_converter_failures() {
		let anchor_idl = br#"{"name":"counter","version":"0.1.0","instructions":[]}"#;
		let temp = tempfile::TempDir::new().expect("temp");
		let npx = fake_script(
			temp.path(),
			"fake-npx",
			"echo 'converter exploded' >&2; exit 3",
		);

		let error = normalize_to_codama(anchor_idl, &npx.to_string_lossy())
			.err()
			.expect("a failing converter must be reported");
		assert!(error.to_string().contains("conversion failed"));
		assert!(error.to_string().contains("converter exploded"));

		// A converter that exits successfully without JSON is a parse failure.
		let npx = fake_script(temp.path(), "fake-npx-quiet", "printf 'not json at all'");
		let error = normalize_to_codama(anchor_idl, &npx.to_string_lossy())
			.err()
			.expect("non-JSON converter output must be reported");
		assert!(error.to_string().contains("not valid JSON"));
	}

	#[test]
	fn reports_missing_converters() {
		let anchor_idl = br#"{"name":"counter","version":"0.1.0","instructions":[]}"#;
		let error = normalize_to_codama(anchor_idl, "./definitely-not-installed")
			.err()
			.expect("a missing converter must be reported");
		assert!(error.to_string().contains("could not run"));
	}

	#[cfg(unix)]
	#[test]
	fn fetches_cluster_idls_through_the_official_client() {
		use std::io::Write as _;

		use flate2::Compression;
		use flate2::write::ZlibEncoder;

		let root = codama_root_json();
		let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
		encoder.write_all(root.as_bytes()).expect("compress");
		let compressed = encoder.finish().expect("compress");
		let hex: String = compressed
			.iter()
			.map(|byte| format!("{byte:02x}"))
			.collect();

		let temp = tempfile::TempDir::new().expect("temp");
		let npx = fake_executable(temp.path(), "fake-npx", &hex);

		let bytes = fetch_cluster(
			"localnet",
			"GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS",
			&npx.to_string_lossy(),
		)
		.expect("the fake client serves a compressed IDL");
		let value = serde_json::from_slice::<Value>(&bytes).expect("json");
		assert_eq!(value.get("kind").and_then(Value::as_str), Some("rootNode"));
	}
}
