//! CLI coverage for `pina rehearse` against loopback fakes.
//!
//! The remote cluster is a [`FakeRpcServer`] in this process. Surfpool is a
//! shell script that re-executes this test binary as `fake_surfnet_sentinel`,
//! which serves a [`FakeFork`] on the port `pina` chose, so the real binary
//! runs its whole pipeline (process start, readiness, hydration, program
//! swap, profiling, and reporting) without the SVM.

#![cfg(unix)]

#[path = "support/rehearse.rs"]
mod fakes;
mod support;

use std::fs;
use std::net::TcpListener;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::sync::Arc;

use fakes::Binary;
use fakes::FakeFork;
use fakes::FakeRpcServer;
use fakes::Fixtures;
use fakes::ForkFaults;
use fakes::Handler;
use fakes::remote_handler;
use fakes::serve_forever;

fn workspace_root() -> &'static Path {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(Path::parent)
		.unwrap_or_else(|| Path::new("."))
}

/// Serve a fake fork when the fake Surfpool launches this binary.
#[test]
#[ignore = "sentinel: the fake Surfpool runs it to serve a fork on its port"]
fn fake_surfnet_sentinel() {
	let Some(port) = std::env::var("PINA_FAKE_SURFNET_PORT")
		.ok()
		.and_then(|port| port.parse::<u16>().ok())
	else {
		return;
	};
	let listener = TcpListener::bind(("127.0.0.1", port))
		.unwrap_or_else(|error| panic!("bind the fake fork on {port}: {error}"));
	let fork = FakeFork::new(ForkFaults::default());
	let handler: Handler = Arc::new(move |method, params| fork.handle(method, params));

	serve_forever(&listener, &handler);
}

/// Scratch space with a fake Surfpool and candidate binaries.
struct Harness {
	directory: tempfile::TempDir,
	surfpool: PathBuf,
}

impl Harness {
	fn new() -> Self {
		let directory =
			tempfile::tempdir().unwrap_or_else(|error| panic!("create harness dir: {error}"));
		let surfpool = directory.path().join("surfpool");
		write_executable(
			&surfpool,
			r#"#!/bin/sh
if [ "$1" = "--version" ]; then
	echo "surfpool $PINA_FAKE_SURFPOOL_VERSION"
	exit 0
fi
port=""
previous=""
for argument in "$@"; do
	if [ "$previous" = "--port" ]; then
		port="$argument"
	fi
	previous="$argument"
done
PINA_FAKE_SURFNET_PORT="$port" exec "$PINA_FAKE_SURFNET_EXE" --ignored --exact fake_surfnet_sentinel --quiet
"#,
		);

		Self {
			directory,
			surfpool,
		}
	}

	fn candidate(&self, binary: Binary) -> PathBuf {
		let path = self.directory.path().join(format!("{binary:?}.so"));
		fs::write(&path, binary.elf()).unwrap_or_else(|error| panic!("write candidate: {error}"));
		path
	}

	fn pina(&self, version: &str) -> Command {
		let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
		command
			.arg("rehearse")
			.env("PINA_SURFPOOL", &self.surfpool)
			.env("PINA_FAKE_SURFPOOL_VERSION", version)
			.env(
				"PINA_FAKE_SURFNET_EXE",
				std::env::current_exe()
					.unwrap_or_else(|error| panic!("locate the test binary: {error}")),
			);
		command
	}

	/// Rehearse the counter example's captured traffic against `binary`.
	fn rehearse(&self, remote: &FakeRpcServer, binary: Binary, extra: &[&str]) -> Output {
		self.pina("1.6.0")
			.arg("--project")
			.arg(workspace_root().join("examples/counter_program"))
			.args(["--rpc-url", &remote.url])
			.arg("--program")
			.arg(self.candidate(binary))
			.args(extra)
			.output()
			.unwrap_or_else(|error| panic!("run pina rehearse: {error}"))
	}
}

/// Write a fake executable through the shared helper, which never opens it in
/// this process, so a concurrent `fork` cannot make its `exec` fail with
/// "Text file busy".
fn write_executable(path: &Path, contents: &str) {
	support::write_executable(path, contents)
		.unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}

fn stdout(output: &Output, remote: &FakeRpcServer) -> String {
	String::from_utf8_lossy(&output.stdout).replace(&remote.url, "http://127.0.0.1:[port]")
}

fn stderr(output: &Output) -> String {
	String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn rehearsing_the_deployed_binary_is_clean() {
	let harness = Harness::new();
	let remote = FakeRpcServer::start(remote_handler(&Fixtures::load()));
	let output = harness.rehearse(&remote, Binary::Deployed, &[]);

	assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
	assert!(stderr(&output).contains("Starting a Surfpool fork of http://127.0.0.1:"));
	insta::assert_snapshot!("rehearse_unchanged_text", stdout(&output, &remote));
}

#[test]
fn behaviour_changes_fail_with_a_stable_json_report() {
	let harness = Harness::new();
	let remote = FakeRpcServer::start(remote_handler(&Fixtures::load()));
	let output = harness.rehearse(&remote, Binary::Variant, &["--json"]);

	assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
	insta::assert_snapshot!("rehearse_state_changed_json", stdout(&output, &remote));

	let text = harness.rehearse(&remote, Binary::Variant, &[]);
	assert_eq!(text.status.code(), Some(2));
	insta::assert_snapshot!("rehearse_state_changed_text", stdout(&text, &remote));
}

#[test]
fn accepted_outcome_changes_exit_zero() {
	let harness = Harness::new();
	let fixtures = Fixtures::load();
	let remote = FakeRpcServer::start(remote_handler(&fixtures));
	let increment = fixtures.signatures()[0].clone();
	let output = harness.rehearse(
		&remote,
		Binary::Foreign,
		&["--allow-changes", "--signature", &increment],
	);

	assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
	insta::assert_snapshot!("rehearse_outcome_changed_text", stdout(&output, &remote));
	assert_eq!(remote.methods(), ["getTransaction"]);
}

#[test]
fn a_rehearsal_that_compares_nothing_exits_three() {
	let harness = Harness::new();
	let fixtures = Fixtures::load();
	let remote = FakeRpcServer::start(remote_handler(&fixtures));
	let initialize = fixtures.initialize_signature();
	let output = harness.rehearse(
		&remote,
		Binary::Variant,
		&["--signature", &initialize, "--allow-changes"],
	);
	let stdout = stdout(&output, &remote);

	assert_eq!(output.status.code(), Some(3), "{}", stderr(&output));
	assert!(stdout.contains("failed_in_both"), "{stdout}");
	assert!(
		stdout.contains("this rehearsal verified nothing about the upgrade"),
		"{stdout}"
	);
}

#[test]
fn operational_errors_exit_one_without_output() {
	let harness = Harness::new();
	let project = workspace_root().join("examples/counter_program");
	let secret = "private-rpc-credential";
	let cases: [(Vec<String>, &str, &str); 4] = [
		(
			vec![
				"--rpc-url".to_owned(),
				format!("https://agent:{secret}@rpc.example/?token={secret}"),
			],
			"1.6.0",
			"unsafe Surfpool RPC URL",
		),
		(
			vec![
				"--network".to_owned(),
				"devnet".to_owned(),
				"--signature".to_owned(),
				"not-a-signature".to_owned(),
			],
			"1.6.0",
			"invalid transaction signature \"not-a-signature\"",
		),
		(
			vec!["--network".to_owned(), "devnet".to_owned()],
			"1.5.0",
			"`pina rehearse` requires Surfpool 1.6.0 or newer",
		),
		(
			vec![
				"--network".to_owned(),
				"testnet".to_owned(),
				"--project".to_owned(),
				project.join("missing").to_string_lossy().into_owned(),
			],
			"1.6.0",
			"Could not",
		),
	];

	for (arguments, version, message) in cases {
		let output = harness
			.pina(version)
			.args(&arguments)
			.output()
			.unwrap_or_else(|error| panic!("run pina rehearse: {error}"));
		let stderr = stderr(&output);

		assert_eq!(output.status.code(), Some(1), "{arguments:?}: {stderr}");
		assert!(stderr.contains(message), "{arguments:?}: {stderr}");
		assert!(!stderr.contains(secret));
		assert_eq!(String::from_utf8_lossy(&output.stdout), "");
	}

	let missing_network = harness
		.pina("1.6.0")
		.output()
		.unwrap_or_else(|error| panic!("run pina rehearse: {error}"));
	assert_eq!(missing_network.status.code(), Some(2));
	assert!(stderr(&missing_network).contains("--network <CLUSTER>"));
}

#[test]
fn build_runs_the_project_build_before_rehearsing() {
	let harness = Harness::new();
	let remote = FakeRpcServer::start(remote_handler(&Fixtures::load()));
	let project = harness.directory.path().join("rehearse_build");
	pina_cli::init_project(&project, "rehearse_build", false)
		.unwrap_or_else(|error| panic!("create a project to build: {error}"));
	// Exercise the build wiring without the migration policy the scaffold
	// records, and keep the project out of the surrounding workspace.
	fs::write(project.join("pina.toml"), "[project]\nprogram = \".\"\n")
		.unwrap_or_else(|error| panic!("write pina.toml: {error}"));
	let manifest = project.join("Cargo.toml");
	let contents =
		fs::read_to_string(&manifest).unwrap_or_else(|error| panic!("read manifest: {error}"));
	fs::write(&manifest, format!("{contents}\n[workspace]\n"))
		.unwrap_or_else(|error| panic!("isolate manifest: {error}"));
	let cargo = harness.directory.path().join("cargo");
	write_executable(
		&cargo,
		r#"#!/bin/sh
case "$1" in
	metadata) exec cargo "$@" ;;
	build-sbf)
		mkdir -p "$CARGO_TARGET_DIR/sbf-build"
		printf '\177ELF built' > "$CARGO_TARGET_DIR/sbf-build/rehearse_build.so"
		;;
	*) exit 91 ;;
esac
"#,
	);
	let output = harness
		.pina("1.6.0")
		.arg("--project")
		.arg(&project)
		.args(["--rpc-url", &remote.url, "--build"])
		.env("CARGO", &cargo)
		.env("CARGO_TARGET_DIR", project.join("target"))
		.output()
		.unwrap_or_else(|error| panic!("run pina rehearse --build: {error}"));
	let stderr = stderr(&output);

	// The scaffold's program was never deployed to the captured cluster.
	assert_eq!(output.status.code(), Some(1), "{stderr}");
	assert!(
		stderr.contains("Building the candidate with `pina build`"),
		"{stderr}"
	);
	assert!(
		stderr.contains("does not exist on http://127.0.0.1:"),
		"{stderr}"
	);
	assert!(project.join("target/deploy/rehearse_build.so").is_file());
}
