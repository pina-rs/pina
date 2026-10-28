//! End-to-end CLI coverage for deployment planning and safety gates.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
#[cfg(unix)]
use std::process::Stdio;

use ed25519_dalek::SigningKey;
use tempfile::TempDir;

#[cfg(unix)]
mod support;

#[cfg(unix)]
#[path = "support/rehearse.rs"]
mod rpc_support;

struct ProjectFixture {
	_temp: TempDir,
	root: PathBuf,
	program: PathBuf,
	program_keypair: PathBuf,
	program_id: String,
	authority: PathBuf,
	payer: PathBuf,
	rpc_url: String,
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
			program_id,
			authority,
			payer,
			rpc_url: "http://127.0.0.1:1".to_owned(),
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

	fn enable_migrations(&self) {
		fs::write(self.root.join("pina.toml"), "[project]\nprogram = \".\"\n")
			.unwrap_or_else(|error| panic!("write migration config: {error}"));
		fs::write(
			self.root.join("src/lib.rs"),
			format!(
				"use pina::*;\ndeclare_id!(\"{}\");\n#[discriminator]\nenum Kind {{ State = 1 \
				 }}\n#[account(discriminator = Kind::State, migrations)]\nstruct State {{ value: \
				 u64 }}\n",
				self.program_id
			),
		)
		.unwrap_or_else(|error| panic!("write migration source: {error}"));
		pina_cli::migrations::create_migrations(&self.root)
			.unwrap_or_else(|error| panic!("create migration snapshot: {error}"));
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
	// The synthetic artifact is not an ELF file, which the plan points out.
	let warnings = plan["warnings"].as_array().expect("plan warnings");
	assert_eq!(warnings.len(), 1, "{warnings:?}");
	assert!(
		warnings[0]
			.as_str()
			.is_some_and(|warning| warning.contains("not an ELF"))
	);

	// Naming the program keypair as its own upgrade authority is legal but
	// hands upgrade rights to whoever holds the program keypair.
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("deploy")
		.arg("--project")
		.arg(&fixture.root)
		.arg("--upgrade-authority")
		.arg(&fixture.program_keypair)
		.arg("--payer")
		.arg(&fixture.payer)
		.args(["--cluster", "localnet", "--dry-run"])
		.output()
		.unwrap_or_else(|error| panic!("run self-authority dry deployment: {error}"));
	assert!(output.status.success());
	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(stdout.contains("Warning:"), "stdout: {stdout}");
	assert!(
		stdout.contains("also the upgrade authority"),
		"stdout: {stdout}"
	);
}

#[test]
fn upgrade_only_rejects_a_declared_keypair_path_before_planning_or_execution() {
	let fixture = ProjectFixture::new();
	let marker = fixture.root.join("unexpected-deploy.txt");
	let marker_command = "echo started > unexpected-deploy.txt";
	let output = fixture
		.command()
		.args([
			"--cluster",
			"localnet",
			"--upgrade-only",
			"--remote-command",
			marker_command,
		])
		.output()
		.unwrap();
	assert!(
		output.status.success(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert_eq!(fs::read_to_string(&marker).unwrap().trim(), "started");
	fs::remove_file(&marker).unwrap();

	fs::write(
		fixture.root.join("src/lib.rs"),
		r#"declare_id!("payer.json");"#,
	)
	.unwrap();

	for dry_run in [true, false] {
		let mut command = fixture.command();
		command.args(["--cluster", "localnet", "--upgrade-only"]);

		if dry_run {
			command.args(["--dry-run", "--json"]);
		} else {
			command.args(["--remote-command", marker_command]);
		}

		let output = command.output().unwrap();
		let stderr = String::from_utf8_lossy(&output.stderr);
		assert!(!output.status.success());
		assert!(
			stderr.contains("base58-encoded 32-byte Solana address"),
			"{stderr}"
		);
		assert!(output.stdout.is_empty());
		assert!(!marker.exists(), "deployment child must not run");
	}
}

#[test]
fn upgrade_only_plans_by_declared_id_and_conflicts_with_a_program_keypair() {
	let fixture = ProjectFixture::new();
	fs::remove_file(&fixture.program_keypair)
		.unwrap_or_else(|error| panic!("remove program keypair: {error}"));
	let output = fixture
		.command()
		.args([
			"--cluster",
			"devnet",
			"--upgrade-only",
			"--dry-run",
			"--json",
		])
		.output()
		.unwrap_or_else(|error| panic!("run upgrade-only dry deployment: {error}"));

	assert!(
		output.status.success(),
		"stderr: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let plan = serde_json::from_slice::<serde_json::Value>(&output.stdout)
		.unwrap_or_else(|error| panic!("parse deployment JSON: {error}"));
	assert_eq!(plan["program_keypair"], serde_json::Value::Null);
	assert_eq!(plan["program_id"], fixture.program_id);
	assert_eq!(plan["commands"][0]["args"][4], fixture.program_id);

	let output = fixture
		.command()
		.arg("--program-keypair")
		.arg(&fixture.authority)
		.args(["--cluster", "devnet", "--upgrade-only", "--dry-run"])
		.output()
		.unwrap_or_else(|error| panic!("run conflicting upgrade-only deployment: {error}"));
	assert!(!output.status.success());
	assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used with"));
}

#[cfg(unix)]
#[test]
fn upgrade_only_execution_passes_the_declared_id_to_solana() {
	let fixture = ProjectFixture::new();
	fs::remove_file(&fixture.program_keypair)
		.unwrap_or_else(|error| panic!("remove program keypair: {error}"));
	let bin = fixture.root.join("upgrade-fake-bin");
	let log = fixture.root.join("upgrade-args.txt");
	fs::create_dir_all(&bin).unwrap_or_else(|error| panic!("create fake bin: {error}"));
	support::write_executable(
		&bin.join("solana"),
		"#!/bin/sh\nset -eu\nprintf '%s\\n' \"$@\" > \"$PINA_DEPLOY_TEST_LOG\"\n",
	)
	.unwrap();
	let existing_path = std::env::var_os("PATH").unwrap_or_default();
	let joined_path =
		std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&existing_path)))
			.unwrap_or_else(|error| panic!("construct test PATH: {error}"));
	let output = fixture
		.command()
		.args(["--cluster", "localnet", "--upgrade-only"])
		.env("PATH", joined_path)
		.env("PINA_DEPLOY_TEST_LOG", &log)
		.output()
		.unwrap_or_else(|error| panic!("run upgrade-only deployment: {error}"));

	assert!(
		output.status.success(),
		"deployment failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let args = fs::read_to_string(&log)
		.unwrap_or_else(|error| panic!("read fake Solana arguments: {error}"));
	let args = args.lines().collect::<Vec<_>>();
	assert_eq!(
		[args[3], args[4]],
		["--program-id", fixture.program_id.as_str()]
	);
	assert!(
		String::from_utf8_lossy(&output.stdout).contains("none (upgrade by declared program ID)")
	);
}

#[test]
fn override_command_replaces_ambient_deployment_environment() {
	let fixture = ProjectFixture::new();
	let log = fixture.root.join("deployment-environment.txt");
	let reserved = [
		"PINA_DEPLOY_PROGRAM",
		"PINA_DEPLOY_PROGRAM_ID",
		"PINA_DEPLOY_PROGRAM_KEYPAIR",
		"PINA_DEPLOY_UPGRADE_AUTHORITY",
		"PINA_DEPLOY_PAYER",
		"PINA_DEPLOY_RPC_URL",
		"PINA_DEPLOY_CLUSTER",
	];
	#[cfg(unix)]
	let script = r#"printf '%s\n' "$PINA_DEPLOY_PROGRAM" "$PINA_DEPLOY_PROGRAM_ID" "${PINA_DEPLOY_PROGRAM_KEYPAIR-unset}" "$PINA_DEPLOY_UPGRADE_AUTHORITY" "$PINA_DEPLOY_PAYER" "$PINA_DEPLOY_RPC_URL" "$PINA_DEPLOY_CLUSTER" "$PINA_DEPLOY_TEST_SENTINEL" > "$PINA_DEPLOY_TEST_LOG""#;
	#[cfg(not(unix))]
	let script = r#"(echo %PINA_DEPLOY_PROGRAM%& echo %PINA_DEPLOY_PROGRAM_ID%& (if defined PINA_DEPLOY_PROGRAM_KEYPAIR (echo %PINA_DEPLOY_PROGRAM_KEYPAIR%) else (echo unset))& echo %PINA_DEPLOY_UPGRADE_AUTHORITY%& echo %PINA_DEPLOY_PAYER%& echo %PINA_DEPLOY_RPC_URL%& echo %PINA_DEPLOY_CLUSTER%& echo %PINA_DEPLOY_TEST_SENTINEL%)> "%PINA_DEPLOY_TEST_LOG%""#;

	for upgrade_only in [true, false] {
		let mut command = fixture.command();
		command
			.args(["--cluster", "devnet", "--yes", "--remote-command", script])
			.envs(reserved.iter().map(|key| (key, "stale-parent-value")))
			.env("PINA_DEPLOY_TEST_SENTINEL", "preserved-parent-value")
			.env("PINA_DEPLOY_TEST_LOG", &log);

		if upgrade_only {
			command.arg("--upgrade-only");
		}

		let output = command
			.output()
			.unwrap_or_else(|error| panic!("run deployment environment probe: {error}"));
		assert!(
			output.status.success(),
			"deployment failed: {}",
			String::from_utf8_lossy(&output.stderr)
		);
		let captured = fs::read_to_string(&log)
			.unwrap_or_else(|error| panic!("read deployment environment: {error}"));
		let values = captured.lines().collect::<Vec<_>>();
		assert_eq!(values.len(), 8, "captured environment: {captured}");
		assert_eq!(values[1], fixture.program_id);
		assert_eq!(values[5], "https://api.devnet.solana.com");
		assert_eq!(values[6], "devnet");
		assert_eq!(values[7], "preserved-parent-value");

		for (value, name) in [
			(values[0], "program.so"),
			(values[3], "upgrade-authority.json"),
			(values[4], "payer.json"),
		] {
			assert!(Path::new(value).ends_with(name), "{value}");
		}

		if upgrade_only {
			assert_eq!(values[2], "unset");
		} else {
			assert!(Path::new(values[2]).ends_with("program-keypair.json"));
			assert_ne!(values[2], canonical(&fixture.program_keypair));
		}
	}
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
	let fixture = ProjectFixture::new();
	let bin = fixture.root.join("invalid-target-fake-bin");
	let cargo = bin.join("cargo");
	let marker = fixture.root.join("unexpected-build.txt");
	fs::create_dir_all(&bin).unwrap_or_else(|error| panic!("create fake bin: {error}"));
	support::write_executable(
		&cargo,
		"#!/bin/sh\nset -eu\nprintf 'invoked\\n' > \"$PINA_DEPLOY_BUILD_MARKER\"\nexit 97\n",
	)
	.unwrap();
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

	// A mistyped cluster name names the accepted spellings instead of a URL
	// parser error.
	for cluster in ["mainnet", "Localnet", "localhost"] {
		let output = fixture
			.command()
			.args(["--cluster", cluster, "--build"])
			.env("PATH", &joined_path)
			.env("PINA_DEPLOY_BUILD_MARKER", &marker)
			.output()
			.unwrap_or_else(|error| panic!("run mistyped cluster deployment: {error}"));
		let stderr = String::from_utf8_lossy(&output.stderr);
		assert!(!output.status.success());
		assert!(
			!marker.exists(),
			"a mistyped cluster crossed the build boundary"
		);
		assert!(
			stderr.contains("expected a named cluster"),
			"stderr: {stderr}"
		);
		assert!(stderr.contains("mainnet-beta"), "stderr: {stderr}");
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
fn local_deployment_passes_verified_snapshots_and_modeled_arguments_to_solana() {
	let fixture = ProjectFixture::new();
	let bin = fixture.root.join("fake-bin");
	let solana = bin.join("solana");
	let log = fixture.root.join("solana-args.txt");
	fs::create_dir_all(&bin).unwrap_or_else(|error| panic!("create fake bin: {error}"));
	support::write_executable(
		&solana,
		"#!/bin/sh\nset -eu\npwd > \"$PINA_DEPLOY_TEST_CWD\"\nprintf '%s\\n' \"$@\" > \
		 \"$PINA_DEPLOY_TEST_LOG\"\ncat \"$3\" > \"$PINA_DEPLOY_TEST_PROGRAM\"\ncat \"$5\" > \
		 \"$PINA_DEPLOY_TEST_PROGRAM_KEYPAIR\"\ncat \"$7\" > \"$PINA_DEPLOY_TEST_AUTHORITY\"\ncat \
		 \"$9\" > \"$PINA_DEPLOY_TEST_PAYER\"\n",
	)
	.unwrap();
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
		.env("PINA_DEPLOY_TEST_PROGRAM", fixture.root.join("executed.so"))
		.env(
			"PINA_DEPLOY_TEST_PROGRAM_KEYPAIR",
			fixture.root.join("executed-program-keypair.json"),
		)
		.env(
			"PINA_DEPLOY_TEST_AUTHORITY",
			fixture.root.join("executed-authority.json"),
		)
		.env(
			"PINA_DEPLOY_TEST_PAYER",
			fixture.root.join("executed-payer.json"),
		)
		.output()
		.unwrap_or_else(|error| panic!("run local deployment: {error}"));

	assert!(
		output.status.success(),
		"deployment failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let args = fs::read_to_string(&log)
		.unwrap_or_else(|error| panic!("read fake Solana arguments: {error}"));
	let args = args.lines().collect::<Vec<_>>();
	assert_eq!(
		[
			args[0], args[1], args[3], args[5], args[7], args[9], args[10]
		],
		[
			"program",
			"deploy",
			"--program-id",
			"--upgrade-authority",
			"--fee-payer",
			"--url",
			"http://127.0.0.1:8899",
		]
	);
	// The fee payer doubles as the default signer, so no Solana CLI config is read.
	assert_eq!([args[11], args[12]], ["--keypair", args[8]]);
	assert_eq!([args[13], args[14]], ["--commitment", "finalized"]);
	for (actual, original, name) in [
		(args[2], &fixture.program, "program.so"),
		(args[4], &fixture.program_keypair, "program-keypair.json"),
		(args[6], &fixture.authority, "upgrade-authority.json"),
		(args[8], &fixture.payer, "payer.json"),
	] {
		assert_ne!(actual, canonical(original));
		assert!(actual.ends_with(name));
		assert!(!Path::new(actual).exists());
	}
	for (executed, original) in [
		(fixture.root.join("executed.so"), &fixture.program),
		(
			fixture.root.join("executed-program-keypair.json"),
			&fixture.program_keypair,
		),
		(
			fixture.root.join("executed-authority.json"),
			&fixture.authority,
		),
		(fixture.root.join("executed-payer.json"), &fixture.payer),
	] {
		assert_eq!(
			fs::read(executed).unwrap_or_else(|error| panic!("read executed input: {error}")),
			fs::read(original).unwrap_or_else(|error| panic!("read original input: {error}")),
		);
	}
	let cwd = fs::read_to_string(fixture.root.join("solana-cwd.txt"))
		.unwrap_or_else(|error| panic!("read fake Solana working directory: {error}"));
	assert_eq!(cwd.trim(), canonical(&fixture.root));
}

/// A deployment subprocess receives EOF while the parent CLI's stdin stays open.
#[cfg(unix)]
#[test]
fn solana_child_receives_eof_while_pina_stdin_remains_open() {
	let fixture = ProjectFixture::new();
	let bin = fixture.root.join("stdin-fake-bin");
	let solana = bin.join("solana");
	let marker = fixture.root.join("solana-stdin-eof.txt");
	fs::create_dir_all(&bin).unwrap_or_else(|error| panic!("create fake bin: {error}"));
	support::write_executable(
		&solana,
		"#!/usr/bin/env bash\nset -eu\nif IFS= read -r -t 5 value; then\n  exit 41\nelse\n  status=$?\n  \
		 [ \"$status\" -eq 1 ] || exit \"$status\"\nfi\nprintf 'eof\\n' > \
		 \"$PINA_DEPLOY_STDIN_MARKER\"\n",
	)
	.unwrap();
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
		// An unread plan-output pipe can fill before the Solana child starts.
		.stdout(Stdio::null())
		.stderr(Stdio::piped())
		.spawn()
		.unwrap_or_else(|error| panic!("spawn deployment with held-open stdin: {error}"));
	// Bound the fake child's read, independently of Cargo discovery and CLI
	// startup. Keep stdin open outside wait_with_output, which closes the
	// handle in child.stdin while draining stdout and stderr concurrently.
	let held_stdin = child.stdin.take().unwrap();
	let output = child
		.wait_with_output()
		.unwrap_or_else(|error| panic!("collect deployment: {error}"));
	drop(held_stdin);

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

#[cfg(unix)]
fn remote_command(fixture: &ProjectFixture, name: &str, script: &str) -> Command {
	let bin = fixture.root.join(name);
	let solana = bin.join("solana");
	fs::create_dir_all(&bin).unwrap_or_else(|error| panic!("create fake bin: {error}"));
	support::write_executable(&solana, script).unwrap();
	let existing_path = std::env::var_os("PATH").unwrap_or_default();
	let paths = std::iter::once(bin)
		.chain(std::env::split_paths(&existing_path))
		.collect::<Vec<_>>();
	let joined_path =
		std::env::join_paths(paths).unwrap_or_else(|error| panic!("construct test PATH: {error}"));
	let mut command = fixture.command();
	command
		.args([
			"--cluster",
			&fixture.rpc_url,
			"--record-publication",
			"--yes",
		])
		.env("PATH", joined_path);
	command
}

#[cfg(unix)]
#[test]
fn remote_deployments_record_and_retain_crash_safe_migration_state() {
	let mut successful = ProjectFixture::new();
	successful.enable_migrations();
	let server = publication_rpc(&successful);
	successful.rpc_url = server.url.clone();
	let output = remote_command(&successful, "success-bin", "#!/bin/sh\nset -eu\nexit 0\n")
		.output()
		.unwrap_or_else(|error| panic!("run successful remote deployment: {error}"));
	assert!(
		output.status.success(),
		"remote deployment failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let ledger = pina_abi::decode_publication_ledger(
		&fs::read(successful.root.join("migrations/publications.json"))
			.unwrap_or_else(|error| panic!("read successful publication: {error}")),
	)
	.unwrap_or_else(|error| panic!("decode successful publication: {error}"));
	assert!(ledger.pending.is_none());
	assert_eq!(ledger.receipts.len(), 1);

	let failed = ProjectFixture::new();
	failed.enable_migrations();
	let output = remote_command(&failed, "failure-bin", "#!/bin/sh\nset -eu\nexit 73\n")
		.output()
		.unwrap_or_else(|error| panic!("run failed remote deployment: {error}"));
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(!output.status.success());
	assert!(stderr.contains("recoverable pending publication was retained"));
	let ledger = pina_abi::decode_publication_ledger(
		&fs::read(failed.root.join("migrations/publications.json"))
			.unwrap_or_else(|error| panic!("read retained publication: {error}")),
	)
	.unwrap_or_else(|error| panic!("decode retained publication: {error}"));
	assert!(ledger.pending.is_some());
}

#[cfg(unix)]
#[test]
fn a_deploy_program_that_never_starts_discards_its_fresh_pending_publication() {
	let fixture = ProjectFixture::new();
	fixture.enable_migrations();
	// An empty PATH guarantees `solana` cannot be spawned, so nothing can have
	// reached the cluster and the draft versions must stay editable.
	let empty_bin = fixture.root.join("empty-bin");
	fs::create_dir_all(&empty_bin).unwrap_or_else(|error| panic!("create empty bin: {error}"));
	let output = fixture
		.command()
		.args(["--cluster", "devnet", "--yes"])
		.env("PATH", &empty_bin)
		// Project discovery still needs Cargo, so name it by absolute path.
		.env("CARGO", env!("CARGO"))
		.output()
		.unwrap_or_else(|error| panic!("run unstartable deployment: {error}"));
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(!output.status.success());
	assert!(stderr.contains("failed to start"), "stderr: {stderr}");
	assert!(
		stderr.contains("pending migration publication was discarded"),
		"stderr: {stderr}"
	);
	let ledger = pina_abi::decode_publication_ledger(
		&fs::read(fixture.root.join("migrations/publications.json"))
			.unwrap_or_else(|error| panic!("read discarded publication: {error}")),
	)
	.unwrap_or_else(|error| panic!("decode discarded publication: {error}"));
	assert!(ledger.pending.is_none());
	assert!(ledger.receipts.is_empty());
}

#[cfg(unix)]
#[test]
fn remote_deployment_detects_post_plan_artifact_and_history_tampering() {
	let artifact_swap = ProjectFixture::new();
	artifact_swap.enable_migrations();
	let output = remote_command(
		&artifact_swap,
		"artifact-swap-bin",
		"#!/bin/sh\nset -eu\nprintf tampered >> \"$PINA_DEPLOY_MUTATE_PATH\"\n",
	)
	.env("PINA_DEPLOY_MUTATE_PATH", &artifact_swap.program)
	.output()
	.unwrap_or_else(|error| panic!("run artifact-swap deployment: {error}"));
	assert!(!output.status.success());
	assert!(String::from_utf8_lossy(&output.stderr).contains("inputs changed"));

	let history_swap = ProjectFixture::new();
	history_swap.enable_migrations();
	let manifest = history_swap.root.join("migrations/manifest.json");
	let output = remote_command(
		&history_swap,
		"history-swap-bin",
		"#!/bin/sh\nset -eu\nrm \"$PINA_DEPLOY_MUTATE_PATH\"\n",
	)
	.env("PINA_DEPLOY_MUTATE_PATH", &manifest)
	.output()
	.unwrap_or_else(|error| panic!("run history-swap deployment: {error}"));
	assert!(!output.status.success());
	assert!(String::from_utf8_lossy(&output.stderr).contains("inputs changed"));
}

#[cfg(unix)]
#[test]
fn conflicting_pending_publication_stops_before_remote_execution() {
	use sha2::Digest as _;

	let fixture = ProjectFixture::new();
	fixture.enable_migrations();
	let digest: [u8; 32] = sha2::Sha256::digest(b"synthetic SBF").into();
	let program = fs::canonicalize(&fixture.program)
		.unwrap_or_else(|error| panic!("canonicalize fixture program: {error}"));
	pina_cli::migrations::begin_publication(
		&fixture.root,
		"testnet",
		"https://api.testnet.solana.com",
		&fixture.program_id,
		&program,
		digest,
	)
	.unwrap_or_else(|error| panic!("reserve conflicting publication: {error}"));
	let marker = fixture.root.join("unexpected-solana-execution");
	let output = remote_command(
		&fixture,
		"pending-conflict-bin",
		"#!/bin/sh\nset -eu\nprintf invoked > \"$PINA_DEPLOY_MARKER\"\n",
	)
	.env("PINA_DEPLOY_MARKER", &marker)
	.output()
	.unwrap_or_else(|error| panic!("run conflicting deployment: {error}"));
	assert!(!output.status.success());
	assert!(
		String::from_utf8_lossy(&output.stderr).contains("Could not reserve migration publication")
	);
	assert!(!marker.exists());
}

/// SEC-12: an exit-zero child cannot finalize a receipt without RPC evidence.
#[cfg(unix)]
#[test]
fn audit_sec_12_an_exit_zero_remote_command_is_not_deployment_evidence() {
	let fixture = ProjectFixture::new();
	fixture.enable_migrations();
	let output = remote_command(&fixture, "audit-noop-bin", "#!/bin/sh\nset -eu\nexit 0\n")
		.args(["--remote-command", "exit 0"])
		.output()
		.unwrap_or_else(|error| panic!("run no-op remote deployment: {error}"));

	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(
		stderr.contains("pending") && !output.status.success(),
		"a no-op deployment child must not report a completed deployment: {stderr}"
	);

	let ledger = pina_abi::decode_publication_ledger(
		&fs::read(fixture.root.join("migrations/publications.json"))
			.unwrap_or_else(|error| panic!("read publication ledger: {error}")),
	)
	.unwrap_or_else(|error| panic!("decode publication ledger: {error}"));

	assert!(
		ledger.pending.is_some() && ledger.receipts.is_empty(),
		"SEC-12: an exit-zero no-op child produced a finalized deployment receipt without any \
		 on-chain proof"
	);
}

/// Serve the approved loader state at finalized commitment, including spare
/// zero-filled allocation beyond the exact approved artifact.
#[cfg(unix)]
fn publication_rpc(fixture: &ProjectFixture) -> rpc_support::FakeRpcServer {
	use std::str::FromStr as _;

	use base64::Engine as _;
	use serde_json::json;
	use solana_address::Address;

	let loader = "BPFLoaderUpgradeab1e11111111111111111111111";
	let program = Address::from_str(&fixture.program_id).unwrap();
	let (programdata, _) =
		Address::find_program_address(&[program.as_ref()], &Address::from_str_const(loader));
	let mut program_bytes = 2_u32.to_le_bytes().to_vec();
	program_bytes.extend_from_slice(programdata.as_ref());
	let mut data_bytes = 3_u32.to_le_bytes().to_vec();
	data_bytes.extend_from_slice(&41_u64.to_le_bytes());
	data_bytes.push(1);
	data_bytes.extend_from_slice(&SigningKey::from_bytes(&[5; 32]).verifying_key().to_bytes());
	data_bytes.extend_from_slice(b"synthetic SBF\0\0");
	let account = |data: Vec<u8>, executable| {
		json!({
			"owner": loader, "executable": executable, "lamports": 1,
			"data": [base64::engine::general_purpose::STANDARD.encode(data), "base64"]
		})
	};
	let accounts = json!({ "context": { "slot": 42 }, "value": [
		account(program_bytes, true), account(data_bytes, false)
	] });
	let program_id = fixture.program_id.clone();
	rpc_support::FakeRpcServer::start(move |method, params| {
		match method {
			"getGenesisHash" => {
				rpc_support::Reply::Result(json!("11111111111111111111111111111111"))
			}
			"getMultipleAccounts" => {
				assert_eq!(params[0], json!([program_id, programdata.to_string()]));
				assert_eq!(
					params[1],
					json!({ "encoding": "base64", "commitment": "finalized" })
				);
				rpc_support::Reply::Result(accounts.clone())
			}
			_ => panic!("unexpected RPC method {method}"),
		}
	})
}
