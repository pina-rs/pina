//! CLI coverage for `pina rehearse` and `pina deploy --rehearse` against
//! loopback fakes.
//!
//! The remote cluster is a [`FakeRpcServer`] in this process. Surfpool is a
//! shell script that re-executes this test binary as `fake_surfnet_sentinel`,
//! which serves a [`FakeFork`] on the port `pina` chose, so the real binary
//! runs its whole pipeline (process start, readiness, hydration, program
//! swap, profiling, and reporting) without the SVM. Deployments run an
//! operator `--remote-command` that copies the artifact it was handed, so a
//! test sees exactly what would have been deployed, if anything.

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

use ed25519_dalek::SigningKey;
use fakes::Binary;
use fakes::FakeFork;
use fakes::FakeRpcServer;
use fakes::Fixtures;
use fakes::ForkFaults;
use fakes::Handler;
use fakes::Reply;
use fakes::remote_handler;
use fakes::remote_handler_for;
use fakes::serve_forever;
use serde_json::Value;
use serde_json::json;

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
	let fork = std::env::var("PINA_FAKE_SURFNET_PROGRAM_ID").map_or_else(
		|_| FakeFork::new(ForkFaults::default()),
		|program_id| FakeFork::for_program(&program_id, ForkFaults::default()),
	);
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
if [ -n "$PINA_FAKE_SURFPOOL_REPLACE" ]; then
	printf 'replaced while rehearsing' > "$PINA_FAKE_SURFPOOL_REPLACE"
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
		self.subcommand("rehearse", version)
	}

	/// `pina <subcommand>` with the fake Surfpool reporting `version`.
	fn subcommand(&self, subcommand: &str, version: &str) -> Command {
		let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
		command
			.arg(subcommand)
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

	/// A deployable project whose upgrade is `binary`.
	fn deploy_project(&self, name: &str, binary: Binary) -> DeployProject {
		DeployProject::new(&self.directory.path().join(name), binary)
	}

	/// `pina deploy --rehearse` of `project` to `remote`, deploying through a
	/// remote command that copies the artifact it receives to the project's
	/// marker.
	fn deploy(&self, project: &DeployProject, remote: &FakeRpcServer, extra: &[&str]) -> Output {
		self.deploy_command(project, remote, "1.6.0")
			.args(extra)
			.output()
			.unwrap_or_else(|error| panic!("run pina deploy --rehearse: {error}"))
	}

	fn deploy_command(
		&self,
		project: &DeployProject,
		remote: &FakeRpcServer,
		version: &str,
	) -> Command {
		let mut command = self.subcommand("deploy", version);
		command
			.arg("--project")
			.arg(&project.root)
			.arg("--program")
			.arg(&project.artifact)
			.arg("--program-keypair")
			.arg(&project.program_keypair)
			.arg("--upgrade-authority")
			.arg(&project.authority)
			.arg("--payer")
			.arg(&project.authority)
			.args(["--cluster", &remote.url, "--remote-command"])
			.arg(format!(
				"cp \"$PINA_DEPLOY_PROGRAM\" '{}'",
				project.deployed.display()
			))
			.env("PINA_FAKE_SURFNET_PROGRAM_ID", &project.program_id);
		command
	}
}

/// A project deployed at a seeded address, so its program keypair exists,
/// whose upgrade candidate is one of the captured binaries.
struct DeployProject {
	root: PathBuf,
	artifact: PathBuf,
	program_keypair: PathBuf,
	authority: PathBuf,
	program_id: String,
	/// Where the deployment's remote command copies the artifact it deploys.
	deployed: PathBuf,
}

impl DeployProject {
	fn new(root: &Path, binary: Binary) -> Self {
		fs::create_dir_all(root.join("src"))
			.unwrap_or_else(|error| panic!("create the deploy project: {error}"));
		fs::write(
			root.join("Cargo.toml"),
			"[package]\nname = \"deploy_rehearsal\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n",
		)
		.unwrap_or_else(|error| panic!("write the deploy manifest: {error}"));
		let artifact = root.join("deploy_rehearsal.so");
		fs::write(&artifact, binary.elf())
			.unwrap_or_else(|error| panic!("write the candidate: {error}"));
		let program_keypair = root.join("program-keypair.json");
		let authority = root.join("authority.json");
		let program_id = write_keypair(&program_keypair, 31);
		write_keypair(&authority, 32);
		fs::write(
			root.join("src/lib.rs"),
			format!("use pina::prelude::*;\ndeclare_id!(\"{program_id}\");\n"),
		)
		.unwrap_or_else(|error| panic!("write the deploy source: {error}"));

		Self {
			root: root.to_path_buf(),
			artifact,
			program_keypair,
			authority,
			program_id,
			deployed: root.join("deployed.so"),
		}
	}

	/// The bytes the remote command deployed, or `None` if it never ran.
	fn deployed(&self) -> Option<Vec<u8>> {
		fs::read(&self.deployed).ok()
	}

	/// A remote cluster where this project's program is deployed and the
	/// counter's captured traffic is its history.
	fn remote(&self) -> FakeRpcServer {
		FakeRpcServer::start(remote_handler_for(&Fixtures::load(), &self.program_id))
	}
}

/// Write an owner-only keypair from `seed` and return its address.
fn write_keypair(path: &Path, seed: u8) -> String {
	use std::os::unix::fs::PermissionsExt as _;

	let signing_key = SigningKey::from_bytes(&[seed; 32]);
	let public = signing_key.verifying_key().to_bytes();
	let mut bytes = signing_key.to_bytes().to_vec();
	bytes.extend_from_slice(&public);
	fs::write(path, Value::from(bytes).to_string())
		.unwrap_or_else(|error| panic!("write keypair: {error}"));
	fs::set_permissions(path, fs::Permissions::from_mode(0o600))
		.unwrap_or_else(|error| panic!("protect keypair: {error}"));
	bs58::encode(public).into_string()
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
	assert_eq!(remote.methods(), ["getAccountInfo", "getTransaction"]);
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

#[test]
fn deploy_rehearsal_stops_a_changed_upgrade_before_anything_is_sent() {
	let harness = Harness::new();
	let project = harness.deploy_project("changed", Binary::Variant);
	let remote = project.remote();
	let output = harness.deploy(&project, &remote, &["--rehearse"]);
	let stdout = stdout(&output, &remote);
	let stderr = stderr(&output);

	assert_eq!(output.status.code(), Some(2), "{stderr}");
	assert!(stdout.starts_with("Deployment plan\n"), "{stdout}");
	assert!(stdout.contains("state_changed"), "{stdout}");
	assert!(
		stdout.ends_with("rerun with --allow-rehearsal-changes to accept them.\n"),
		"{stdout}"
	);
	assert!(
		stderr.contains(
			"Deployment stopped before anything was sent: the rehearsal found behaviour changes"
		),
		"{stderr}"
	);
	assert!(stderr.contains("--allow-rehearsal-changes"), "{stderr}");
	assert_eq!(project.deployed(), None);
	assert_eq!(
		remote.requests()[1].1[1]["limit"],
		25,
		"the default limit matches `pina rehearse`"
	);

	let accepted = harness.deploy(
		&project,
		&remote,
		&["--rehearse", "--allow-rehearsal-changes"],
	);

	assert_eq!(
		accepted.status.code(),
		Some(0),
		"{}",
		String::from_utf8_lossy(&accepted.stderr)
	);
	assert!(String::from_utf8_lossy(&accepted.stdout).contains("Deployment complete"));
	assert_eq!(project.deployed(), Some(Binary::Variant.elf()));
}

#[test]
fn deploy_rehearsal_deploys_the_exact_rehearsed_bytes_when_nothing_changed() {
	let harness = Harness::new();
	let project = harness.deploy_project("unchanged", Binary::Deployed);
	let remote = project.remote();
	let output = harness.deploy(&project, &remote, &["--rehearse"]);
	let stdout = stdout(&output, &remote);
	let stderr = stderr(&output);

	assert_eq!(output.status.code(), Some(0), "{stderr}");
	assert!(
		stderr.contains("Starting a Surfpool fork of http://127.0.0.1:"),
		"{stderr}"
	);
	assert!(stdout.starts_with("Deployment plan\n"), "{stdout}");
	assert!(stdout.contains("\nRehearsal of "), "{stdout}");
	assert!(stdout.ends_with("Deployment complete\n"), "{stdout}");
	assert_eq!(project.deployed(), Some(Binary::Deployed.elf()));
}

#[test]
fn deploy_rehearsal_that_compares_nothing_never_deploys() {
	let harness = Harness::new();
	let project = harness.deploy_project("nothing", Binary::Variant);
	let fixtures = Fixtures::load();
	let initialize = fixtures.initialize_signature();
	let only_initialize: Vec<Value> = fixtures
		.signatures
		.as_array()
		.into_iter()
		.flatten()
		.filter(|entry| entry["signature"] == initialize.as_str())
		.cloned()
		.collect();
	let history = remote_handler_for(&fixtures, &project.program_id);
	let remote = FakeRpcServer::start(move |method, params| {
		if method == "getSignaturesForAddress" {
			Reply::Result(json!(only_initialize))
		} else {
			history(method, params)
		}
	});
	let output = harness.deploy(
		&project,
		&remote,
		&[
			"--rehearse",
			"--rehearse-limit",
			"1",
			"--allow-rehearsal-changes",
		],
	);
	let stderr = stderr(&output);

	assert_eq!(output.status.code(), Some(3), "{stderr}");
	assert!(
		stderr.contains("the rehearsal compared no transaction, so the upgrade is unverified"),
		"{stderr}"
	);
	assert!(stderr.contains("--rehearse-limit"), "{stderr}");
	assert_eq!(project.deployed(), None);
	assert_eq!(remote.requests()[1].1[1]["limit"], 1);
}

#[test]
fn deploy_rehearsal_cannot_verify_a_first_deployment() {
	let harness = Harness::new();
	let project = harness.deploy_project("first", Binary::Deployed);
	// The captured cluster has the counter program, not this one.
	let remote = FakeRpcServer::start(remote_handler(&Fixtures::load()));
	let output = harness.deploy(&project, &remote, &["--rehearse"]);
	let stdout = stdout(&output, &remote);
	let stderr = stderr(&output);

	assert_eq!(output.status.code(), Some(3), "{stderr}");
	assert!(
		stderr.contains(&format!(
			"program {} is not deployed on http://127.0.0.1:",
			project.program_id
		)),
		"{stderr}"
	);
	assert!(
		stderr.contains("deploy it the first time without --rehearse"),
		"{stderr}"
	);
	assert!(!stderr.contains("Starting a Surfpool fork"), "{stderr}");
	assert!(stdout.starts_with("Deployment plan\n"), "{stdout}");
	assert!(!stdout.contains("Rehearsal of "), "{stdout}");
	assert_eq!(remote.methods(), ["getAccountInfo"]);
	assert_eq!(project.deployed(), None);
}

#[test]
fn deploy_rehearsal_dry_runs_print_one_json_document() {
	let harness = Harness::new();
	let project = harness.deploy_project("json", Binary::Deployed);
	let remote = project.remote();
	let output = harness.deploy(&project, &remote, &["--rehearse", "--dry-run", "--json"]);
	let document: Value = serde_json::from_slice(&output.stdout)
		.unwrap_or_else(|error| panic!("one JSON document on stdout: {error}"));

	assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
	assert_eq!(document["program_id"], project.program_id.as_str());
	assert_eq!(document["cluster"], "custom");
	assert_eq!(document["rehearsal"]["schemaVersion"], 1);
	assert_eq!(
		document["rehearsal"]["programId"],
		project.program_id.as_str()
	);
	assert_eq!(document["rehearsal"]["cluster"], remote.url.as_str());
	assert_eq!(document["rehearsal"]["summary"]["outcomeChanged"], 0);
	assert_eq!(document["rehearsal"]["summary"]["stateChanged"], 0);
	assert_eq!(project.deployed(), None);

	let changed = harness.deploy_project("json-changed", Binary::Variant);
	let remote = changed.remote();
	let output = harness.deploy(&changed, &remote, &["--rehearse", "--dry-run", "--json"]);
	let document: Value = serde_json::from_slice(&output.stdout)
		.unwrap_or_else(|error| panic!("one JSON document on stdout: {error}"));

	assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
	assert_ne!(document["rehearsal"]["summary"]["stateChanged"], 0);
	assert_eq!(changed.deployed(), None);
}

#[test]
fn deploy_rehearsal_never_deploys_a_candidate_replaced_while_rehearsing() {
	let harness = Harness::new();
	let project = harness.deploy_project("replaced", Binary::Deployed);
	let remote = project.remote();
	let output = harness
		.deploy_command(&project, &remote, "1.6.0")
		.arg("--rehearse")
		.env("PINA_FAKE_SURFPOOL_REPLACE", &project.artifact)
		.output()
		.unwrap_or_else(|error| panic!("run pina deploy --rehearse: {error}"));
	let stdout = stdout(&output, &remote);
	let stderr = stderr(&output);

	// The rehearsal replays the bytes the plan pinned, so it passes; the
	// replacement is caught before the deployment runs.
	assert_eq!(output.status.code(), Some(1), "{stderr}");
	assert!(stdout.contains("Rehearsal of "), "{stdout}");
	assert!(
		stderr.contains("deployment inputs changed after planning"),
		"{stderr}"
	);
	assert_eq!(project.deployed(), None);
}

#[test]
fn deploy_rehearsal_failures_and_flag_misuse_never_deploy() {
	let harness = Harness::new();
	let project = harness.deploy_project("failures", Binary::Deployed);
	let remote = project.remote();
	let old_surfpool = harness
		.deploy_command(&project, &remote, "1.5.0")
		.arg("--rehearse")
		.output()
		.unwrap_or_else(|error| panic!("run pina deploy --rehearse: {error}"));
	let stderr_text = stderr(&old_surfpool);

	assert_eq!(old_surfpool.status.code(), Some(1), "{stderr_text}");
	assert!(
		stderr_text.contains("the rehearsal failed: Surfpool 1.5.0 is too old"),
		"{stderr_text}"
	);

	for arguments in [
		&["--rehearse-limit", "5"][..],
		&["--allow-rehearsal-changes"],
		&["--rehearse", "--rehearse-limit", "0"],
		&["--rehearse", "--rehearse-limit", "1001"],
	] {
		let output = harness.deploy(&project, &remote, arguments);
		let stderr = stderr(&output);

		assert_eq!(output.status.code(), Some(2), "{arguments:?}: {stderr}");
		assert!(stderr.contains("--rehearse"), "{arguments:?}: {stderr}");
	}

	assert_eq!(project.deployed(), None);
}
