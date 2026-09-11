//! End-to-end CLI coverage for migration lifecycle output.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use sha2::Digest as _;
use sha2::Sha256;
use tempfile::TempDir;

const PROGRAM_ID: &str = "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS";

struct MigrationFixture {
	_temporary: TempDir,
	root: PathBuf,
	artifact: PathBuf,
}

impl MigrationFixture {
	fn new(migratable: bool) -> Self {
		let temporary = TempDir::new().unwrap_or_else(|error| panic!("create fixture: {error}"));
		let root = fs::canonicalize(temporary.path())
			.unwrap_or_else(|error| panic!("canonicalize fixture: {error}"));
		fs::create_dir_all(root.join("src"))
			.unwrap_or_else(|error| panic!("create fixture source: {error}"));
		fs::write(
			root.join("Cargo.toml"),
			"[package]\nname = \"migration-command-fixture\"\nversion = \"0.0.0\"\nedition = \
			 \"2024\"\n[lib]\npath = \"src/lib.rs\"\n",
		)
		.unwrap_or_else(|error| panic!("write fixture manifest: {error}"));
		fs::write(
			root.join("pina.toml"),
			"[project]\nprogram = \".\"\n\n[migrations]\nversion-type = \"u8\"\n",
		)
		.unwrap_or_else(|error| panic!("write fixture config: {error}"));
		let source = if migratable {
			format!(
				"use pina::*;\ndeclare_id!(\"{PROGRAM_ID}\");\n#[discriminator]\nenum Kind {{ \
				 State = 1 }}\n#[account(discriminator = Kind::State, migrations)]\nstruct State \
				 {{ value: u64 }}\n"
			)
		} else {
			format!("use pina::*;\ndeclare_id!(\"{PROGRAM_ID}\");\n")
		};
		fs::write(root.join("src/lib.rs"), source)
			.unwrap_or_else(|error| panic!("write fixture source: {error}"));
		let artifact = root.join("program.so");
		fs::write(&artifact, b"migration artifact")
			.unwrap_or_else(|error| panic!("write fixture artifact: {error}"));

		Self {
			_temporary: temporary,
			root,
			artifact,
		}
	}

	fn command(&self, operation: &str) -> Command {
		let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
		command
			.arg("migrations")
			.arg(operation)
			.arg("--project")
			.arg(&self.root);
		command
	}

	fn publish(&self, record: bool) {
		let digest: [u8; 32] = Sha256::digest(b"migration artifact").into();
		pina_cli::migrations::begin_publication(
			&self.root,
			"devnet",
			"https://api.devnet.solana.com",
			PROGRAM_ID,
			&self.artifact,
			digest,
		)
		.unwrap_or_else(|error| panic!("begin publication: {error}"));
		if record {
			pina_cli::migrations::record_publication(
				&self.root,
				"devnet",
				"https://api.devnet.solana.com",
				PROGRAM_ID,
				&self.artifact,
				digest,
			)
			.unwrap_or_else(|error| panic!("record publication: {error}"));
		}
	}

	fn write_fields(&self, fields: &str) {
		fs::write(
			self.root.join("src/lib.rs"),
			format!(
				"use pina::*;\ndeclare_id!(\"{PROGRAM_ID}\");\n#[discriminator]\nenum Kind {{ \
				 State = 1 }}\n#[account(discriminator = Kind::State, migrations)]\nstruct State \
				 {{ {fields} }}\n"
			),
		)
		.unwrap_or_else(|error| panic!("update fixture source: {error}"));
	}
}

fn run(command: &mut Command) -> String {
	let output = command
		.output()
		.unwrap_or_else(|error| panic!("run migration command: {error}"));
	assert!(
		output.status.success(),
		"migration command failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	String::from_utf8(output.stdout)
		.unwrap_or_else(|error| panic!("migration output was not UTF-8: {error}"))
}

/// Run a command that must fail, returning its stdout and stderr streams.
fn run_failure(command: &mut Command) -> (String, String) {
	let output = command
		.output()
		.unwrap_or_else(|error| panic!("run migration command: {error}"));
	assert!(
		!output.status.success(),
		"migration command unexpectedly succeeded: {}",
		String::from_utf8_lossy(&output.stdout)
	);
	let stdout = String::from_utf8(output.stdout)
		.unwrap_or_else(|error| panic!("migration stdout was not UTF-8: {error}"));
	let stderr = String::from_utf8(output.stderr)
		.unwrap_or_else(|error| panic!("migration stderr was not UTF-8: {error}"));
	(stdout, stderr)
}

#[test]
fn migration_commands_report_draft_pending_published_and_updated_states() {
	let fixture = MigrationFixture::new(true);
	let made = run(&mut fixture.command("make"));
	assert!(made.contains("Created account:1:01@0"));

	let draft = run(&mut fixture.command("status"));
	assert!(draft.contains("account State v0 (draft)"));
	assert!(draft.contains("Migration history is consistent"));

	let json = run(fixture.command("check").arg("--json"));
	let statuses: serde_json::Value = serde_json::from_str(&json)
		.unwrap_or_else(|error| panic!("parse migration status JSON: {error}"));
	assert_eq!(statuses[0]["currentVersion"], 0);

	fixture.publish(false);
	let pending = run(&mut fixture.command("status"));
	assert!(pending.contains("publication pending"));
	fixture.publish(true);
	let published = run(&mut fixture.command("check"));
	assert!(published.contains("published"));

	fixture.write_fields("value: u32");
	let advanced = run(&mut fixture.command("make"));
	assert!(advanced.contains("Advanced account:1:01@1"));
	assert!(advanced.contains("Manual migration required"));
	let identity = pina_abi::ContractIdentity::try_new(pina_abi::ContractKind::Account, 1, 1)
		.unwrap_or_else(|error| panic!("create fixture identity: {error}"));
	let transition = fixture
		.root
		.join(pina_abi::transition_path(&identity, 0, 1));
	fs::write(&transition, "pub(crate) fn migrate(_: &mut [u8]) {}\n")
		.unwrap_or_else(|error| panic!("complete migration: {error}"));
	let refreshed = run(fixture.command("make").arg("--json"));
	assert!(refreshed.contains("updatedDrafts"));

	fixture.write_fields("value: u16");
	let updated = run(&mut fixture.command("make"));
	assert!(updated.contains("Updated draft account:1:01@1"));
}

#[test]
fn make_reports_answer_and_question_failures_for_agents() {
	let fixture = MigrationFixture::new(true);
	let made = run(&mut fixture.command("make"));
	assert!(made.contains("Created account:1:01@0"));

	// A malformed answer flag fails at parse time before any project work.
	let malformed = run_failure(fixture.command("make").arg("--rename").arg("no-separator"));
	assert!(malformed.1.contains("Error"), "stderr: {}", malformed.1);

	// A published version cannot change silently: the ambiguous rename fails
	// closed, printing questions on stdout under --json for agents.
	fixture.publish(true);
	fixture.write_fields("points: u64");
	let questions = run_failure(
		fixture
			.command("make")
			.arg("--no-interactive")
			.arg("--json"),
	);
	assert!(questions.0.contains("\"from\""), "stdout: {}", questions.0);
	assert!(questions.0.contains("value"), "stdout: {}", questions.0);
	assert!(
		questions.1.contains("--rename value:points"),
		"stderr: {}",
		questions.1
	);

	// Without --json the questions stay on stderr only.
	let plain = run_failure(fixture.command("make").arg("--no-interactive"));
	assert!(plain.0.is_empty(), "stdout: {}", plain.0);
	assert!(
		plain.1.contains("ambiguous field changes"),
		"stderr: {}",
		plain.1
	);

	// Any other failure goes through the generic error arm.
	let corrupt = MigrationFixture::new(true);
	run(&mut corrupt.command("make"));
	fs::write(corrupt.root.join("migrations/manifest.json"), b"{ corrupt")
		.unwrap_or_else(|error| panic!("corrupt manifest: {error}"));
	let broken = run_failure(&mut corrupt.command("make"));
	assert!(broken.1.contains("Error"), "stderr: {}", broken.1);
}

#[test]
fn make_prints_rent_warnings_for_growing_transitions() {
	let fixture = MigrationFixture::new(true);
	run(&mut fixture.command("make"));
	fixture.publish(true);
	fixture.write_fields("value: u64, enabled: bool");
	let grown = run(&mut fixture.command("make"));
	assert!(
		grown.contains("grows from 10 to 11 bytes"),
		"stdout: {grown}"
	);
	assert!(grown.contains("lamport budget"), "stdout: {grown}");
}

#[test]
fn reconcile_reports_clear_pending_and_abandoned_states() {
	// With a recorded history but no pending publication, reconcile reports
	// nothing to do.
	let fixture = MigrationFixture::new(true);
	run(&mut fixture.command("make"));
	let clear = run(&mut fixture.command("reconcile"));
	assert!(clear.contains("No pending deployment."), "stdout: {clear}");

	// The JSON variant serializes the same report for tooling.
	let clear_json = run(fixture.command("reconcile").arg("--json"));
	let report: serde_json::Value = serde_json::from_str(&clear_json)
		.unwrap_or_else(|error| panic!("parse reconcile JSON: {error}"));
	assert_eq!(report["no_pending"], true);

	// A begun-but-unrecorded deployment reports its identity and how to
	// resolve it.
	fixture.publish(false);
	let pending = run(&mut fixture.command("reconcile"));
	assert!(
		pending.contains("Pending deployment for"),
		"stdout: {pending}"
	);
	assert!(
		pending.contains("Planned executable digest:"),
		"stdout: {pending}"
	);
	assert!(
		pending.contains("Rerun the exact same devnet deployment"),
		"stdout: {pending}"
	);

	// Abandoning freezes the shipped versions either way.
	let abandoned_fixture = MigrationFixture::new(true);
	run(&mut abandoned_fixture.command("make"));
	abandoned_fixture.publish(false);
	let abandoned = run(&mut abandoned_fixture.command("reconcile").arg("--abandon"));
	assert!(
		abandoned.contains("recorded as abandoned"),
		"stdout: {abandoned}"
	);
	assert!(abandoned.contains("versions stay frozen"));
}

#[test]
fn status_reports_projects_without_migration_aware_contracts() {
	let fixture = MigrationFixture::new(false);
	let output = run(&mut fixture.command("status"));
	assert!(output.contains("No migration-aware contracts."));

	let json = run(fixture.command("status").arg("--json"));
	assert_eq!(json.trim(), "[]");
}
