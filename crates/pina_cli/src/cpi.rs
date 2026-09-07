//! Standalone CPI crate generation from Codama and Anchor IDLs.

use std::ffi::OsStr;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use codama_nodes::RootNode;
use pina_cpi_renderer::RenderConfig;
use pina_cpi_renderer::RenderMode;
use serde_json::Value;

use crate::codama::validate_render_target;
use crate::error::CodamaError;
use crate::project::GenerationMode;

const MAX_IDL_BYTES: u64 = 16 * 1024 * 1024;
const NODES_FROM_ANCHOR_PACKAGE: &str = "@codama/nodes-from-anchor@1.5.5";
const ANCHOR_CONVERT_SCRIPT: &str = r#"
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { delimiter, dirname, join } from "node:path";
import { pathToFileURL } from "node:url";

const packageName = "@codama/nodes-from-anchor";
const [resolutionMode, idlPath] = process.argv.slice(1);
const pathRoots = (process.env.PATH ?? "").split(delimiter).filter(Boolean).map(dirname);
const searchRoots = resolutionMode === "npx"
	? pathRoots.slice(0, 1)
	: [process.cwd(), ...pathRoots];
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
	throw new Error(`could not resolve ${packageName} from the ${resolutionMode} environment`);
}

if (!idlPath) {
	throw new Error("missing Anchor IDL path");
}

const idl = JSON.parse(readFileSync(idlPath, "utf8"));
process.stdout.write(JSON.stringify(rootNodeFromAnchor(idl)));
"#;

#[derive(Clone, Copy)]
enum ConverterResolution {
	Npx,
	Project,
}

impl ConverterResolution {
	const fn as_str(self) -> &'static str {
		match self {
			Self::Npx => "npx",
			Self::Project => "project",
		}
	}
}

/// Options for generating one standalone Pina CPI crate.
#[derive(Clone, Debug)]
pub struct CpiGenerateOptions {
	pub idl: PathBuf,
	pub output: PathBuf,
	pub mode: GenerationMode,
	pub scaffold: bool,
	pub npx: String,
}

/// Errors produced while normalizing an IDL and rendering a CPI crate.
#[derive(Debug, thiserror::Error)]
pub enum CpiGenerateError {
	#[error("Could not read CPI IDL at {path}: {source}")]
	ReadIdl {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("CPI IDL at {path} exceeds the {MAX_IDL_BYTES}-byte safety limit")]
	IdlTooLarge { path: PathBuf },

	#[error("Invalid JSON in CPI IDL at {path}: {source}")]
	InvalidJson {
		path: PathBuf,
		source: serde_json::Error,
	},

	#[error("Invalid Codama CPI IDL at {path}: {source}")]
	InvalidCodama {
		path: PathBuf,
		source: serde_json::Error,
	},

	#[error("Failed to run Anchor IDL converter `{cmd}`: {source}")]
	RunConverter { cmd: String, source: std::io::Error },

	#[error("Anchor IDL conversion with `{cmd}` failed with status {status}{details}")]
	ConverterFailed {
		cmd: String,
		status: i32,
		details: String,
	},

	#[error("Anchor IDL converter returned an invalid Codama root for {path}: {source}")]
	InvalidConvertedIdl {
		path: PathBuf,
		source: serde_json::Error,
	},

	#[error(transparent)]
	UnsafeOutput(#[from] CodamaError),

	#[error("CPI crate rendering failed for {path}: {source}")]
	Render {
		path: PathBuf,
		source: pina_cpi_renderer::RenderError,
	},

	#[error("Standard input must contain a Codama root node")]
	ExpectedCodamaStdin,
}

/// Generate a standalone Pina CPI crate from a Codama or Anchor IDL file.
///
/// Codama roots are rendered natively. Other JSON documents are passed through
/// `@codama/nodes-from-anchor` before the same renderer is invoked.
///
/// # Errors
///
/// Returns an error when the input is unsafe or malformed, Anchor conversion
/// fails, or the generated crate cannot be written.
pub fn generate_cpi_crate(options: &CpiGenerateOptions) -> Result<(), CpiGenerateError> {
	let input = read_idl(&options.idl)?;
	let root = normalize_idl(&input, &options.idl, &options.npx)?;

	render_cpi_root(&root, &options.output, options.mode, options.scaffold)
}

/// Generate a standalone Pina CPI crate from a reader.
///
/// This is the pipeline entry point used by Codama visitors, which always pass
/// their current normalized Codama root. Use [`generate_cpi_crate`] when a raw
/// Anchor IDL still needs normalization.
///
/// # Errors
///
/// Returns an error when reading, normalization, or rendering fails.
pub fn generate_cpi_crate_from_reader(
	reader: impl Read,
	output: &Path,
) -> Result<(), CpiGenerateError> {
	generate_cpi_crate_from_reader_with_config(reader, output, GenerationMode::Auto, true)
}

/// Generate a standalone Pina CPI crate from a reader with explicit output settings.
///
/// # Errors
///
/// Returns an error when reading, normalization, or rendering fails.
pub fn generate_cpi_crate_from_reader_with_config(
	reader: impl Read,
	output: &Path,
	mode: GenerationMode,
	scaffold: bool,
) -> Result<(), CpiGenerateError> {
	let path = PathBuf::from("<stdin>");
	let input = read_bounded(reader, &path)?;
	let value = parse_json(&input, &path)?;
	let Some(root) = parse_codama(&value, &path)? else {
		return Err(CpiGenerateError::ExpectedCodamaStdin);
	};

	render_cpi_root(&root, output, mode, scaffold)
}

fn read_idl(path: &Path) -> Result<Vec<u8>, CpiGenerateError> {
	let file = std::fs::File::open(path).map_err(|source| {
		CpiGenerateError::ReadIdl {
			path: path.to_path_buf(),
			source,
		}
	})?;

	read_bounded(file, path)
}

fn read_bounded(mut reader: impl Read, path: &Path) -> Result<Vec<u8>, CpiGenerateError> {
	let mut bytes = Vec::new();
	reader
		.by_ref()
		.take(MAX_IDL_BYTES + 1)
		.read_to_end(&mut bytes)
		.map_err(|source| {
			CpiGenerateError::ReadIdl {
				path: path.to_path_buf(),
				source,
			}
		})?;

	if bytes.len() as u64 > MAX_IDL_BYTES {
		return Err(CpiGenerateError::IdlTooLarge {
			path: path.to_path_buf(),
		});
	}

	Ok(bytes)
}

fn normalize_idl(input: &[u8], path: &Path, npx: &str) -> Result<RootNode, CpiGenerateError> {
	let value = parse_json(input, path)?;

	match parse_codama(&value, path)? {
		Some(root) => Ok(root),
		None => convert_anchor(path, npx),
	}
}

fn parse_json(input: &[u8], path: &Path) -> Result<Value, CpiGenerateError> {
	serde_json::from_slice(input).map_err(|source| {
		CpiGenerateError::InvalidJson {
			path: path.to_path_buf(),
			source,
		}
	})
}

fn parse_codama(value: &Value, path: &Path) -> Result<Option<RootNode>, CpiGenerateError> {
	match serde_json::from_value::<RootNode>(value.clone()) {
		Ok(root) => Ok(Some(root)),
		Err(source) if value.get("standard").and_then(Value::as_str) == Some("codama") => {
			Err(CpiGenerateError::InvalidCodama {
				path: path.to_path_buf(),
				source,
			})
		}
		Err(_) => Ok(None),
	}
}

fn convert_anchor(path: &Path, npx: &str) -> Result<RootNode, CpiGenerateError> {
	let (output, command_name) = if Path::new(npx).file_stem() == Some(OsStr::new("node")) {
		(run_project_converter(npx, path)?, "node")
	} else {
		(run_npx_converter(npx, path)?, npx)
	};

	if !output.status.success() {
		return Err(converter_failure(&output, command_name));
	}

	serde_json::from_slice(&output.stdout).map_err(|source| {
		CpiGenerateError::InvalidConvertedIdl {
			path: path.to_path_buf(),
			source,
		}
	})
}

fn run_project_converter(
	node: &str,
	path: &Path,
) -> Result<std::process::Output, CpiGenerateError> {
	run_node_converter(
		Command::new(node),
		path,
		"node",
		ConverterResolution::Project,
	)
}

fn run_npx_converter(npx: &str, path: &Path) -> Result<std::process::Output, CpiGenerateError> {
	run_npx_converter_with_temp_dir(npx, path, tempfile::tempdir())
}

fn run_npx_converter_with_temp_dir(
	npx: &str,
	path: &Path,
	isolated_dir: std::io::Result<tempfile::TempDir>,
) -> Result<std::process::Output, CpiGenerateError> {
	let absolute_path = path.canonicalize().map_err(|source| {
		CpiGenerateError::ReadIdl {
			path: path.to_path_buf(),
			source,
		}
	})?;
	let isolated_dir = isolated_dir.map_err(|source| {
		CpiGenerateError::RunConverter {
			cmd: npx.to_string(),
			source,
		}
	})?;
	let mut command = Command::new(npx);
	command
		.current_dir(isolated_dir.path())
		.args(["-y", "-p", NODES_FROM_ANCHOR_PACKAGE, "node"]);

	run_node_converter(command, &absolute_path, npx, ConverterResolution::Npx)
}

fn run_node_converter(
	mut command: Command,
	path: &Path,
	name: &str,
	resolution: ConverterResolution,
) -> Result<std::process::Output, CpiGenerateError> {
	command
		.args(["--input-type=module", "--eval", ANCHOR_CONVERT_SCRIPT])
		.arg(resolution.as_str())
		.arg(path);
	command.output().map_err(|source| {
		CpiGenerateError::RunConverter {
			cmd: name.to_string(),
			source,
		}
	})
}

fn converter_failure(output: &std::process::Output, cmd: &str) -> CpiGenerateError {
	let stderr = diagnostic_text(&output.stderr);
	let stdout = diagnostic_text(&output.stdout);
	let details = if !stderr.is_empty() {
		format!(": {stderr}")
	} else if !stdout.is_empty() {
		format!(": {stdout}")
	} else {
		String::new()
	};

	CpiGenerateError::ConverterFailed {
		cmd: cmd.to_string(),
		status: output.status.code().unwrap_or(-1),
		details,
	}
}

fn diagnostic_text(bytes: &[u8]) -> String {
	const MAX_DIAGNOSTIC_BYTES: usize = 16 * 1024;

	let start = bytes.len().saturating_sub(MAX_DIAGNOSTIC_BYTES);
	String::from_utf8_lossy(&bytes[start..])
		.trim()
		.chars()
		.flat_map(char::escape_default)
		.collect()
}

fn render_cpi_root(
	root: &RootNode,
	output: &Path,
	mode: GenerationMode,
	scaffold: bool,
) -> Result<(), CpiGenerateError> {
	validate_render_target(output)?;
	let config = RenderConfig {
		mode: cpi_render_mode(mode),
		scaffold,
		..RenderConfig::default()
	};
	pina_cpi_renderer::render_root_node(root, output, &config).map_err(|source| {
		CpiGenerateError::Render {
			path: output.to_path_buf(),
			source,
		}
	})
}

const fn cpi_render_mode(mode: GenerationMode) -> RenderMode {
	match mode {
		GenerationMode::Auto => RenderMode::Auto,
		GenerationMode::Create => RenderMode::Create,
		GenerationMode::Update => RenderMode::Update,
		GenerationMode::Overwrite => RenderMode::Overwrite,
	}
}

#[cfg(test)]
mod tests {
	use std::io::Cursor;
	use std::io::Error;
	use std::io::ErrorKind;
	use std::process::ExitStatus;

	use tempfile::TempDir;

	use super::*;

	fn fixture_path() -> PathBuf {
		Path::new(env!("CARGO_MANIFEST_DIR"))
			.parent()
			.and_then(Path::parent)
			.unwrap_or_else(|| Path::new("."))
			.join("codama/idls/vesting_program.json")
	}

	fn fixture_bytes() -> Vec<u8> {
		std::fs::read(fixture_path())
			.unwrap_or_else(|error| panic!("failed to read fixture: {error}"))
	}

	#[cfg(unix)]
	fn executable(path: &Path, contents: &str) {
		use std::os::unix::fs::PermissionsExt;

		std::fs::write(path, contents)
			.unwrap_or_else(|error| panic!("failed to write fake command: {error}"));
		let mut permissions = std::fs::metadata(path)
			.unwrap_or_else(|error| panic!("failed to stat fake command: {error}"))
			.permissions();
		permissions.set_mode(0o755);
		std::fs::set_permissions(path, permissions)
			.unwrap_or_else(|error| panic!("failed to chmod fake command: {error}"));
	}

	#[test]
	fn codama_files_and_readers_render_the_same_cpi_surface() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
		let file_output = temp.path().join("file-cpi");
		let reader_output = temp.path().join("reader-cpi");

		generate_cpi_crate(&CpiGenerateOptions {
			idl: fixture_path(),
			output: file_output.clone(),
			mode: GenerationMode::Auto,
			scaffold: true,
			npx: "must-not-run".to_string(),
		})
		.unwrap_or_else(|error| panic!("file generation failed: {error}"));
		generate_cpi_crate_from_reader(Cursor::new(fixture_bytes()), &reader_output)
			.unwrap_or_else(|error| panic!("reader generation failed: {error}"));

		for output in [file_output, reader_output] {
			let source =
				std::fs::read_to_string(output.join("src/generated/instructions/initialize.rs"))
					.unwrap_or_else(|error| panic!("failed to read generated source: {error}"));
			assert!(source.contains("pub fn invoke(&self, program: &ProgramAccount<'_>)"));
			assert!(source.contains("pub fn invoke_signed("));
			assert!(source.contains("pub struct Initialize<'account>"));
			assert!(source.contains("pub ix: InitializeIx"));
			assert!(source.contains("let data = self.ix.to_bytes()?;"));
			assert!(source.contains("CpiContext::new(*program, accounts)"));
		}
	}

	#[test]
	fn input_validation_is_bounded_and_fail_closed() {
		let path = Path::new("input.json");
		let malformed = parse_json(b"{", path).expect_err("malformed JSON must fail");
		assert!(matches!(malformed, CpiGenerateError::InvalidJson { .. }));

		let invalid_codama = serde_json::json!({ "standard": "codama" });
		let error = parse_codama(&invalid_codama, path).expect_err("invalid root must fail");
		assert!(matches!(error, CpiGenerateError::InvalidCodama { .. }));

		let anchor = serde_json::json!({ "version": "0.1.0" });
		assert!(parse_codama(&anchor, path).unwrap_or(None).is_none());

		let oversized = Cursor::new(vec![0u8; (MAX_IDL_BYTES + 1) as usize]);
		assert!(matches!(
			read_bounded(oversized, path),
			Err(CpiGenerateError::IdlTooLarge { .. })
		));

		struct BrokenReader;

		impl Read for BrokenReader {
			fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
				Err(Error::from(ErrorKind::PermissionDenied))
			}
		}

		assert!(matches!(
			read_bounded(BrokenReader, path),
			Err(CpiGenerateError::ReadIdl { .. })
		));
		assert!(matches!(
			generate_cpi_crate_from_reader(Cursor::new(b"{}"), Path::new("unused")),
			Err(CpiGenerateError::ExpectedCodamaStdin)
		));
	}

	#[test]
	fn file_and_output_failures_keep_their_context() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
		let missing = temp.path().join("missing.json");
		assert!(matches!(
			generate_cpi_crate(&CpiGenerateOptions {
				idl: missing,
				output: temp.path().join("missing-output"),
				mode: GenerationMode::Auto,
				scaffold: true,
				npx: "npx".to_string(),
			}),
			Err(CpiGenerateError::ReadIdl { .. })
		));

		assert!(matches!(
			generate_cpi_crate_from_reader(Cursor::new(fixture_bytes()), Path::new("/")),
			Err(CpiGenerateError::UnsafeOutput(_))
		));

		let mut root: RootNode = serde_json::from_slice(&fixture_bytes())
			.unwrap_or_else(|error| panic!("fixture parse failed: {error}"));
		root.program.public_key = "not a public key".to_string();
		assert!(matches!(
			render_cpi_root(
				&root,
				&temp.path().join("invalid-program"),
				GenerationMode::Auto,
				true,
			),
			Err(CpiGenerateError::Render { .. })
		));
	}

	#[cfg(unix)]
	#[test]
	fn anchor_conversion_supports_node_and_npx_commands() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
		let converted = temp.path().join("converted.json");
		let anchor = temp.path().join("anchor.json");
		let node = temp.path().join("node");
		let fake_npx = temp.path().join("fake-npx");
		let script = format!("#!/bin/sh\nexec /bin/cat '{}'\n", converted.display());
		std::fs::write(&converted, fixture_bytes())
			.unwrap_or_else(|error| panic!("failed to write converted IDL: {error}"));
		std::fs::write(&anchor, br#"{"version":"0.1.0"}"#)
			.unwrap_or_else(|error| panic!("failed to write Anchor IDL: {error}"));
		executable(&node, &script);
		executable(&fake_npx, &script);

		for (command, name) in [(&node, "node"), (&fake_npx, "npx")] {
			let output = temp.path().join(format!("{name}-output"));
			generate_cpi_crate(&CpiGenerateOptions {
				idl: anchor.clone(),
				output: output.clone(),
				mode: GenerationMode::Auto,
				scaffold: true,
				npx: command.to_string_lossy().into_owned(),
			})
			.unwrap_or_else(|error| panic!("{name} conversion failed: {error}"));
			assert!(output.join("src/generated/mod.rs").is_file());
		}
	}

	#[cfg(unix)]
	#[test]
	fn npx_resolution_does_not_load_a_project_local_converter() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
		let project = temp.path().join("project");
		let npx_root = temp.path().join("npx/node_modules");
		let anchor = temp.path().join("anchor.json");
		std::fs::create_dir_all(&project)
			.unwrap_or_else(|error| panic!("failed to create project: {error}"));
		std::fs::write(&anchor, b"{}")
			.unwrap_or_else(|error| panic!("failed to write Anchor IDL: {error}"));

		let write_package = |node_modules: &Path, source: &str| {
			let package = node_modules.join("@codama/nodes-from-anchor");
			std::fs::create_dir_all(&package)
				.unwrap_or_else(|error| panic!("failed to create converter package: {error}"));
			std::fs::write(
				package.join("package.json"),
				br#"{"type":"module","exports":"./index.js"}"#,
			)
			.unwrap_or_else(|error| panic!("failed to write converter manifest: {error}"));
			std::fs::write(package.join("index.js"), source)
				.unwrap_or_else(|error| panic!("failed to write converter module: {error}"));
		};

		write_package(
			&project.join("node_modules"),
			"throw new Error('project-local sentinel was loaded');\n",
		);
		let fixture = String::from_utf8(fixture_bytes())
			.unwrap_or_else(|error| panic!("fixture must be UTF-8: {error}"));
		write_package(
			&npx_root,
			&format!("export const rootNodeFromAnchor = () => ({fixture});\n"),
		);
		let npx_bin = npx_root.join(".bin");
		std::fs::create_dir_all(&npx_bin)
			.unwrap_or_else(|error| panic!("failed to create npx bin: {error}"));

		let mut command = Command::new("node");
		command.current_dir(&project);
		let inherited_path = std::env::var_os("PATH").unwrap_or_default();
		let mut paths = vec![npx_bin];
		paths.extend(std::env::split_paths(&inherited_path));
		command.env(
			"PATH",
			std::env::join_paths(paths)
				.unwrap_or_else(|error| panic!("failed to assemble PATH: {error}")),
		);

		let output = run_node_converter(command, &anchor, "node", ConverterResolution::Npx)
			.unwrap_or_else(|error| panic!("converter failed: {error}"));
		let diagnostic = diagnostic_text(&output.stderr);
		assert!(output.status.success(), "{}", diagnostic);
		serde_json::from_slice::<RootNode>(&output.stdout)
			.unwrap_or_else(|error| panic!("safe converter output was invalid: {error}"));
	}

	#[cfg(unix)]
	#[test]
	fn converter_failures_are_bounded_and_actionable() {
		use std::os::unix::process::ExitStatusExt;

		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
		let anchor = temp.path().join("anchor.json");
		std::fs::write(&anchor, b"{}")
			.unwrap_or_else(|error| panic!("failed to write Anchor IDL: {error}"));
		assert!(matches!(
			run_npx_converter("npx", &temp.path().join("missing.json")),
			Err(CpiGenerateError::ReadIdl { .. })
		));
		assert!(matches!(
			run_npx_converter_with_temp_dir(
				"npx",
				&anchor,
				Err(Error::other("temp directory failed")),
			),
			Err(CpiGenerateError::RunConverter { .. })
		));

		assert!(matches!(
			convert_anchor(
				&anchor,
				temp.path().join("missing-node").to_str().unwrap_or("")
			),
			Err(CpiGenerateError::RunConverter { .. })
		));

		let invalid = temp.path().join("node");
		executable(&invalid, "#!/bin/sh\nprintf 'not json'\n");
		assert!(matches!(
			convert_anchor(&anchor, invalid.to_str().unwrap_or("")),
			Err(CpiGenerateError::InvalidConvertedIdl { .. })
		));

		let failed = temp.path().join("failed-node");
		executable(
			&failed,
			"#!/bin/sh\nprintf 'unsafe\\n' >&2\nprintf 'ignored'\nexit 7\n",
		);
		let error = convert_anchor(&anchor, failed.to_str().unwrap_or(""))
			.expect_err("failed converter must fail");
		let message = error.to_string();
		assert!(message.contains("status 7"));
		assert!(message.contains("unsafe"));

		let stdout_only = converter_failure(
			&std::process::Output {
				status: ExitStatus::from_raw(2 << 8),
				stdout: b"stdout detail".to_vec(),
				stderr: Vec::new(),
			},
			"fake",
		);
		assert!(stdout_only.to_string().contains("stdout detail"));

		let empty = converter_failure(
			&std::process::Output {
				status: ExitStatus::from_raw(3 << 8),
				stdout: Vec::new(),
				stderr: Vec::new(),
			},
			"fake",
		);
		assert!(!empty.to_string().ends_with(':'));

		let long = vec![b'x'; 20 * 1024];
		assert_eq!(diagnostic_text(&long).len(), 16 * 1024);
		assert_eq!(diagnostic_text(b"first\nsecond"), "first\\nsecond");
	}
}
