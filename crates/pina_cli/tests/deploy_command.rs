//! End-to-end CLI coverage for deployment planning and safety gates.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
#[cfg(unix)]
use std::process::Stdio;
#[cfg(unix)]
use std::time::Duration;
#[cfg(unix)]
use std::time::Instant;

use ed25519_dalek::SigningKey;
use tempfile::TempDir;

struct ProjectFixture {
	_temp: TempDir,
	root: PathBuf,
	program: PathBuf,
	program_keypair: PathBuf,
	authority: PathBuf,
	payer: PathBuf,
}

impl ProjectFixture {
	fn new() -> Self {
		let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
		let root = temp.path().join("project");
		let deploy = root.join("target/deploy");
		let program = deploy.join("deploy_fixture.so");
		let program_keypair = deploy.join("deploy_fixture-keypair.json");
		let authority = root.join("authority.json");
		let payer = root.join("payer.json");
		fs::create_dir_all(root.join("src"))
			.unwrap_or_else(|error| panic!("create fixture source: {error}"));
		fs::create_dir_all(&deploy)
			.unwrap_or_else(|error| panic!("create fixture deploy directory: {error}"));
		fs::write(
			root.join("Cargo.toml"),
			"[package]\nname = \"deploy-fixture\"\nversion = \"0.1.0\"\n",
		)
		.unwrap_or_else(|error| panic!("write fixture manifest: {error}"));
		fs::write(&program, b"synthetic SBF")
			.unwrap_or_else(|error| panic!("write fixture artifact: {error}"));
		let program_id = write_keypair(&program_keypair, 3);
		write_keypair(&authority, 5);
		write_keypair(&payer, 7);
		fs::write(
			root.join("src/lib.rs"),
			format!("use pina::prelude::*;\ndeclare_id!(\"{program_id}\");\n"),
		)
		.unwrap_or_else(|error| panic!("write fixture source: {error}"));

		Self {
			_temp: temp,
			root,
			program,
			program_keypair,
			authority,
			payer,
		}
	}

	fn command(&self) -> Command {
		let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
		command
			.arg("deploy")
			.arg("--project")
			.arg(&self.root)
			.arg("--upgrade-authority")
			.arg(&self.authority)
			.arg("--payer")
			.arg(&self.payer);
		command
	}
}

fn write_keypair(path: &Path, seed: u8) -> String {
	let signing_key = SigningKey::from_bytes(&[seed; 32]);
	let public = signing_key.verifying_key().to_bytes();
	let mut bytes = signing_key.to_bytes().to_vec();
	bytes.extend_from_slice(&public);
	fs::write(
		path,
		serde_json::to_vec(&bytes)
			.unwrap_or_else(|error| panic!("serialize fixture keypair: {error}")),
	)
	.unwrap_or_else(|error| panic!("write fixture keypair: {error}"));
	make_keypair_private(path);
	bs58::encode(public).into_string()
}

#[cfg(unix)]
fn make_keypair_private(path: &Path) {
	use std::os::unix::fs::PermissionsExt as _;

	fs::set_permissions(path, fs::Permissions::from_mode(0o600))
		.unwrap_or_else(|error| panic!("protect fixture keypair: {error}"));
}

#[cfg(not(unix))]
fn make_keypair_private(_: &Path) {}

fn canonical(path: &Path) -> String {
	fs::canonicalize(path)
		.unwrap_or_else(|error| panic!("canonicalize {}: {error}", path.display()))
		.to_string_lossy()
		.into_owned()
}

#[test]
fn dry_run_json_is_machine_readable_and_starts_no_child() {
	let fixture = ProjectFixture::new();
	let output = fixture
		.command()
		.args([
			"--cluster",
			"https://rpc.example.com/solana",
			"--dry-run",
			"--json",
		])
		.output()
		.unwrap_or_else(|error| panic!("run dry deployment: {error}"));

	assert!(output.status.success());
	assert!(output.stderr.is_empty());
	let text = String::from_utf8_lossy(&output.stdout);
	let plan = serde_json::from_slice::<serde_json::Value>(&output.stdout)
		.unwrap_or_else(|error| panic!("parse deployment JSON: {error}"));
	assert_eq!(plan["cluster"], "custom");
	assert_eq!(plan["requires_mainnet_acknowledgement"], true);
	assert_eq!(plan["program"], canonical(&fixture.program));
	assert_eq!(plan["program_keypair"], canonical(&fixture.program_keypair));
	assert!(text.contains("https://rpc.example.com/solana"));
}

#[test]
fn rpc_query_values_are_rejected_without_leaking_them() {
	let fixture = ProjectFixture::new();
	let output = fixture
		.command()
		.args([
			"--cluster",
			"https://rpc.example.com/?token=sentinel-secret",
			"--dry-run",
		])
		.output()
		.unwrap_or_else(|error| panic!("run query deployment: {error}"));
	let stderr = String::from_utf8_lossy(&output.stderr);

	assert!(!output.status.success());
	assert!(stderr.contains("process arguments"));
	assert!(!stderr.contains("sentinel-secret"));
}

#[cfg(unix)]
#[test]
fn invalid_rpc_targets_fail_before_the_build_boundary() {
	use std::os::unix::fs::PermissionsExt as _;

	let fixture = ProjectFixture::new();
	let bin = fixture.root.join("invalid-target-fake-bin");
	let cargo = bin.join("cargo");
	let marker = fixture.root.join("unexpected-build.txt");
	fs::create_dir_all(&bin).unwrap_or_else(|error| panic!("create fake bin: {error}"));
	fs::write(
		&cargo,
		"#!/bin/sh\nset -eu\nprintf 'invoked\\n' > \"$PINA_DEPLOY_BUILD_MARKER\"\nexit 97\n",
	)
	.unwrap_or_else(|error| panic!("write fake cargo: {error}"));
	let mut permissions = fs::metadata(&cargo)
		.unwrap_or_else(|error| panic!("stat fake cargo: {error}"))
		.permissions();
	permissions.set_mode(0o755);
	fs::set_permissions(&cargo, permissions)
		.unwrap_or_else(|error| panic!("make fake cargo executable: {error}"));
	let existing_path = std::env::var_os("PATH").unwrap_or_default();
	let paths = std::iter::once(bin)
		.chain(std::env::split_paths(&existing_path))
		.collect::<Vec<_>>();
	let joined_path =
		std::env::join_paths(paths).unwrap_or_else(|error| panic!("construct test PATH: {error}"));

	for rpc_url in [
		"https://user:sentinel-secret@rpc.example.com",
		"https://rpc.example.com/?token=sentinel-secret",
		"https://rpc.example.com/#sentinel-secret",
		"https://rpc.example.com/\ncontrol",
	] {
		let output = fixture
			.command()
			.args(["--cluster", rpc_url, "--build"])
			.env("PATH", &joined_path)
			.env("PINA_DEPLOY_BUILD_MARKER", &marker)
			.output()
			.unwrap_or_else(|error| panic!("run invalid target deployment: {error}"));

		assert!(!output.status.success());
		assert!(
			!marker.exists(),
			"invalid target crossed the build boundary"
		);
		assert!(!String::from_utf8_lossy(&output.stderr).contains("sentinel-secret"));
	}
}

#[test]
fn clap_rejects_missing_target_and_mutating_dry_run_combinations() {
	let fixture = ProjectFixture::new();
	let missing_target = fixture
		.command()
		.output()
		.unwrap_or_else(|error| panic!("run missing-target command: {error}"));
	assert!(!missing_target.status.success());
	assert!(String::from_utf8_lossy(&missing_target.stderr).contains("--cluster"));

	for args in [
		["--cluster", "devnet", "--dry-run", "--build"],
		["--cluster", "devnet", "--dry-run", "--yes"],
		["--cluster", "devnet", "--json", "--yes"],
	] {
		let output = fixture
			.command()
			.args(args)
			.output()
			.unwrap_or_else(|error| panic!("run invalid deploy command: {error}"));
		assert!(!output.status.success());
	}
}

#[test]
fn non_tty_remote_deployment_fails_before_child_execution() {
	let fixture = ProjectFixture::new();
	let output = fixture
		.command()
		.args(["--cluster", "devnet"])
		.output()
		.unwrap_or_else(|error| panic!("run remote deployment: {error}"));

	assert!(!output.status.success());
	assert!(
		String::from_utf8_lossy(&output.stderr).contains("remote deployment was not confirmed")
	);
}

#[test]
fn every_named_remote_cluster_can_be_planned_without_execution() {
	let fixture = ProjectFixture::new();

	for cluster in ["testnet", "mainnet-beta"] {
		let output = fixture
			.command()
			.args(["--cluster", cluster, "--dry-run"])
			.output()
			.unwrap_or_else(|error| panic!("plan {cluster} deployment: {error}"));
		assert!(output.status.success());
		assert!(String::from_utf8_lossy(&output.stdout).contains(cluster));
	}
}

#[test]
fn deployment_resolution_errors_are_reported_without_starting_a_child() {
	let fixture = ProjectFixture::new();
	let output = fixture
		.command()
		.args([
			"--cluster",
			"localnet",
			"--program",
			"missing-program.so",
			"--dry-run",
		])
		.output()
		.unwrap_or_else(|error| panic!("run invalid deployment: {error}"));

	assert!(!output.status.success());
	assert!(String::from_utf8_lossy(&output.stderr).contains("program is not a readable"));
}

#[test]
fn build_failure_stops_before_final_planning() {
	let fixture = ProjectFixture::new();
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("deploy")
		.arg("--project")
		.arg(fixture.root.join("missing-project"))
		.arg("--upgrade-authority")
		.arg(&fixture.authority)
		.arg("--payer")
		.arg(&fixture.payer)
		.args(["--cluster", "localnet", "--build"])
		.output()
		.unwrap_or_else(|error| panic!("run failing build: {error}"));

	assert!(!output.status.success());
	assert!(String::from_utf8_lossy(&output.stderr).contains("deployment build failed"));
	assert!(!String::from_utf8_lossy(&output.stdout).contains("Deployment plan"));
}

#[cfg(unix)]
#[test]
fn local_deployment_passes_the_exact_modeled_arguments_to_solana() {
	use std::os::unix::fs::PermissionsExt;

	let fixture = ProjectFixture::new();
	let bin = fixture.root.join("fake-bin");
	let solana = bin.join("solana");
	let log = fixture.root.join("solana-args.txt");
	fs::create_dir_all(&bin).unwrap_or_else(|error| panic!("create fake bin: {error}"));
	fs::write(
		&solana,
		"#!/bin/sh\nset -eu\npwd > \"$PINA_DEPLOY_TEST_CWD\"\nprintf '%s\\n' \"$@\" > \
		 \"$PINA_DEPLOY_TEST_LOG\"\n",
	)
	.unwrap_or_else(|error| panic!("write fake solana: {error}"));
	let mut permissions = fs::metadata(&solana)
		.unwrap_or_else(|error| panic!("stat fake solana: {error}"))
		.permissions();
	permissions.set_mode(0o755);
	fs::set_permissions(&solana, permissions)
		.unwrap_or_else(|error| panic!("make fake solana executable: {error}"));
	let existing_path = std::env::var_os("PATH").unwrap_or_default();
	let paths = std::iter::once(bin.clone())
		.chain(std::env::split_paths(&existing_path))
		.collect::<Vec<_>>();
	let joined_path =
		std::env::join_paths(paths).unwrap_or_else(|error| panic!("construct test PATH: {error}"));
	let output = fixture
		.command()
		.args(["--cluster", "localnet"])
		.env("PATH", joined_path)
		.env("PINA_DEPLOY_TEST_CWD", fixture.root.join("solana-cwd.txt"))
		.env("PINA_DEPLOY_TEST_LOG", &log)
		.output()
		.unwrap_or_else(|error| panic!("run local deployment: {error}"));

	assert!(
		output.status.success(),
		"deployment failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let args = fs::read_to_string(&log)
		.unwrap_or_else(|error| panic!("read fake Solana arguments: {error}"));
	let expected = format!(
		"program\ndeploy\n{}\n--program-id\n{}\n--upgrade-authority\n{}\n--fee-payer\n{}\n--url\nhttp://127.0.0.1:8899\n",
		canonical(&fixture.program),
		canonical(&fixture.program_keypair),
		canonical(&fixture.authority),
		canonical(&fixture.payer),
	);
	assert_eq!(args, expected);
	let cwd = fs::read_to_string(fixture.root.join("solana-cwd.txt"))
		.unwrap_or_else(|error| panic!("read fake Solana working directory: {error}"));
	assert_eq!(cwd.trim(), canonical(&fixture.root));
}

#[cfg(unix)]
#[test]
fn solana_child_receives_eof_while_pina_stdin_remains_open() {
	use std::os::unix::fs::PermissionsExt as _;

	let fixture = ProjectFixture::new();
	let bin = fixture.root.join("stdin-fake-bin");
	let solana = bin.join("solana");
	let marker = fixture.root.join("solana-stdin-eof.txt");
	fs::create_dir_all(&bin).unwrap_or_else(|error| panic!("create fake bin: {error}"));
	fs::write(
		&solana,
		"#!/bin/sh\nset -eu\nif IFS= read -r value; then exit 41; fi\nprintf 'eof\\n' > \
		 \"$PINA_DEPLOY_STDIN_MARKER\"\n",
	)
	.unwrap_or_else(|error| panic!("write stdin-probing Solana: {error}"));
	let mut permissions = fs::metadata(&solana)
		.unwrap_or_else(|error| panic!("stat fake solana: {error}"))
		.permissions();
	permissions.set_mode(0o755);
	fs::set_permissions(&solana, permissions)
		.unwrap_or_else(|error| panic!("make fake solana executable: {error}"));
	let existing_path = std::env::var_os("PATH").unwrap_or_default();
	let paths = std::iter::once(bin)
		.chain(std::env::split_paths(&existing_path))
		.collect::<Vec<_>>();
	let joined_path =
		std::env::join_paths(paths).unwrap_or_else(|error| panic!("construct test PATH: {error}"));
	let mut child = fixture
		.command()
		.args(["--cluster", "localnet"])
		.env("PATH", joined_path)
		.env("PINA_DEPLOY_STDIN_MARKER", &marker)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.unwrap_or_else(|error| panic!("spawn deployment with held-open stdin: {error}"));
	let deadline = Instant::now() + Duration::from_secs(3);

	let timed_out = loop {
		if child
			.try_wait()
			.unwrap_or_else(|error| panic!("poll deployment: {error}"))
			.is_some()
		{
			break false;
		}

		if Instant::now() >= deadline {
			drop(child.stdin.take());
			break true;
		}

		std::thread::sleep(Duration::from_millis(10));
	};

	let output = child
		.wait_with_output()
		.unwrap_or_else(|error| panic!("collect deployment: {error}"));
	assert!(
		!timed_out,
		"Solana inherited Pina stdin and blocked; stderr: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(
		output.status.success(),
		"deployment failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert_eq!(
		fs::read_to_string(&marker)
			.unwrap_or_else(|error| panic!("read stdin EOF marker: {error}")),
		"eof\n"
	);
}
