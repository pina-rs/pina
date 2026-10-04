//! `pina profile trace`: trace-driven, source-mapped compute-unit profiles.
//!
//! The workflow builds the program twice with `cargo build-sbf`, both with
//! the production size profile `pina build` uses:
//!
//! 1. With DWARF line tables and an unstripped linker output. The tests load
//!    the stripped copy and attribution reads the unstripped one, because
//!    Mollusk cannot load a program whose `.symtab` holds long Rust symbol
//!    names while it traces.
//! 2. As the plain release build, to report whether debug information changed
//!    code generation. It runs second so the shared linker output under the
//!    Cargo target directory ends up matching `target/deploy` again.
//!
//! It then runs the project's tests with `SBF_TRACE_DIR` set, so a
//! `mollusk-svm` dev-dependency built with `register-tracing` records every
//! top-level invocation, and attributes each executed instruction to a source
//! line and call stack.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitStatus;

use atomic_write_file::AtomicWriteFile;
use heck::ToSnakeCase;
use pina_profile::trace_report::ObservedDiscriminator;
use pina_profile::trace_report::ReleaseComparison;
use pina_profile::trace_report::TraceReport;
use pina_profile::trace_report::TracedProgram;
use serde::Serialize;

use crate::build::BuildError;
use crate::build::BuildOptions;
use crate::build::SbfCompilation;
use crate::build::SizeProfile;
use crate::build::compile_sbf;
use crate::build::unstripped_artifact;
use crate::ir::InstructionIr;
use crate::project::Project;
use crate::project::ProjectError;
use crate::project::WorkspaceLayout;

/// The self-contained HTML report template.
const HTML_TEMPLATE: &str = include_str!("../templates/profile-trace.html");

/// Marker the report data replaces inside the template's JSON script element.
const HTML_DATA_MARKER: &str = "/*PINA_TRACE_DATA*/";

/// Largest source file embedded in the HTML report.
const MAX_SOURCE_BYTES: u64 = 2 * 1024 * 1024;

/// Paths of one project's trace workflow, all under `<target>/pina/trace/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceLayout {
	/// `<target>/pina/trace`.
	pub root: PathBuf,
	/// `SBF_OUT_DIR` for the traced tests.
	pub program_dir: PathBuf,
	/// The stripped traced program the tests load.
	pub program: PathBuf,
	/// The unstripped traced program, with DWARF line tables.
	pub debug_program: PathBuf,
	/// `--sbf-out-dir` of the comparison release build.
	pub release_dir: PathBuf,
	/// The release program the traced build is compared with.
	pub release_program: PathBuf,
	/// `SBF_TRACE_DIR` for the traced tests.
	pub traces: PathBuf,
	/// The HTML report.
	pub report: PathBuf,
}

impl TraceLayout {
	/// The trace layout for `project`.
	#[must_use]
	pub fn new(project: &Project) -> Self {
		let root = project.target_dir.join("pina").join("trace");
		let program_file = format!("{}.so", project.library_name);
		let program_dir = root.join("build");
		let release_dir = root.join("release");

		Self {
			program: program_dir.join(&program_file),
			debug_program: root.join(format!("{program_file}.debug")),
			release_program: release_dir.join(&program_file),
			traces: root.join("traces"),
			report: root.join(format!("{}.html", project.library_name)),
			program_dir,
			release_dir,
			root,
		}
	}
}

/// Inputs for [`trace_project`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraceOptions {
	/// Directory inside the project.
	pub project: PathBuf,
	/// Cargo test-name filter for the traced tests.
	pub filter: Option<String>,
	/// Analyze these existing traces instead of building and running tests.
	pub trace_dir: Option<PathBuf>,
}

/// A finished trace analysis.
#[derive(Debug, Clone)]
pub struct TraceRun {
	/// The discovered project.
	pub project: Project,
	/// The workspace that owns it, used to resolve recorded source paths.
	pub workspace: WorkspaceLayout,
	/// Where the workflow's files live.
	pub layout: TraceLayout,
	/// The profile of every traced instruction.
	pub report: TraceReport,
	/// Non-fatal problems to show the user.
	pub warnings: Vec<String>,
}

/// Trace workflow failures.
#[derive(Debug, thiserror::Error)]
pub enum TraceError {
	#[error(transparent)]
	Project(#[from] ProjectError),

	#[error(transparent)]
	Build(#[from] BuildError),

	#[error(transparent)]
	Profile(#[from] pina_profile::ProfileError),

	#[error(transparent)]
	Trace(#[from] pina_profile::trace::TraceError),

	#[error("failed to prepare {path:?}: {source}")]
	Prepare {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("cargo build-sbf left no unstripped program under {target:?}")]
	MissingDebugBuild { target: PathBuf },

	#[error(
		"no traced build at {path:?}; run `pina profile trace` without --trace-dir to build one"
	)]
	MissingTracedBuild { path: PathBuf },

	#[error("failed to run {program:?}: {source}")]
	RunTests {
		program: OsString,
		source: std::io::Error,
	},

	#[error(
		"`cargo test` exited unsuccessfully with {status}. Traces recorded before the failure \
		 remain in {traces:?}; analyze them with `pina profile trace --trace-dir {traces:?}`."
	)]
	TestsFailed { status: ExitStatus, traces: PathBuf },

	#[error(
		"no SBF register traces were recorded in {directory:?}.\nMollusk records traces only \
		 when its `register-tracing` feature is enabled. Declare the dev-dependency in \
		 {manifest:?} as:\n\n    {declaration}\n\nand make sure the selected tests execute the \
		 program through Mollusk."
	)]
	NoTraces {
		directory: PathBuf,
		manifest: PathBuf,
		declaration: String,
	},

	#[error(
		"{recorded} recorded traces in {directory:?}, but none executed this build of \
		 {program:?} (sha256 {sha256}). Tests must load the program from SBF_OUT_DIR; a copy in \
		 tests/fixtures/ takes precedence over it."
	)]
	NoMatchingTraces {
		recorded: usize,
		directory: PathBuf,
		program: String,
		sha256: String,
	},

	#[error("no traced instruction is named {requested:?}; traced: {available}")]
	UnknownInstruction {
		requested: String,
		available: String,
	},

	#[error("failed to serialize the HTML report data: {0}")]
	SerializeReport(serde_json::Error),

	#[error("refusing to write {path:?}: the path contains a symbolic link or reparse point")]
	UnsafeOutput { path: PathBuf },

	#[error("failed to write {path:?}: {source}")]
	Write {
		path: PathBuf,
		source: std::io::Error,
	},
}

impl TraceError {
	/// The process exit code: the test runner's code when the tests failed.
	#[must_use]
	pub fn exit_code(&self) -> i32 {
		match self {
			Self::TestsFailed { status, .. } => status.code().unwrap_or(1),
			_ => 1,
		}
	}
}

/// Build, run, and analyze a project's traced tests, or analyze existing
/// traces when [`TraceOptions::trace_dir`] is set.
///
/// # Errors
///
/// Returns an error when discovery, either build, the test run, trace
/// parsing, or attribution fails, when no trace was recorded, or when no
/// recording executed the traced build.
pub fn trace_project(options: &TraceOptions) -> Result<TraceRun, TraceError> {
	let project = Project::discover(&options.project)?;
	let layout = TraceLayout::new(&project);
	let traces_dir = if let Some(directory) = &options.trace_dir {
		directory.clone()
	} else {
		build_traceable(&project, &layout)?;
		run_traced_tests(&project, &layout, options.filter.as_deref())?;
		layout.traces.clone()
	};

	for path in [&layout.program, &layout.debug_program] {
		if !path.is_file() {
			return Err(TraceError::MissingTracedBuild { path: path.clone() });
		}
	}

	let program = TracedProgram::load(&layout.program, &layout.debug_program)?;
	let traces = pina_profile::trace::read_trace_dir(&traces_dir)?;

	if traces.is_empty() {
		let manifest = project.program_dir.join("Cargo.toml");
		return Err(TraceError::NoTraces {
			directory: traces_dir,
			declaration: register_tracing_declaration(&manifest),
			manifest,
		});
	}

	let mut report = pina_profile::trace_report::analyze_traces(&traces, &program);

	if report.instructions.is_empty() {
		return Err(TraceError::NoMatchingTraces {
			recorded: report.recorded_traces,
			directory: traces_dir,
			program: report.program,
			sha256: report.executable_sha256,
		});
	}

	let mut warnings = Vec::new();
	report.release_build = release_comparison(&layout)?;

	if let Some(comparison) = report.release_build
		&& !comparison.identical_text
	{
		warnings.push(divergence_warning(comparison));
	}

	name_instructions(&project, &mut report, &mut warnings);

	Ok(TraceRun {
		workspace: project.workspace_layout()?,
		project,
		layout,
		report,
		warnings,
	})
}

fn build_traceable(project: &Project, layout: &TraceLayout) -> Result<(), TraceError> {
	let options = BuildOptions {
		project_dir: project.root.clone(),
		features: Vec::new(),
		no_default_features: false,
		size_profile: SizeProfile::default(),
	};

	std::fs::create_dir_all(&layout.root).map_err(prepare_error(&layout.root))?;
	compile_sbf(
		project,
		&options,
		&SbfCompilation {
			out_dir: &layout.program_dir,
			line_tables: true,
			quiet_stdout: true,
		},
	)?;

	let linker_output = unstripped_artifact(project).ok_or_else(|| {
		TraceError::MissingDebugBuild {
			target: project.target_dir.clone(),
		}
	})?;
	std::fs::copy(&linker_output, &layout.debug_program)
		.map_err(prepare_error(&layout.debug_program))?;

	compile_sbf(
		project,
		&options,
		&SbfCompilation {
			out_dir: &layout.release_dir,
			line_tables: false,
			quiet_stdout: true,
		},
	)?;

	Ok(())
}

fn prepare_error(path: &Path) -> impl FnOnce(std::io::Error) -> TraceError {
	let path = path.to_path_buf();
	move |source| TraceError::Prepare { path, source }
}

/// Run the project's tests with tracing pointed at the traced build.
///
/// Mollusk searches `BPF_OUT_DIR` before `SBF_OUT_DIR`, so an inherited
/// `BPF_OUT_DIR` is removed. `SBF_DEBUG_PORT` would make a debugger-enabled
/// Mollusk wait for a client, and `SBF_TRACE_DISASSEMBLE` only adds files.
fn run_traced_tests(
	project: &Project,
	layout: &TraceLayout,
	filter: Option<&str>,
) -> Result<(), TraceError> {
	match std::fs::remove_dir_all(&layout.traces) {
		Ok(()) => {}
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
		Err(error) => return Err(prepare_error(&layout.traces)(error)),
	}
	std::fs::create_dir_all(&layout.traces).map_err(prepare_error(&layout.traces))?;

	let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
	let mut command = Command::new(&cargo);
	command
		.current_dir(&project.root)
		.env("SBF_OUT_DIR", &layout.program_dir)
		.env("SBF_TRACE_DIR", &layout.traces)
		.env_remove("BPF_OUT_DIR")
		.env_remove("SBF_DEBUG_PORT")
		.env_remove("SBF_TRACE_DISASSEMBLE")
		.stdout(std::io::stderr())
		.arg("test")
		.arg("--manifest-path")
		.arg(project.program_dir.join("Cargo.toml"));

	if let Some(filter) = filter {
		command.arg(filter);
	}

	let status = command.status().map_err(|source| {
		TraceError::RunTests {
			program: cargo.clone(),
			source,
		}
	})?;

	if !status.success() {
		return Err(TraceError::TestsFailed {
			status,
			traces: layout.traces.clone(),
		});
	}

	Ok(())
}

/// The `mollusk-svm` dev-dependency declaration with `register-tracing`
/// enabled, matching how `manifest` already declares it.
#[must_use]
pub fn register_tracing_declaration(manifest: &Path) -> String {
	let declared = std::fs::read_to_string(manifest)
		.ok()
		.and_then(|text| text.parse::<toml::Table>().ok())
		.and_then(|table| table.get("dev-dependencies")?.get("mollusk-svm").cloned());
	let (source, mut features) = match declared {
		Some(toml::Value::String(version)) => (format!("version = {version:?}"), Vec::new()),
		Some(toml::Value::Table(table)) => {
			let source = if table.get("workspace") == Some(&toml::Value::Boolean(true)) {
				"workspace = true".to_owned()
			} else {
				let version = table
					.get("version")
					.and_then(toml::Value::as_str)
					.unwrap_or("0.15");
				format!("version = {version:?}")
			};
			let features = table
				.get("features")
				.and_then(toml::Value::as_array)
				.map(|features| {
					features
						.iter()
						.filter_map(toml::Value::as_str)
						.map(str::to_owned)
						.collect()
				})
				.unwrap_or_default();
			(source, features)
		}
		_ => ("version = \"0.15\"".to_owned(), Vec::new()),
	};

	if !features.iter().any(|feature| feature == "register-tracing") {
		features.push("register-tracing".to_owned());
	}

	let features = features
		.iter()
		.map(|feature| format!("{feature:?}"))
		.collect::<Vec<_>>()
		.join(", ");
	format!("mollusk-svm = {{ {source}, features = [{features}] }}")
}

/// Compare the traced build's `.text` with the release build's, when built.
fn release_comparison(layout: &TraceLayout) -> Result<Option<ReleaseComparison>, TraceError> {
	if !layout.release_program.is_file() {
		return Ok(None);
	}

	Ok(Some(ReleaseComparison::between(
		&layout.program,
		&layout.release_program,
	)?))
}

fn divergence_warning(comparison: ReleaseComparison) -> String {
	format!(
		"debug information changed code generation: {} of {} instruction slots differ from the \
		 release build ({} in the traced build). Executed counts describe the traced build and \
		 can differ slightly from the deployed program.",
		comparison.differing_instructions,
		comparison.release_instructions,
		comparison.traced_instructions,
	)
}

/// Name profiles from the program's instruction discriminators.
fn name_instructions(project: &Project, report: &mut TraceReport, warnings: &mut Vec<String>) {
	let auto = crate::migrations::manifest_auto_policy(&project.program_dir);
	let parsed = crate::parse::parse_program_with_auto(
		&project.program_dir,
		Some(&project.library_name),
		&auto,
	);

	match parsed {
		Ok(program) => {
			report.assign_names(|observed| instruction_for(&program.instructions, observed));
		}
		Err(error) => {
			warnings.push(format!(
				"instruction names are unavailable, so traces are labelled by id: {error}"
			));
		}
	}
}

/// The single instruction whose discriminator matches the loaded bytes.
///
/// Only the bytes both sides cover are compared: a wider load also read the
/// instruction's arguments, and a narrower one only part of the discriminator.
fn instruction_for(
	instructions: &[InstructionIr],
	observed: &ObservedDiscriminator,
) -> Option<String> {
	let mut matches = instructions.iter().filter(|instruction| {
		let bytes = usize::from(observed.width).min(instruction.discriminator.repr_size);
		let mask = if bytes >= 8 {
			u64::MAX
		} else {
			(1_u64 << (bytes * 8)) - 1
		};

		bytes > 0 && instruction.discriminator.value & mask == observed.value & mask
	});
	let first = matches.next()?;

	matches.next().is_none().then(|| first.name.clone())
}

/// Keep only the profiles of one instruction.
///
/// `requested` matches an instruction name in any case style (`increment`,
/// `Increment`) or a profile's display label (`increment #2`).
///
/// # Errors
///
/// Returns [`TraceError::UnknownInstruction`] listing the traced names when
/// nothing matches.
pub fn retain_instruction(report: &mut TraceReport, requested: &str) -> Result<(), TraceError> {
	let wanted = requested.to_snake_case();
	let available = report
		.instructions
		.iter()
		.map(|profile| profile.name.clone())
		.collect::<Vec<_>>()
		.join(", ");

	report.instructions.retain(|profile| {
		profile.name == requested
			|| profile
				.instruction
				.as_deref()
				.is_some_and(|name| name.to_snake_case() == wanted)
	});

	if report.instructions.is_empty() {
		return Err(TraceError::UnknownInstruction {
			requested: requested.to_owned(),
			available,
		});
	}

	Ok(())
}

/// Atomically write `contents` to `path`, refusing paths through links.
///
/// # Errors
///
/// Returns an error when the path traverses a link or the write fails.
pub fn write_output(path: &Path, contents: &[u8]) -> Result<(), TraceError> {
	let write_error = |source| {
		TraceError::Write {
			path: path.to_path_buf(),
			source,
		}
	};

	if crate::path_security::has_untrusted_link_component(path).map_err(write_error)? {
		return Err(TraceError::UnsafeOutput {
			path: path.to_path_buf(),
		});
	}

	let mut file = AtomicWriteFile::open(path).map_err(write_error)?;
	file.write_all(contents).map_err(write_error)?;
	file.commit().map_err(write_error)
}

/// A source file embedded in the HTML report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFile {
	/// The resolved path, relative to the workspace root when inside it.
	pub display_path: String,
	/// The file's lines, without line terminators.
	pub lines: Vec<String>,
}

/// The data the HTML report renders.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HtmlData<'a> {
	generator: String,
	report: &'a TraceReport,
	warnings: &'a [String],
	/// Source text by recorded path, for files that resolve on disk.
	sources: &'a BTreeMap<String, SourceFile>,
}

/// Write the self-contained HTML report to [`TraceLayout::report`].
///
/// Source files are read from disk now; files that do not resolve are shown
/// as line numbers only.
///
/// # Errors
///
/// Returns an error when the report cannot be serialized or written.
pub fn write_html_report(run: &TraceRun) -> Result<PathBuf, TraceError> {
	let sources = collect_sources(&run.report, &run.workspace, &run.project.program_dir);
	let html = render_html(&run.report, &run.warnings, &sources)?;

	std::fs::create_dir_all(&run.layout.root).map_err(prepare_error(&run.layout.root))?;
	write_output(&run.layout.report, html.as_bytes())?;

	Ok(run.layout.report.clone())
}

/// Render the HTML report around the template.
///
/// The data is embedded as JSON in a `<script type="application/json">`
/// element. `<`, `>`, `&`, and the JavaScript line separators are escaped as
/// `\uXXXX`, which keeps the JSON identical while making it impossible for a
/// path or symbol to close the element. The page renders every string with
/// `textContent`.
///
/// # Errors
///
/// Returns an error when the data cannot be serialized.
pub fn render_html(
	report: &TraceReport,
	warnings: &[String],
	sources: &BTreeMap<String, SourceFile>,
) -> Result<String, TraceError> {
	let data = HtmlData {
		generator: format!("pina {}", env!("CARGO_PKG_VERSION")),
		report,
		warnings,
		sources,
	};
	let json = serde_json::to_string(&data).map_err(TraceError::SerializeReport)?;

	Ok(HTML_TEMPLATE.replacen(HTML_DATA_MARKER, &escape_script_json(&json), 1))
}

/// Escape JSON so it can sit inside a `<script>` element.
#[must_use]
pub fn escape_script_json(json: &str) -> String {
	let mut escaped = String::with_capacity(json.len());

	for character in json.chars() {
		match character {
			'<' => escaped.push_str("\\u003c"),
			'>' => escaped.push_str("\\u003e"),
			'&' => escaped.push_str("\\u0026"),
			'\u{2028}' => escaped.push_str("\\u2028"),
			'\u{2029}' => escaped.push_str("\\u2029"),
			character => escaped.push(character),
		}
	}

	escaped
}

/// Read every sampled source file that resolves on disk.
#[must_use]
pub fn collect_sources(
	report: &TraceReport,
	workspace: &WorkspaceLayout,
	program_dir: &Path,
) -> BTreeMap<String, SourceFile> {
	let recorded: BTreeSet<&str> = report
		.instructions
		.iter()
		.flat_map(|profile| profile.lines.iter().map(|line| line.file.as_str()))
		.collect();

	recorded
		.into_iter()
		.filter_map(|file| {
			let path = resolve_source(file, workspace, program_dir)?;
			let source = read_source(&path)?;
			let display_path = path
				.strip_prefix(&workspace.root)
				.unwrap_or(&path)
				.to_string_lossy()
				.replace('\\', "/");

			Some((
				file.to_owned(),
				SourceFile {
					display_path,
					lines: source
						.lines()
						.map(|line| line.trim_end_matches('\r').to_owned())
						.collect(),
				},
			))
		})
		.collect()
}

/// Find a recorded source path on disk.
///
/// Cargo records workspace members relative to the workspace root, which is
/// how relative paths are resolved. Packages from a registry record paths
/// relative to their own root (`src/lib.rs`), so a relative path that lands
/// directly in a root package other than the program is ambiguous and is not
/// resolved. Absolute paths resolve when the file exists.
#[must_use]
pub fn resolve_source(
	recorded: &str,
	workspace: &WorkspaceLayout,
	program_dir: &Path,
) -> Option<PathBuf> {
	let recorded_path = Path::new(recorded);

	if recorded_path.is_absolute() {
		return recorded_path.is_file().then(|| recorded_path.to_path_buf());
	}

	// DWARF records `/`-separated paths. Joining them component by component
	// keeps them valid under any Windows root, including a verbatim one.
	let candidate = recorded
		.split('/')
		.fold(workspace.root.clone(), |path, component| {
			path.join(component)
		});

	if !candidate.is_file() {
		return None;
	}

	let owner = workspace
		.member_dirs
		.iter()
		.filter(|directory| candidate.starts_with(directory))
		.max_by_key(|directory| directory.components().count())?;
	let same = |left: &Path, right: &Path| {
		std::fs::canonicalize(left).ok() == std::fs::canonicalize(right).ok()
	};

	if same(owner, &workspace.root) && !same(owner, program_dir) {
		return None;
	}

	Some(candidate)
}

fn read_source(path: &Path) -> Option<String> {
	let metadata = std::fs::metadata(path).ok()?;

	if metadata.len() > MAX_SOURCE_BYTES {
		return None;
	}

	std::fs::read_to_string(path).ok()
}

#[cfg(test)]
mod tests {
	use std::fs;

	use pina_profile::trace_report::InstructionProfile;
	use pina_profile::trace_report::LineCost;

	use super::*;
	use crate::ir::DiscriminatorIr;

	fn instruction(name: &str, value: u64, repr_size: usize) -> InstructionIr {
		InstructionIr {
			name: name.to_owned(),
			rust_name: name.to_owned(),
			accounts: Vec::new(),
			arguments: Vec::new(),
			discriminator: DiscriminatorIr { value, repr_size },
			docs: Vec::new(),
		}
	}

	fn profile(name: &str, instruction: Option<&str>, file: &str) -> InstructionProfile {
		InstructionProfile {
			name: name.to_owned(),
			instruction: instruction.map(str::to_owned),
			discriminator: None,
			trace_ids: vec!["00".to_owned()],
			executed_instructions: 1,
			syscall_invocations: 0,
			unattributed_instructions: 0,
			lines: vec![LineCost {
				file: file.to_owned(),
				line: 1,
				executed_instructions: 1,
				syscalls: Vec::new(),
			}],
			functions: Vec::new(),
			syscalls: Vec::new(),
			stacks: Vec::new(),
		}
	}

	fn report(instructions: Vec<InstructionProfile>) -> TraceReport {
		TraceReport {
			schema_version: pina_profile::trace_report::TRACE_SCHEMA_VERSION,
			program: "demo".to_owned(),
			executable_sha256: String::new(),
			line_info: true,
			recorded_traces: instructions.len(),
			skipped_traces: 0,
			release_build: None,
			instructions,
		}
	}

	#[test]
	fn instruction_for_compares_the_bytes_both_sides_cover() {
		let instructions = [
			instruction("initialize", 0, 1),
			instruction("increment", 1, 1),
			instruction("wide", 0x0102, 2),
		];
		let observed = |width: u8, value: u64| ObservedDiscriminator { width, value };

		assert_eq!(
			instruction_for(&instructions, &observed(1, 1)).as_deref(),
			Some("increment")
		);
		// An eight-byte load also read the arguments after the discriminator.
		assert_eq!(
			instruction_for(&instructions, &observed(8, 0xff00_0102)).as_deref(),
			Some("wide")
		);
		// A one-byte load compares only the low byte of a wider discriminator.
		assert_eq!(
			instruction_for(&instructions, &observed(1, 2)).as_deref(),
			Some("wide")
		);
		// Ambiguous or missing matches stay unnamed.
		let ambiguous = [
			instruction("initialize", 0, 1),
			instruction("other", 0x0200, 2),
		];
		assert_eq!(instruction_for(&ambiguous, &observed(1, 0)), None);
		assert_eq!(instruction_for(&instructions, &observed(1, 9)), None);
		assert_eq!(
			instruction_for(&[instruction("zero", 0, 0)], &observed(1, 0)),
			None
		);
		assert_eq!(
			instruction_for(&[instruction("long", u64::MAX, 8)], &observed(8, u64::MAX)).as_deref(),
			Some("long")
		);
	}

	#[test]
	fn retain_instruction_matches_names_in_any_case_or_labels() {
		let mut by_name = report(vec![
			profile("increment", Some("increment"), "a.rs"),
			profile("initialize", Some("initialize"), "a.rs"),
		]);
		let retained = retain_instruction(&mut by_name, "Increment");
		assert!(retained.is_ok(), "{retained:?}");
		assert_eq!(by_name.instructions.len(), 1);

		let mut by_label = report(vec![profile("trace 00", None, "a.rs")]);
		let retained = retain_instruction(&mut by_label, "trace 00");
		assert!(retained.is_ok(), "{retained:?}");
		assert_eq!(by_label.instructions.len(), 1);

		let mut missing = report(vec![profile("increment", Some("increment"), "a.rs")]);
		let error = retain_instruction(&mut missing, "transfer")
			.expect_err("an untraced instruction must fail");
		assert_eq!(
			error.to_string(),
			"no traced instruction is named \"transfer\"; traced: increment"
		);
	}

	#[test]
	fn register_tracing_declaration_follows_the_manifest() {
		let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
		let manifest = temp.path().join("Cargo.toml");
		let declaration = |contents: &str| {
			fs::write(&manifest, contents).unwrap_or_else(|error| panic!("write: {error}"));
			register_tracing_declaration(&manifest)
		};

		assert_eq!(
			declaration("[dev-dependencies]\nmollusk-svm = { workspace = true }\n"),
			"mollusk-svm = { workspace = true, features = [\"register-tracing\"] }"
		);
		assert_eq!(
			declaration("[dev-dependencies]\nmollusk-svm = \"0.15.1\"\n"),
			"mollusk-svm = { version = \"0.15.1\", features = [\"register-tracing\"] }"
		);
		assert_eq!(
			declaration(
				"[dev-dependencies]\nmollusk-svm = { version = \"0.14\", features = [\"serde\"] }\n"
			),
			"mollusk-svm = { version = \"0.14\", features = [\"serde\", \"register-tracing\"] }"
		);
		assert_eq!(
			declaration(
				"[dev-dependencies]\nmollusk-svm = { git = \"x\", features = \
				 [\"register-tracing\"] }\n"
			),
			"mollusk-svm = { version = \"0.15\", features = [\"register-tracing\"] }"
		);
		assert_eq!(
			declaration("[package]\nname = \"demo\"\n"),
			"mollusk-svm = { version = \"0.15\", features = [\"register-tracing\"] }"
		);
		assert_eq!(
			register_tracing_declaration(&temp.path().join("missing.toml")),
			"mollusk-svm = { version = \"0.15\", features = [\"register-tracing\"] }"
		);
	}

	#[test]
	fn escape_script_json_keeps_data_inside_the_script_element() {
		let mut hostile = report(vec![profile(
			"</script><script>alert(1)</script>",
			None,
			"src/<b>&\u{2028}\u{2029}.rs",
		)]);
		hostile.program = "</script>".to_owned();

		let rendered = render_html(&hostile, &["<warn>".to_owned()], &BTreeMap::new());
		let html = rendered.unwrap_or_else(|error| panic!("render failed: {error}"));
		let marker = "id=\"trace-data\">";
		let start = html.find(marker).map(|index| index + marker.len());
		let start = start.unwrap_or_else(|| panic!("data element missing"));
		let end = html[start..].find("</script>").map(|index| start + index);
		let end = end.unwrap_or_else(|| panic!("data element is not closed"));
		let embedded = &html[start..end];
		let parsed = serde_json::from_str::<serde_json::Value>(embedded);
		let parsed = parsed.unwrap_or_else(|error| panic!("embedded JSON is invalid: {error}"));

		assert!(!embedded.contains('<'));
		assert!(!embedded.contains('>'));
		assert!(!embedded.contains('&'));
		assert!(!embedded.contains('\u{2028}'));
		assert_eq!(parsed["report"]["program"], "</script>");
		assert_eq!(
			parsed["report"]["instructions"][0]["lines"][0]["file"],
			"src/<b>&\u{2028}\u{2029}.rs"
		);
		assert!(!html.contains(HTML_DATA_MARKER));
	}

	fn workspace(root: &Path, members: &[&str]) -> WorkspaceLayout {
		WorkspaceLayout {
			root: root.to_path_buf(),
			member_dirs: members.iter().map(|member| root.join(member)).collect(),
		}
	}

	fn write_file(path: &Path, contents: &str) {
		let created = fs::create_dir_all(path.parent().unwrap_or(path));
		created.unwrap_or_else(|error| panic!("create {path:?}: {error}"));
		fs::write(path, contents).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
	}

	#[test]
	fn resolve_source_follows_cargo_path_conventions() {
		let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
		let root = temp.path().to_path_buf();
		write_file(&root.join("src/lib.rs"), "root\n");
		write_file(&root.join("programs/demo/src/lib.rs"), "demo\n");
		write_file(&root.join("loose.rs"), "loose\n");
		let program = root.join("programs/demo");
		let layout = workspace(&root, &["", "programs/demo"]);

		// Workspace-relative member paths resolve.
		assert_eq!(
			resolve_source("programs/demo/src/lib.rs", &layout, &program),
			Some(
				root.join("programs")
					.join("demo")
					.join("src")
					.join("lib.rs")
			)
		);
		// `src/lib.rs` in a root package that is not the program is ambiguous.
		assert_eq!(resolve_source("src/lib.rs", &layout, &program), None);
		// ...but it is the program's own file when the program is the root.
		assert_eq!(
			resolve_source("src/lib.rs", &layout, &root),
			Some(root.join("src").join("lib.rs"))
		);
		// Files outside every member, missing files, and absolute paths.
		let no_root_package = workspace(&root, &["programs/demo"]);
		assert_eq!(resolve_source("loose.rs", &no_root_package, &program), None);
		assert_eq!(resolve_source("src/missing.rs", &layout, &program), None);
		let absolute = root.join("loose.rs");
		assert_eq!(
			resolve_source(&absolute.to_string_lossy(), &layout, &program),
			Some(absolute)
		);
		assert_eq!(
			resolve_source("/nonexistent/file.rs", &layout, &program),
			None
		);
	}

	#[test]
	fn collect_sources_embeds_resolvable_sampled_files() {
		let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
		let root = temp.path().to_path_buf();
		write_file(&root.join("src/lib.rs"), "fn main() {}\r\nlet x = 1;\n");
		write_file(&root.join("src/huge.rs"), "");
		let huge = fs::File::options()
			.write(true)
			.open(root.join("src/huge.rs"));
		let grown = huge.and_then(|file| file.set_len(MAX_SOURCE_BYTES + 1));
		grown.unwrap_or_else(|error| panic!("grow huge.rs: {error}"));
		let layout = workspace(&root, &[""]);
		let sampled = report(vec![
			profile("a", None, "src/lib.rs"),
			profile("b", None, "src/huge.rs"),
			profile("c", None, "src/missing.rs"),
		]);

		let sources = collect_sources(&sampled, &layout, &root);

		assert_eq!(sources.len(), 1);
		assert_eq!(
			sources["src/lib.rs"],
			SourceFile {
				display_path: "src/lib.rs".to_owned(),
				lines: vec!["fn main() {}".to_owned(), "let x = 1;".to_owned()],
			}
		);
	}

	#[test]
	fn layout_keeps_every_file_under_the_trace_root() {
		let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
		write_file(
			&temp.path().join("Cargo.toml"),
			"[package]\nname = \"demo-program\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
		);
		write_file(&temp.path().join("src/lib.rs"), "");
		let mut project =
			Project::discover(temp.path()).unwrap_or_else(|error| panic!("discover: {error}"));
		project.target_dir = PathBuf::from("/target");
		let layout = TraceLayout::new(&project);

		assert_eq!(layout.root, Path::new("/target/pina/trace"));
		assert_eq!(
			layout.program,
			Path::new("/target/pina/trace/build/demo_program.so")
		);
		assert_eq!(
			layout.debug_program,
			Path::new("/target/pina/trace/demo_program.so.debug")
		);
		assert_eq!(
			layout.release_program,
			Path::new("/target/pina/trace/release/demo_program.so")
		);
		assert_eq!(layout.traces, Path::new("/target/pina/trace/traces"));
		assert_eq!(
			layout.report,
			Path::new("/target/pina/trace/demo_program.html")
		);
	}

	#[test]
	fn write_output_refuses_links_and_reports_failures() {
		let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
		let root = fs::canonicalize(temp.path()).unwrap_or_else(|error| panic!("canon: {error}"));
		let output = root.join("report.txt");

		write_output(&output, b"ok").unwrap_or_else(|error| panic!("write failed: {error}"));
		assert_eq!(fs::read(&output).unwrap_or_default(), b"ok");

		let directory = root.join("directory");
		fs::create_dir(&directory).unwrap_or_else(|error| panic!("mkdir: {error}"));
		let error = write_output(&directory, b"no").expect_err("a directory cannot be replaced");
		assert!(matches!(error, TraceError::Write { .. }), "{error}");

		let missing_parent = write_output(&root.join("missing").join("child"), b"no")
			.expect_err("a missing parent directory must fail");
		assert!(matches!(missing_parent, TraceError::Write { .. }));

		#[cfg(unix)]
		{
			let link = root.join("link");
			let linked = std::os::unix::fs::symlink(&directory, &link);
			linked.unwrap_or_else(|error| panic!("symlink: {error}"));
			let refused = write_output(&link.join("report.txt"), b"no")
				.expect_err("a linked path must be refused");
			assert!(matches!(refused, TraceError::UnsafeOutput { .. }));
		}
	}

	#[test]
	fn exit_code_propagates_test_failures_only() {
		#[cfg(unix)]
		{
			use std::os::unix::process::ExitStatusExt;

			let tests_failed = TraceError::TestsFailed {
				status: ExitStatus::from_raw(3 << 8),
				traces: PathBuf::from("traces"),
			};
			assert_eq!(tests_failed.exit_code(), 3);

			let signalled = TraceError::TestsFailed {
				status: ExitStatus::from_raw(9),
				traces: PathBuf::from("traces"),
			};
			assert_eq!(signalled.exit_code(), 1);
		}

		assert_eq!(
			TraceError::MissingDebugBuild {
				target: PathBuf::from("target"),
			}
			.exit_code(),
			1
		);
	}

	#[test]
	fn divergence_warning_quantifies_the_difference() {
		let warning = divergence_warning(ReleaseComparison::of(&[0; 16], &[1; 8]));

		assert!(
			warning.starts_with(
				"debug information changed code generation: 2 of 1 instruction slots differ from \
				 the release build (2 in the traced build)"
			),
			"{warning}"
		);
	}
}
