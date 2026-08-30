//! Project-aware test and development workflows.

use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;

use crate::build::BuildError;
use crate::build::build_project;
use crate::project::Project;
use crate::project::ProjectError;

/// The minimum Surfpool CLI version whose delegated flags Pina relies on.
pub const MINIMUM_SURFPOOL_VERSION: (u64, u64, u64) = (1, 5, 0);

const VERSION_OUTPUT_LIMIT: usize = 4 * 1024;

/// Errors produced by project test and development workflows.
#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
	#[error(transparent)]
	Project(#[from] ProjectError),

	#[error(transparent)]
	Build(#[from] BuildError),

	#[error("failed to run `{program}`: {source}")]
	RunCommand { program: String, source: io::Error },

	#[error("`{program}` exited unsuccessfully with {status}")]
	CommandFailed { program: String, status: ExitStatus },

	#[error("Surfpool integration test package is incomplete; missing: {path}")]
	MissingSurfpoolTest { path: PathBuf },

	#[error(
		"Surfpool's deployment runbook is missing: {path}; run `pina dev --yes` once to allow \
		 Surfpool to create it"
	)]
	MissingSurfpoolRunbook { path: PathBuf },

	#[error(
		"Surfpool's deployment runbook must be a regular file, not a symlink or reparse point: \
		 {path}"
	)]
	UnsafeSurfpoolRunbook { path: PathBuf },

	#[error("failed to inspect Surfpool's deployment runbook {path}: {source}")]
	InspectSurfpoolRunbook { path: PathBuf, source: io::Error },

	#[error(
		"Surfpool {found} is too old; Pina requires Surfpool {}.{}.{} or newer",
		MINIMUM_SURFPOOL_VERSION.0,
		MINIMUM_SURFPOOL_VERSION.1,
		MINIMUM_SURFPOOL_VERSION.2
	)]
	SurfpoolTooOld { found: String },

	#[error("could not parse the Surfpool version from `{output}`")]
	InvalidSurfpoolVersion { output: String },

	#[error(
		"unsafe Surfpool RPC URL; use an HTTP(S) endpoint with a host and without credentials, \
		 query parameters, fragments, or control characters"
	)]
	UnsafeSurfpoolRpcUrl,
}

impl WorkflowError {
	/// Return the child exit code when a delegated command failed.
	pub fn exit_code(&self) -> i32 {
		let Self::CommandFailed { status, .. } = self else {
			return 1;
		};

		if let Some(code) = status.code() {
			return code;
		}

		#[cfg(unix)]
		{
			use std::os::unix::process::ExitStatusExt;

			status.signal().map_or(1, |signal| 128 + signal)
		}

		#[cfg(not(unix))]
		{
			1
		}
	}
}

/// Network selection forwarded to a persistent Surfpool development instance.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum SurfpoolNetwork {
	/// Do not contact an upstream RPC endpoint.
	#[default]
	Offline,
	/// Fork from a named Solana cluster.
	Cluster(String),
	/// Fork from an explicit RPC endpoint.
	RpcUrl(String),
}

/// Inputs for `pina test`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TestOptions {
	/// Directory in or below the project.
	pub project: PathBuf,
	/// Run native Rust and Mollusk tests without building SBF or starting Surfpool.
	pub unit: bool,
	/// Optional Cargo test-name filter.
	pub filter: Option<String>,
}

/// Inputs for `pina dev`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DevOptions {
	/// Directory in or below the project.
	pub project: PathBuf,
	/// Upstream state selection. Offline is the safe default.
	pub network: SurfpoolNetwork,
	/// Allow Surfpool to create or update its deployment runbook without prompting.
	pub accept_runbook_changes: bool,
}

/// Run a project's fast native tests or its isolated Surfpool integration test.
pub fn test_project(options: &TestOptions) -> Result<(), WorkflowError> {
	let project = Project::discover(&options.project)?;

	if options.unit {
		return run_native_tests(&project, options.filter.as_deref());
	}

	require_surfpool_test(&project)?;
	let output = build_project(&options.project)?;
	run_surfpool_test(&project, &output.sbf_artifact, options.filter.as_deref())
}

/// Build a project, then delegate persistent watch and redeploy to Surfpool.
pub fn dev_project(options: &DevOptions) -> Result<(), WorkflowError> {
	validate_surfpool_network(&options.network)?;
	let project = Project::discover(&options.project)?;
	let runbook = project.root.join("txtx.yml");

	if !validate_runbook(&runbook)? && !options.accept_runbook_changes {
		return Err(WorkflowError::MissingSurfpoolRunbook { path: runbook });
	}

	let surfpool = executable("PINA_SURFPOOL", "surfpool");
	ensure_surfpool_version(&surfpool)?;
	let output = build_project(&options.project)?;

	let artifacts_dir = output
		.sbf_artifact
		.parent()
		.map_or_else(|| project.root.clone(), Path::to_path_buf);
	let arguments = dev_arguments(
		&artifacts_dir,
		&runbook,
		&options.network,
		options.accept_runbook_changes,
	);
	let mut command = Command::new(&surfpool);
	command.current_dir(&project.root).args(arguments);

	run(&mut command, &surfpool.to_string_lossy())
}

fn validate_surfpool_network(network: &SurfpoolNetwork) -> Result<(), WorkflowError> {
	let SurfpoolNetwork::RpcUrl(value) = network else {
		return Ok(());
	};

	if value.contains('#') || value.chars().any(char::is_control) {
		return Err(WorkflowError::UnsafeSurfpoolRpcUrl);
	}

	let endpoint = value
		.parse::<http::Uri>()
		.map_err(|_| WorkflowError::UnsafeSurfpoolRpcUrl)?;
	let scheme = endpoint
		.scheme_str()
		.ok_or(WorkflowError::UnsafeSurfpoolRpcUrl)?;
	let authority = endpoint
		.authority()
		.ok_or(WorkflowError::UnsafeSurfpoolRpcUrl)?;
	let safe_scheme = scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https");
	let safe = safe_scheme
		&& !authority.host().is_empty()
		&& !authority.as_str().contains('@')
		&& endpoint.query().is_none();

	if !safe {
		return Err(WorkflowError::UnsafeSurfpoolRpcUrl);
	}

	Ok(())
}

fn validate_runbook(path: &Path) -> Result<bool, WorkflowError> {
	validate_runbook_metadata(path, fs::symlink_metadata(path))
}

fn validate_runbook_metadata(
	path: &Path,
	metadata: io::Result<fs::Metadata>,
) -> Result<bool, WorkflowError> {
	let metadata = match metadata {
		Ok(metadata) => metadata,
		Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(false),
		Err(source) => {
			return Err(WorkflowError::InspectSurfpoolRunbook {
				path: path.to_path_buf(),
				source,
			});
		}
	};

	if metadata_is_path_indirection(&metadata) || !metadata.is_file() {
		return Err(WorkflowError::UnsafeSurfpoolRunbook {
			path: path.to_path_buf(),
		});
	}

	Ok(true)
}

fn metadata_is_path_indirection(metadata: &fs::Metadata) -> bool {
	if metadata.file_type().is_symlink() {
		return true;
	}

	#[cfg(windows)]
	{
		use std::os::windows::fs::MetadataExt;

		windows_attributes_indicate_reparse_point(metadata.file_attributes())
	}

	#[cfg(not(windows))]
	{
		false
	}
}

#[cfg(any(windows, test))]
const fn windows_attributes_indicate_reparse_point(attributes: u32) -> bool {
	const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;

	attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

fn require_surfpool_test(project: &Project) -> Result<(), WorkflowError> {
	for path in [
		project.program_dir.join("tests/surfpool/Cargo.toml"),
		project.program_dir.join("tests/surfpool/src/lib.rs"),
	] {
		if !path.is_file() {
			return Err(WorkflowError::MissingSurfpoolTest { path });
		}
	}

	Ok(())
}

fn run_native_tests(project: &Project, filter: Option<&str>) -> Result<(), WorkflowError> {
	let cargo = executable("CARGO", "cargo");
	let manifest = project.program_dir.join("Cargo.toml");
	let mut command = Command::new(&cargo);
	command
		.current_dir(&project.root)
		.arg("test")
		.arg("--manifest-path")
		.arg(manifest);

	if let Some(filter) = filter {
		command.arg(filter);
	}

	run(&mut command, &cargo.to_string_lossy())
}

fn run_surfpool_test(
	project: &Project,
	artifact: &Path,
	filter: Option<&str>,
) -> Result<(), WorkflowError> {
	let cargo = executable("CARGO", "cargo");
	let manifest = project.program_dir.join("tests/surfpool/Cargo.toml");
	let mut command = Command::new(&cargo);
	command
		.current_dir(&project.root)
		.env("PINA_SBF_ARTIFACT", artifact)
		.arg("test")
		.arg("--manifest-path")
		.arg(manifest)
		.arg("--lib");

	if let Some(filter) = filter {
		command.arg(filter);
	}

	command.arg("--").arg("--ignored").arg("--nocapture");
	run(&mut command, &cargo.to_string_lossy())
}

fn dev_arguments(
	artifacts_dir: &Path,
	runbook: &Path,
	network: &SurfpoolNetwork,
	accept_runbook_changes: bool,
) -> Vec<OsString> {
	let mut arguments = vec![
		OsString::from("start"),
		OsString::from("--watch"),
		OsString::from("--artifacts-path"),
		artifacts_dir.as_os_str().to_owned(),
		OsString::from("--manifest-file-path"),
		runbook.as_os_str().to_owned(),
	];

	if accept_runbook_changes {
		arguments.push(OsString::from("--yes"));
	}

	match network {
		SurfpoolNetwork::Offline => arguments.push(OsString::from("--offline")),
		SurfpoolNetwork::Cluster(network) => {
			arguments.push(OsString::from("--network"));
			arguments.push(OsString::from(network));
		}
		SurfpoolNetwork::RpcUrl(rpc_url) => {
			arguments.push(OsString::from("--rpc-url"));
			arguments.push(OsString::from(rpc_url));
		}
	}

	arguments
}

fn executable(variable: &str, fallback: &str) -> OsString {
	std::env::var_os(variable).unwrap_or_else(|| OsString::from(fallback))
}

fn run(command: &mut Command, program: &str) -> Result<(), WorkflowError> {
	let status = command.status().map_err(|source| {
		WorkflowError::RunCommand {
			program: program.to_owned(),
			source,
		}
	})?;

	if !status.success() {
		return Err(WorkflowError::CommandFailed {
			program: program.to_owned(),
			status,
		});
	}

	Ok(())
}

fn ensure_surfpool_version(surfpool: &OsStr) -> Result<(), WorkflowError> {
	let program = surfpool.to_string_lossy().into_owned();
	let mut child = Command::new(surfpool)
		.arg("--version")
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::null())
		.spawn()
		.map_err(|source| {
			WorkflowError::RunCommand {
				program: program.clone(),
				source,
			}
		})?;
	let stdout = take_version_stdout(&mut child, &program)?;
	let captured = capture_version_output(&mut child, stdout, &program)?;
	let status = child
		.wait()
		.map_err(|source| command_io_error(&program, source))?;

	if !status.success() {
		return Err(WorkflowError::CommandFailed { program, status });
	}

	let output = String::from_utf8_lossy(&captured.bytes);
	let output = output.trim();
	let diagnostic = sanitize_version_output(output, captured.truncated);
	let version = output
		.split_whitespace()
		.find_map(parse_version)
		.ok_or_else(|| {
			WorkflowError::InvalidSurfpoolVersion {
				output: diagnostic.clone(),
			}
		})?;

	let minimum = semver::Version::new(
		MINIMUM_SURFPOOL_VERSION.0,
		MINIMUM_SURFPOOL_VERSION.1,
		MINIMUM_SURFPOOL_VERSION.2,
	);

	if !version.pre.is_empty() || version < minimum {
		return Err(WorkflowError::SurfpoolTooOld { found: diagnostic });
	}

	Ok(())
}

struct CapturedVersionOutput {
	bytes: Vec<u8>,
	truncated: bool,
}

fn take_version_stdout(
	child: &mut std::process::Child,
	program: &str,
) -> Result<std::process::ChildStdout, WorkflowError> {
	let Some(stdout) = child.stdout.take() else {
		terminate_child(child);

		return Err(command_io_error(
			program,
			io::Error::other("Surfpool version stdout was not captured"),
		));
	};

	Ok(stdout)
}

fn capture_version_output(
	child: &mut std::process::Child,
	reader: impl Read,
	program: &str,
) -> Result<CapturedVersionOutput, WorkflowError> {
	match read_bounded_version_output(reader) {
		Ok(captured) => Ok(captured),
		Err(source) => {
			terminate_child(child);

			Err(command_io_error(program, source))
		}
	}
}

fn read_bounded_version_output(mut reader: impl Read) -> io::Result<CapturedVersionOutput> {
	let mut bytes = Vec::with_capacity(VERSION_OUTPUT_LIMIT);
	let mut buffer = [0_u8; 1024];
	let mut truncated = false;

	loop {
		let read = reader.read(&mut buffer)?;

		if read == 0 {
			break;
		}

		let remaining = VERSION_OUTPUT_LIMIT.saturating_sub(bytes.len());
		let retained = read.min(remaining);
		bytes.extend_from_slice(&buffer[..retained]);
		truncated |= retained < read;
	}

	Ok(CapturedVersionOutput { bytes, truncated })
}

fn sanitize_version_output(output: &str, truncated: bool) -> String {
	let mut sanitized = String::with_capacity(output.len());

	for character in output.chars() {
		if character.is_control() {
			sanitized.extend(character.escape_default());
		} else {
			sanitized.push(character);
		}
	}

	if truncated {
		sanitized.push_str("… [truncated]");
	}

	sanitized
}

fn terminate_child(child: &mut std::process::Child) {
	let _ = child.kill();
	let _ = child.wait();
}

fn command_io_error(program: &str, source: io::Error) -> WorkflowError {
	WorkflowError::RunCommand {
		program: program.to_owned(),
		source,
	}
}

fn parse_version(value: &str) -> Option<semver::Version> {
	let value = value.trim_start_matches('v');
	semver::Version::parse(value).ok()
}

#[cfg(test)]
mod tests {
	use std::collections::BTreeMap;
	use std::fs;
	#[cfg(unix)]
	use std::process::ExitStatus;

	use super::*;

	#[cfg(unix)]
	fn executable_script(directory: &Path, name: &str, body: &str) -> PathBuf {
		use std::os::unix::fs::symlink;

		let path = directory.join(name);
		let mut body_path = path.as_os_str().to_owned();
		body_path.push(".body");
		fs::write(PathBuf::from(body_path), format!("{body}\n")).expect("write test script body");
		let driver = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell-driver.sh");
		symlink(driver, &path).expect("link stable test shell driver");
		path
	}

	#[cfg(unix)]
	fn wait_for_nested_test(
		child: &mut std::process::Child,
		timeout: std::time::Duration,
	) -> io::Result<Option<ExitStatus>> {
		let deadline = std::time::Instant::now() + timeout;

		loop {
			if let Some(status) = child.try_wait()? {
				return Ok(Some(status));
			}

			if std::time::Instant::now() >= deadline {
				terminate_child(child);

				return Ok(None);
			}

			std::thread::sleep(std::time::Duration::from_millis(10));
		}
	}

	#[cfg(unix)]
	fn nested_profile_path(profile: Option<&OsStr>, fallback: &Path) -> PathBuf {
		let directory = profile
			.and_then(|profile| Path::new(profile).parent())
			.unwrap_or(fallback);

		directory.join("pina-nested-%p-%m.profraw")
	}

	#[cfg(unix)]
	struct FailingReader;

	#[cfg(unix)]
	impl Read for FailingReader {
		fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
			Err(io::Error::other("fixture read failure"))
		}
	}

	#[test]
	fn parses_surfpool_versions() {
		assert_eq!(parse_version("1.5.0"), Some(semver::Version::new(1, 5, 0)));
		assert_eq!(
			parse_version("v2.1.3-beta.1"),
			semver::Version::parse("2.1.3-beta.1").ok()
		);
		assert_eq!(parse_version("surfpool"), None);
	}

	#[test]
	fn bounds_and_sanitizes_version_diagnostics() {
		let mut output = b"\x1b[31munknown".to_vec();
		output.resize(VERSION_OUTPUT_LIMIT + 512, b'x');
		let captured = read_bounded_version_output(io::Cursor::new(output))
			.unwrap_or_else(|error| panic!("capture version output: {error}"));
		assert_eq!(captured.bytes.len(), VERSION_OUTPUT_LIMIT);
		assert!(captured.truncated);

		let output = String::from_utf8_lossy(&captured.bytes);
		let diagnostic = sanitize_version_output(output.trim(), captured.truncated);
		let error = WorkflowError::InvalidSurfpoolVersion { output: diagnostic };
		let message = error.to_string();
		assert!(!message.contains('\x1b'));
		assert!(message.contains("unknown"));
		assert!(message.contains("[truncated]"));
	}

	#[cfg(unix)]
	#[test]
	fn cleans_up_version_children_when_capture_fails() {
		let temporary = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("create temporary directory: {error}"));
		let sleeper = executable_script(temporary.path(), "sleeper", "while :; do :; done");
		let mut without_stdout = Command::new(&sleeper)
			.spawn()
			.unwrap_or_else(|error| panic!("spawn child without captured stdout: {error}"));
		assert!(matches!(
			take_version_stdout(&mut without_stdout, "fixture"),
			Err(WorkflowError::RunCommand { .. })
		));

		let mut unreadable = Command::new(&sleeper)
			.stdout(Stdio::piped())
			.spawn()
			.unwrap_or_else(|error| panic!("spawn child for read failure: {error}"));
		assert!(matches!(
			capture_version_output(&mut unreadable, FailingReader, "fixture"),
			Err(WorkflowError::RunCommand { .. })
		));
	}

	#[cfg(unix)]
	#[test]
	fn version_probe_closes_a_held_open_parent_stdin_pipe() {
		let temporary = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("create temporary directory: {error}"));
		let requires_eof = executable_script(
			temporary.path(),
			"requires-eof",
			"if read line; then echo 'surfpool unknown'; else echo 'surfpool 1.5.0'; fi",
		);
		let current_test = std::env::current_exe()
			.unwrap_or_else(|error| panic!("resolve current test executable: {error}"));
		let parent_profile = std::env::var_os("LLVM_PROFILE_FILE");
		let nested_profile = nested_profile_path(parent_profile.as_deref(), temporary.path());
		assert_eq!(
			nested_profile_path(None, temporary.path()),
			temporary.path().join("pina-nested-%p-%m.profraw")
		);
		assert_eq!(
			nested_profile_path(
				Some(Path::new("coverage/default.profraw").as_os_str()),
				temporary.path()
			),
			Path::new("coverage").join("pina-nested-%p-%m.profraw")
		);
		let mut nested_command = Command::new(current_test);
		nested_command
			.args([
				"--ignored",
				"--exact",
				"workflow::tests::version_probe_eof_sentinel",
			])
			.env("PINA_VERSION_EOF_FIXTURE", &requires_eof)
			.stdin(Stdio::piped())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.env("LLVM_PROFILE_FILE", nested_profile);

		let mut nested = nested_command
			.spawn()
			.unwrap_or_else(|error| panic!("spawn nested EOF test: {error}"));
		let held_open = nested
			.stdin
			.take()
			.unwrap_or_else(|| panic!("nested test stdin should be piped"));
		let status = wait_for_nested_test(&mut nested, std::time::Duration::from_secs(5))
			.unwrap_or_else(|error| panic!("wait for nested EOF test: {error}"));
		drop(held_open);
		assert!(matches!(status, Some(status) if status.success()));

		let blocker = executable_script(temporary.path(), "blocker", "while :; do :; done");
		let mut blocked = Command::new(blocker)
			.spawn()
			.unwrap_or_else(|error| panic!("spawn blocked timeout fixture: {error}"));
		let status = wait_for_nested_test(&mut blocked, std::time::Duration::from_millis(20))
			.unwrap_or_else(|error| panic!("wait for blocked timeout fixture: {error}"));
		assert!(status.is_none());
	}

	#[cfg(unix)]
	#[test]
	#[ignore = "sentinel launched with a deliberately held-open stdin pipe"]
	fn version_probe_eof_sentinel() {
		let fixture = std::env::var_os("PINA_VERSION_EOF_FIXTURE")
			.unwrap_or_else(|| panic!("PINA_VERSION_EOF_FIXTURE is required"));
		ensure_surfpool_version(fixture.as_os_str())
			.unwrap_or_else(|error| panic!("version probe should receive EOF: {error}"));
	}

	#[test]
	fn builds_platform_neutral_development_arguments() {
		let artifacts = Path::new("target directory/deploy");
		let runbook = Path::new("project directory/txtx.yml");
		let strings = |arguments: Vec<OsString>| {
			arguments
				.into_iter()
				.map(|argument| argument.to_string_lossy().into_owned())
				.collect::<Vec<_>>()
		};

		assert_eq!(
			strings(dev_arguments(
				artifacts,
				runbook,
				&SurfpoolNetwork::Offline,
				true,
			)),
			[
				"start",
				"--watch",
				"--artifacts-path",
				"target directory/deploy",
				"--manifest-file-path",
				"project directory/txtx.yml",
				"--yes",
				"--offline",
			]
		);
		assert!(
			strings(dev_arguments(
				artifacts,
				runbook,
				&SurfpoolNetwork::Cluster("devnet".to_owned()),
				false,
			))
			.ends_with(&["--network".to_owned(), "devnet".to_owned()])
		);
		assert!(
			strings(dev_arguments(
				artifacts,
				runbook,
				&SurfpoolNetwork::RpcUrl("https://rpc.example".to_owned()),
				false,
			))
			.ends_with(&["--rpc-url".to_owned(), "https://rpc.example".to_owned(),])
		);
	}

	#[test]
	fn surfpool_network_defaults_offline() {
		assert_eq!(SurfpoolNetwork::default(), SurfpoolNetwork::Offline);
	}

	#[test]
	fn validates_explicit_surfpool_rpc_endpoints_without_disclosing_them() {
		for endpoint in [
			"http://127.0.0.1:8899",
			"https://rpc.example",
			"https://rpc.example/solana",
			"https://[::1]:8899",
		] {
			validate_surfpool_network(&SurfpoolNetwork::RpcUrl(endpoint.to_owned()))
				.unwrap_or_else(|error| panic!("accept safe RPC endpoint {endpoint}: {error}"));
		}

		for endpoint in [
			"",
			"rpc.example",
			"ftp://rpc.example",
			"https:///missing-host",
			"https://agent:private-secret@rpc.example",
			"https://rpc.example/?token=private-secret",
			"https://rpc.example/#private-secret",
			"https://rpc.example/\u{1b}private-secret",
		] {
			let network = SurfpoolNetwork::RpcUrl(endpoint.to_owned());
			let error = validate_surfpool_network(&network)
				.expect_err("reject unsafe explicit Surfpool RPC endpoint");
			let display = error.to_string();
			let debug = format!("{error:?}");

			assert!(matches!(error, WorkflowError::UnsafeSurfpoolRpcUrl));
			assert!(!display.contains("private-secret"));
			assert!(!debug.contains("private-secret"));
		}

		let options = DevOptions {
			project: PathBuf::from("project-that-must-not-be-inspected"),
			network: SurfpoolNetwork::RpcUrl("https://agent:private-secret@rpc.example".to_owned()),
			accept_runbook_changes: true,
		};
		assert!(matches!(
			dev_project(&options),
			Err(WorkflowError::UnsafeSurfpoolRpcUrl)
		));
	}

	#[test]
	fn accepts_only_regular_surfpool_runbooks() {
		let temporary = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("create temporary directory: {error}"));
		let runbook = temporary.path().join("txtx.yml");

		assert!(
			!validate_runbook(&runbook)
				.unwrap_or_else(|error| panic!("inspect missing runbook: {error}"))
		);
		fs::write(&runbook, "runbook")
			.unwrap_or_else(|error| panic!("write regular runbook: {error}"));
		assert!(
			validate_runbook(&runbook)
				.unwrap_or_else(|error| panic!("inspect regular runbook: {error}"))
		);

		fs::remove_file(&runbook).unwrap_or_else(|error| panic!("remove regular runbook: {error}"));
		fs::create_dir(&runbook)
			.unwrap_or_else(|error| panic!("create runbook directory: {error}"));
		assert!(matches!(
			validate_runbook(&runbook),
			Err(WorkflowError::UnsafeSurfpoolRunbook { .. })
		));
	}

	#[test]
	fn reports_runbook_inspection_failures() {
		let runbook = Path::new("txtx.yml");
		let failure = io::Error::new(io::ErrorKind::PermissionDenied, "inspection denied");

		assert!(matches!(
			validate_runbook_metadata(runbook, Err(failure)),
			Err(WorkflowError::InspectSurfpoolRunbook { .. })
		));
	}

	#[cfg(unix)]
	#[test]
	fn rejects_symlinked_surfpool_runbooks() {
		use std::os::unix::fs::symlink;

		let temporary = tempfile::tempdir()
			.unwrap_or_else(|error| panic!("create temporary directory: {error}"));
		let target = temporary.path().join("outside.yml");
		let runbook = temporary.path().join("txtx.yml");
		fs::write(&target, "outside")
			.unwrap_or_else(|error| panic!("write symlink target: {error}"));
		symlink(&target, &runbook)
			.unwrap_or_else(|error| panic!("create runbook symlink: {error}"));

		assert!(matches!(
			validate_runbook(&runbook),
			Err(WorkflowError::UnsafeSurfpoolRunbook { .. })
		));
	}

	#[test]
	fn detects_windows_reparse_point_attributes() {
		assert!(windows_attributes_indicate_reparse_point(0x0400));
		assert!(windows_attributes_indicate_reparse_point(0x0401));
		assert!(!windows_attributes_indicate_reparse_point(0));
		assert!(!windows_attributes_indicate_reparse_point(0x0020));
	}

	#[cfg(unix)]
	#[test]
	fn reports_delegated_exit_codes() {
		use std::os::unix::process::ExitStatusExt;

		let ordinary = WorkflowError::RunCommand {
			program: "missing".to_owned(),
			source: io::Error::new(io::ErrorKind::NotFound, "missing"),
		};
		let exited = WorkflowError::CommandFailed {
			program: "fixture".to_owned(),
			status: ExitStatus::from_raw(23 << 8),
		};
		let signalled = WorkflowError::CommandFailed {
			program: "fixture".to_owned(),
			status: ExitStatus::from_raw(9),
		};

		assert_eq!(ordinary.exit_code(), 1);
		assert_eq!(exited.exit_code(), 23);
		assert_eq!(signalled.exit_code(), 137);
	}

	#[cfg(unix)]
	#[test]
	fn command_runner_reports_start_and_exit_failures() {
		let temporary = tempfile::tempdir().expect("create temporary directory");
		let mut failed_command = Command::new("/bin/sh");
		failed_command.args(["-c", "exit 17"]);
		let error = run(&mut failed_command, "failed").expect_err("command should fail");
		assert_eq!(error.exit_code(), 17);

		let missing = temporary.path().join("missing");
		let mut missing_command = Command::new(&missing);
		assert!(matches!(
			run(&mut missing_command, "missing"),
			Err(WorkflowError::RunCommand { .. })
		));
	}

	#[cfg(unix)]
	#[test]
	fn validates_surfpool_cli_versions() {
		let temporary = tempfile::tempdir().expect("create temporary directory");
		let current =
			executable_script(temporary.path(), "current", "echo 'surfpool 1.5.0'; exit 0");
		let old = executable_script(temporary.path(), "old", "echo 'surfpool 1.4.9'; exit 0");
		let prerelease = executable_script(
			temporary.path(),
			"prerelease",
			"echo 'surfpool 1.5.0-rc.1'; exit 0",
		);
		let next_major_prerelease = executable_script(
			temporary.path(),
			"next-major-prerelease",
			"echo 'surfpool 2.0.0-beta.1'; exit 0",
		);
		let invalid = executable_script(temporary.path(), "invalid", "echo 'surfpool unknown'");
		let failed = executable_script(temporary.path(), "failed-version", "exit 12");

		ensure_surfpool_version(current.as_os_str()).expect("accept current Surfpool");
		assert!(matches!(
			ensure_surfpool_version(old.as_os_str()),
			Err(WorkflowError::SurfpoolTooOld { .. })
		));
		assert!(matches!(
			ensure_surfpool_version(prerelease.as_os_str()),
			Err(WorkflowError::SurfpoolTooOld { .. })
		));
		assert!(matches!(
			ensure_surfpool_version(next_major_prerelease.as_os_str()),
			Err(WorkflowError::SurfpoolTooOld { .. })
		));
		assert!(matches!(
			ensure_surfpool_version(invalid.as_os_str()),
			Err(WorkflowError::InvalidSurfpoolVersion { .. })
		));
		assert!(matches!(
			ensure_surfpool_version(failed.as_os_str()),
			Err(WorkflowError::CommandFailed { .. })
		));
		assert!(matches!(
			ensure_surfpool_version(temporary.path().join("missing").as_os_str()),
			Err(WorkflowError::RunCommand { .. })
		));
	}

	#[cfg(unix)]
	#[test]
	fn requires_generated_surfpool_target() {
		let temporary = tempfile::tempdir().expect("create temporary directory");
		let root = temporary.path().to_path_buf();
		let project = Project {
			root: root.clone(),
			program_dir: root.clone(),
			package_name: "fixture".to_owned(),
			library_name: "fixture".to_owned(),
			library_source: root.join("src/lib.rs"),
			target_dir: root.join("target"),
			idl_dir: root.join("target/idl"),
			clients_dir: root.join("clients"),
			clients: Vec::new(),
			lint_levels: BTreeMap::new(),
		};
		assert!(matches!(
			require_surfpool_test(&project),
			Err(WorkflowError::MissingSurfpoolTest { .. })
		));
		fs::create_dir_all(project.program_dir.join("tests/surfpool/src"))
			.expect("create test directory");
		fs::write(project.program_dir.join("tests/surfpool/Cargo.toml"), "")
			.expect("write test manifest");
		assert!(matches!(
			require_surfpool_test(&project),
			Err(WorkflowError::MissingSurfpoolTest { .. })
		));
		fs::write(project.program_dir.join("tests/surfpool/src/lib.rs"), "")
			.expect("write test source");
		require_surfpool_test(&project).expect("accept Surfpool target");
	}
}
