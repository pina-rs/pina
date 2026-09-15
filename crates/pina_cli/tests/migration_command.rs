//! End-to-end CLI coverage for migration lifecycle output.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use object::Architecture;
use object::BinaryFormat;
use object::Endianness;
use object::SectionKind;
use object::SymbolFlags;
use object::SymbolKind;
use object::SymbolScope;
use object::write::Object;
use object::write::Symbol;
use object::write::SymbolSection;
use sha2::Digest as _;
use sha2::Sha256;
use tempfile::TempDir;

const PROGRAM_ID: &str = "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS";

/// A migration-aware program with one account and one instruction process.
///
/// The account slot is named `state`, so the cost preview links
/// `UpdateInstruction` to the `State` account contract.
fn migratable_program_source(account_fields: &str, instruction_fields: &str) -> String {
	format!(
		r#"use pina::*;

declare_id!("{PROGRAM_ID}");

#[discriminator]
enum AccountKind {{
	State = 1,
}}

#[discriminator]
enum InstructionKind {{
	Update = 2,
}}

#[account(discriminator = AccountKind::State, migrations)]
struct State {{ {account_fields} }}

#[instruction(discriminator = InstructionKind::Update, migrations)]
struct UpdateInstruction {{ {instruction_fields} }}

#[derive(Accounts)]
struct UpdateAccounts<'a> {{
	#[pina(validate(signer))]
	authority: &'a AccountView,
	state: Option<&'a mut AccountView>,
	#[pina(validate(signer))]
	migration_payer: Option<&'a mut AccountView>,
	system_program: Option<&'a AccountView>,
}}

impl<'a> ProcessAccountInfos<'a> for UpdateAccounts<'a> {{
	fn process(self, _data: &[u8]) -> ProgramResult {{
		Ok(())
	}}
}}

pub struct FixtureProgram;

impl CpiProgramId for FixtureProgram {{
	const ID: Address = ID;
}}

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {{
	use super::*;

	nostd_entrypoint!(process_instruction);

	pub fn process_instruction(
		program_id: &Address,
		accounts: &mut [AccountView],
		data: &[u8],
	) -> ProgramResult {{
		let instruction: InstructionKind = parse_instruction(program_id, &ID, data)?;
		match instruction {{
			InstructionKind::Update => {{
				UpdateAccounts::try_from((program_id, accounts))?.process(data)
			}}
		}}
	}}
}}
"#
	)
}

/// Build a minimal SBF ELF whose named symbols carry the profile's CU estimate.
///
/// Each 8-byte `.text` unit costs one CU, so a symbol's size is its estimate.
fn build_sbf_elf(symbols: &[(&str, u64, u64)]) -> Vec<u8> {
	let mut object = Object::new(BinaryFormat::Elf, Architecture::Sbf, Endianness::Little);
	let section = object.add_section(Vec::new(), b".text".to_vec(), SectionKind::Text);
	let text_size = symbols
		.iter()
		.map(|(_, offset, size)| offset.saturating_add(*size))
		.max()
		.unwrap_or(0);
	object.set_section_data(section, vec![0_u8; text_size as usize], 8);

	for &(name, offset, size) in symbols {
		object.add_symbol(Symbol {
			name: name.as_bytes().to_vec(),
			value: offset,
			size,
			kind: SymbolKind::Text,
			scope: SymbolScope::Dynamic,
			weak: false,
			section: SymbolSection::Section(section),
			flags: SymbolFlags::None,
		});
	}

	object
		.write()
		.unwrap_or_else(|error| panic!("failed to write ELF: {error}"))
}

/// The artifact path `pina migrations status` profiles for a fixture.
///
/// The fixture pins `CARGO_TARGET_DIR` to its own `target` directory, so this
/// never collides with an ambient target directory such as llvm-cov's.
fn sbf_artifact_path(root: &Path) -> PathBuf {
	root.join("target")
		.join("deploy")
		.join("migration_command_fixture.so")
}

/// Write bytes to the artifact path `pina migrations status` profiles.
fn write_sbf_artifact(root: &Path, bytes: &[u8]) -> PathBuf {
	let path = sbf_artifact_path(root);
	fs::create_dir_all(path.parent().expect("artifact path has a parent"))
		.unwrap_or_else(|error| panic!("create artifact directory: {error}"));
	fs::write(&path, bytes).unwrap_or_else(|error| panic!("write fixture artifact: {error}"));
	path
}

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
			 \"2024\"\n[lib]\nname = \"migration_command_fixture\"\npath = \"src/lib.rs\"\n",
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
			.env("CARGO_TARGET_DIR", self.root.join("target"))
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

	fn write_source(&self, source: &str) {
		fs::write(self.root.join("src/lib.rs"), source)
			.unwrap_or_else(|error| panic!("update fixture source: {error}"));
	}

	/// Replace `[migrations]` with an explicit auto policy.
	fn configure_auto(&self, auto: &str) {
		fs::write(
			self.root.join("pina.toml"),
			format!("[project]\nprogram = \".\"\n\n[migrations]\nversion-type = \"u8\"\n{auto}"),
		)
		.unwrap_or_else(|error| panic!("write auto config: {error}"));
	}

	fn build_script(&self) -> PathBuf {
		self.root.join("build.rs")
	}

	fn manifest(&self) -> serde_json::Value {
		let source = fs::read_to_string(self.root.join("migrations/manifest.json"))
			.unwrap_or_else(|error| panic!("read manifest: {error}"));
		serde_json::from_str(&source).unwrap_or_else(|error| panic!("parse manifest: {error}"))
	}
}

/// A program with one declaration of each kind and no per-item tokens.
const AUTO_SOURCE: &str = r#"use pina::*;
declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");
#[discriminator]
pub enum Kind { State = 1 }
#[discriminator]
pub enum Instructions { Update = 2 }
#[discriminator]
pub enum Events { Changed = 3 }
#[account(discriminator = Kind::State)]
pub struct State { value: u64 }
#[instruction(discriminator = Instructions::Update)]
pub struct UpdateInstruction { value: u64 }
#[event(discriminator = Events::Changed)]
pub struct ChangedEvent { value: u64 }
pub fn process_instruction(
	program_id: &Address,
	accounts: &mut [AccountView],
	data: &[u8],
) -> ProgramResult {
	let _ = (program_id, accounts, data);
	Ok(())
}
"#;

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
	// The cost preview closes the output even with nothing to estimate.
	assert!(output.contains("Cost preview"), "stdout: {output}");
	assert!(
		output.contains("Most expensive touching transaction: unavailable"),
		"stdout: {output}"
	);

	let json = run(fixture.command("status").arg("--json"));
	let report: serde_json::Value =
		serde_json::from_str(&json).unwrap_or_else(|error| panic!("parse status JSON: {error}"));
	assert_eq!(report["statuses"], serde_json::json!([]));
	assert_eq!(
		report["costPreview"]["mostExpensive"]["status"],
		"unavailable"
	);
}

/// Advance the fixture to an account v0->v1 and instruction v0->v1 history,
/// then write a synthetic SBF artifact whose transition functions carry 10 CU
/// (State) and 20 CU (UpdateInstruction).
fn advanced_cost_fixture() -> (MigrationFixture, PathBuf) {
	let fixture = MigrationFixture::new(true);
	fixture.write_source(&migratable_program_source("value: u64", "value: u64"));
	run(&mut fixture.command("make"));
	fixture.publish(true);
	fixture.write_source(&migratable_program_source(
		"value: u64, enabled: bool",
		"value: u64, memo: u16",
	));
	let advanced = run(&mut fixture.command("make"));
	assert!(
		advanced.contains("Advanced account:1:01@1"),
		"stdout: {advanced}"
	);
	assert!(
		advanced.contains("Advanced instruction:1:02@1"),
		"stdout: {advanced}"
	);
	let artifact = write_sbf_artifact(
		&fixture.root,
		&build_sbf_elf(&[
			(
				"_ZN8my_crate31__pina_state_account_migrations8v0_to_v17migrate17h0000E",
				0,
				80,
			),
			(
				"_ZN9my_crate48__pina_update_instruction_instruction_migrations8v0_to_v17migrate17h1111E",
				80,
				160,
			),
		]),
	);

	(fixture, artifact)
}

#[test]
fn status_previews_costs_and_json_agrees_with_human_output() {
	let (fixture, artifact) = advanced_cost_fixture();
	let human = run(&mut fixture.command("status"));
	let json = run(fixture.command("status").arg("--json"));
	let report: serde_json::Value =
		serde_json::from_str(&json).unwrap_or_else(|error| panic!("parse status JSON: {error}"));
	let preview = &report["costPreview"];

	// The existing status schema stays in place next to the new cost section.
	assert_eq!(report["statuses"].as_array().map(Vec::len), Some(2));
	assert_eq!(report["statuses"][0]["rustName"], "State");
	assert_eq!(preview["artifact"], artifact.to_string_lossy().as_ref());

	// Per contract: 10 -> 11 bytes, one grown byte, 6,960 lamports of rent.
	let state = &preview["contracts"][0];
	assert_eq!(state["rustName"], "State");
	assert_eq!(state["currentSizeBytes"], 11);
	assert_eq!(state["dayOneGrowthBytes"], 1);
	assert_eq!(state["dayOneRentDeficitLamports"], 6_960);
	assert_eq!(state["worstCaseLadder"]["fromVersion"], 0);
	assert_eq!(state["worstCaseLadder"]["toVersion"], 1);
	assert_eq!(state["worstCaseLadder"]["steps"], 1);
	assert_eq!(state["worstCaseLadder"]["staticCu"]["estimatedCu"], 10);

	// Per instruction: the `state` slot links the process to the account, and
	// the payer/authority/program slots (writable-signer or read-only) cannot
	// miss an account contract, so no note fires.
	let instruction = &preview["instructions"][0];
	assert_eq!(instruction["rustName"], "UpdateInstruction");
	assert_eq!(instruction["ladders"][0]["accountRustName"], "State");
	assert_eq!(instruction["ladders"][0]["steps"], 1);
	assert_eq!(instruction["totalSteps"], 1);
	assert_eq!(instruction["totalRentDeficitLamports"], 6_960);
	assert_eq!(instruction["staticCu"]["estimatedCu"], 10);
	assert_eq!(instruction["notes"], serde_json::json!([]));

	// Program-wide: one touching transaction names the same figures, and the
	// rent and step maxima name it independently.
	let summary = &preview["mostExpensive"];
	assert_eq!(summary["status"], "identified");
	assert_eq!(summary["instructionRustName"], "UpdateInstruction");
	assert_eq!(summary["steps"], 1);
	assert_eq!(summary["rentDeficitLamports"], 6_960);
	assert_eq!(summary["staticCu"]["estimatedCu"], 10);
	assert_eq!(summary["maxSteps"], 1);
	assert_eq!(summary["maxStepsInstructionRustName"], "UpdateInstruction");

	// Human output quotes every JSON figure the developer must act on.
	for expected in [
		"CU model: sum of `pina profile` static estimates",
		"account State: 11 bytes now; a day-one account grows 1 bytes and funds ~6960 lamports",
		"worst-case ladder v0->v1 (1 step(s), ~6960 lamports, 10 CU static)",
		"instruction UpdateInstruction v1: 1 ladder(s), 1 step(s), ~6960 lamports, 10 CU static",
		"Most expensive touching transaction: UpdateInstruction (1 ladder(s), 1 step(s), ~6960 \
		 lamports, 10 CU static)",
		"Longest worst-case ladder: UpdateInstruction (1 step(s)); size `MAX_INLINE_STEPS` for \
		 this instruction",
		"Funding: raise the program's lamport budget",
	] {
		assert!(
			human.contains(expected),
			"missing {expected:?} in:\n{human}"
		);
	}
}

#[test]
fn status_says_cu_is_unavailable_without_an_artifact() {
	let fixture = MigrationFixture::new(true);
	run(&mut fixture.command("make"));
	fixture.publish(true);
	fixture.write_fields("value: u64, enabled: bool");
	run(&mut fixture.command("make"));

	let human = run(&mut fixture.command("status"));
	let json = run(fixture.command("status").arg("--json"));
	let report: serde_json::Value =
		serde_json::from_str(&json).unwrap_or_else(|error| panic!("parse status JSON: {error}"));
	let estimate = &report["costPreview"]["contracts"][0]["worstCaseLadder"]["staticCu"];

	assert_eq!(estimate["status"], "unavailable");
	let reason = estimate["reason"]
		.as_str()
		.unwrap_or_else(|| panic!("unavailable estimate carries a reason"));
	assert!(reason.contains("not found"), "reason: {reason}");
	assert!(human.contains("CU unavailable: compiled SBF artifact not found"));
	assert!(
		human.contains("Most expensive touching transaction: unavailable"),
		"stdout: {human}"
	);
}

#[test]
fn status_reports_an_unreadable_artifact_reason() {
	let fixture = MigrationFixture::new(true);
	run(&mut fixture.command("make"));
	fixture.publish(true);
	fixture.write_fields("value: u64, enabled: bool");
	run(&mut fixture.command("make"));
	write_sbf_artifact(&fixture.root, b"not an ELF");

	let json = run(fixture.command("status").arg("--json"));
	let report: serde_json::Value =
		serde_json::from_str(&json).unwrap_or_else(|error| panic!("parse status JSON: {error}"));
	let estimate = &report["costPreview"]["contracts"][0]["worstCaseLadder"]["staticCu"];

	assert_eq!(estimate["status"], "unavailable");
	let reason = estimate["reason"]
		.as_str()
		.unwrap_or_else(|| panic!("unavailable estimate carries a reason"));
	assert!(reason.contains("could not profile"), "reason: {reason}");
}

#[test]
fn auto_policy_snapshots_listed_kinds_and_scaffolds_the_manifest_rerun() {
	let fixture = MigrationFixture::new(false);
	fixture.configure_auto("auto = [\"accounts\", \"events\"]\n");
	fixture.write_source(AUTO_SOURCE);

	let made = run(&mut fixture.command("make"));
	assert!(made.contains("Created account:1:01@0"), "stdout: {made}");
	assert!(made.contains("Created event:1:03@0"), "stdout: {made}");
	assert!(
		made.contains("Auto policy: accounts, events"),
		"stdout: {made}"
	);
	assert!(made.contains("Created"), "stdout: {made}");

	// The policy is recorded in the manifest, not just in pina.toml.
	let manifest = fixture.manifest();
	assert_eq!(manifest["formatVersion"], 4);
	assert_eq!(manifest["auto"], serde_json::json!(["accounts", "events"]));
	assert!(manifest["contracts"].get("instruction:1:02").is_none());

	// The scaffold is idempotent and the scaffolded directive is verified.
	let directive = "cargo:rerun-if-changed=migrations/manifest.json";
	let scaffold = fs::read_to_string(fixture.build_script())
		.unwrap_or_else(|error| panic!("read scaffold: {error}"));
	assert!(scaffold.contains(&format!("\"{directive}\"")), "{scaffold}");
	run(&mut fixture.command("make"));
	assert_eq!(
		fs::read_to_string(fixture.build_script())
			.unwrap_or_else(|error| panic!("read scaffold: {error}")),
		scaffold,
	);
	run(&mut fixture.command("check"));

	// Adding instructions to the policy requires `make` and records exactly one
	// new envelope contract instead of rewriting the recorded ones.
	fixture.configure_auto("auto = true\n");
	let stale = run_failure(&mut fixture.command("check"));
	assert!(
		stale
			.1
			.contains("Run `pina migrations make` to record the policy flip"),
		"stderr: {}",
		stale.1
	);
	let flipped = run(&mut fixture.command("make"));
	assert!(
		flipped.contains("Created instruction:1:02@0"),
		"stdout: {flipped}"
	);
	assert_eq!(
		flipped.matches("Created ").count(),
		1,
		"only the newly enveloped contract is recorded: {flipped}"
	);
	run(&mut fixture.command("check"));
}

#[test]
fn dropping_a_recorded_kind_from_the_policy_fails_as_an_envelope_removal() {
	let fixture = MigrationFixture::new(false);
	fixture.configure_auto("auto = true\n");
	fixture.write_source(AUTO_SOURCE);
	run(&mut fixture.command("make"));

	fixture.configure_auto("auto = [\"events\"]\n");
	let dropped = run_failure(&mut fixture.command("make"));
	assert!(
		dropped
			.1
			.contains("Removing an envelope is a wire-format change"),
		"stderr: {}",
		dropped.1
	);
	assert!(dropped.1.contains("account:1:01"), "stderr: {}", dropped.1);
	// The check gate reports the same condition without touching the manifest.
	let checked = run_failure(&mut fixture.command("check"));
	assert!(
		checked.1.contains("pina.toml configures"),
		"stderr: {}",
		checked.1
	);
}

#[test]
fn migrations_false_on_a_recorded_contract_fails_make_and_check() {
	let fixture = MigrationFixture::new(true);
	run(&mut fixture.command("make"));

	fixture.write_source(&format!(
		"use pina::*;\ndeclare_id!(\"{PROGRAM_ID}\");\n#[discriminator]\nenum Kind {{ State = 1 \
		 }}\n#[account(discriminator = Kind::State, migrations = false)]\nstruct State {{ value: \
		 u64 }}\n"
	));
	let made = run_failure(&mut fixture.command("make"));
	assert!(
		made.1
			.contains("Removing an envelope is a wire-format change"),
		"stderr: {}",
		made.1
	);
	assert!(
		made.1.contains("must record deliberately"),
		"stderr: {}",
		made.1
	);
	let checked = run_failure(&mut fixture.command("check"));
	assert!(
		checked.1.contains("must record deliberately"),
		"stderr: {}",
		checked.1
	);

	// Removing the override restores the recorded contract unchanged.
	fixture.write_source(&format!(
		"use pina::*;\ndeclare_id!(\"{PROGRAM_ID}\");\n#[discriminator]\nenum Kind {{ State = 1 \
		 }}\n#[account(discriminator = Kind::State, migrations)]\nstruct State {{ value: u64 }}\n"
	));
	run(&mut fixture.command("check"));
}

#[test]
fn auto_policy_reports_an_undeclared_rerun_directive_instead_of_clobbering() {
	let fixture = MigrationFixture::new(false);
	fixture.configure_auto("auto = [\"accounts\"]\n");
	fixture.write_source(AUTO_SOURCE);
	let handwritten = "fn main() {\n\tprintln!(\"cargo:rustc-cfg=handwritten\");\n}\n";
	fs::write(fixture.build_script(), handwritten)
		.unwrap_or_else(|error| panic!("write hand-written build script: {error}"));

	let made = run(&mut fixture.command("make"));
	assert!(
		made.contains("cargo:rerun-if-changed=migrations/manifest.json"),
		"stdout: {made}"
	);
	assert_eq!(
		fs::read_to_string(fixture.build_script())
			.unwrap_or_else(|error| panic!("read build script: {error}")),
		handwritten,
		"a hand-written build script must never be clobbered",
	);

	let checked = run_failure(&mut fixture.command("check"));
	assert!(
		checked
			.1
			.contains("cargo:rerun-if-changed=migrations/manifest.json"),
		"stderr: {}",
		checked.1
	);
}
