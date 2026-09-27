//! Exact SBF compute-unit snapshot for the migrations program.
//!
//! This harness used to live at the workspace root, alongside the other
//! example programs' snapshots. It moved here — inside the program's own
//! directory tree — because the program's migration-aware macros resolve
//! `migrations/manifest.json` by walking up from the expanding crate: from
//! the workspace root there is no manifest above, so the source-included
//! program compiled unenveloped there. From `tests/` the walk finds the
//! program's manifest exactly as its own `#[cfg(test)]` modules did.
//!
//! The env contract matches the workspace-root harness (`PINA_CU_ELF_DIR`,
//! `PINA_CU_OUTPUT`, …); without it the snapshot is skipped, so plain
//! `cargo test` still compiles and runs it as a no-op.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use mollusk_svm::Mollusk;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::program::loader_keys::LOADER_V3;
use mollusk_svm::result::InstructionResult;
use serde_json::json;
use sha2::Digest;
use sha2::Sha256;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

// The program is a cdylib only (see Cargo.toml), so its real types come
// in through a source include rather than an rlib dependency.
#[path = "../src/lib.rs"]
mod program;

const MOLLUSK_VERSION: &str = "0.14.0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Measurement {
	compute_units: u64,
	succeeded: bool,
}

impl Measurement {
	fn success(result: &InstructionResult) -> Self {
		Self {
			compute_units: result.compute_units_consumed,
			succeeded: true,
		}
	}
}

fn as_pubkey(address: impl AsRef<[u8]>) -> Pubkey {
	let bytes: [u8; 32] = address
		.as_ref()
		.try_into()
		.unwrap_or_else(|_| panic!("program address must contain 32 bytes"));
	Pubkey::new_from_array(bytes)
}

fn load_program(elf_dir: &Path, name: &str, program_id: &Pubkey) -> Mollusk {
	let path = elf_dir.join(format!("{name}.so"));
	let elf = fs::read(&path)
		.unwrap_or_else(|error| panic!("read exact ELF {}: {error}", path.display()));
	let mut mollusk = Mollusk::default();
	mollusk.add_program_with_loader_and_elf(program_id, &LOADER_V3, &elf);
	mollusk
}

fn system_account(lamports: u64) -> Account {
	Account::new(lamports, 0, &solana_sdk_ids::system_program::id())
}

fn process_success(
	mollusk: &Mollusk,
	case_id: &str,
	instruction: &Instruction,
	accounts: &[(Pubkey, Account)],
) -> InstructionResult {
	let result = mollusk.process_instruction(instruction, accounts);
	assert!(
		result.program_result.is_ok(),
		"{case_id} failed: {:?} ({:?})",
		result.program_result,
		result.raw_result,
	);
	result
}

fn migration_measurements(elf_dir: &Path) -> BTreeMap<String, Measurement> {
	use program::MigrationAccount;
	use program::MigrationInstruction;

	let program_id = as_pubkey(program::ID);
	let mollusk = load_program(elf_dir, "migrations_program", &program_id);
	let authority = Pubkey::new_from_array([2; 32]);
	let referrer = Pubkey::new_from_array([3; 32]);
	let state = Pubkey::new_from_array([4; 32]);
	let payer = Pubkey::new_from_array([5; 32]);

	let mut current_state = vec![0_u8; program::State::SIZE];
	current_state[0] = MigrationAccount::State as u8;
	current_state[1] = 2;
	current_state[2..34].copy_from_slice(authority.as_ref());
	current_state[34..42].copy_from_slice(&7_u64.to_le_bytes());
	current_state[42] = 1;
	let mut current_data = [0_u8; 12];
	current_data[0] = MigrationInstruction::Update as u8;
	current_data[1] = 2;
	current_data[2..10].copy_from_slice(&42_u64.to_le_bytes());
	let update_metas = |state: Pubkey, payer: Pubkey| {
		vec![
			AccountMeta::new_readonly(authority, true),
			AccountMeta::new_readonly(referrer, false),
			AccountMeta::new(state, false),
			AccountMeta::new(payer, true),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		]
	};
	let current_update =
		Instruction::new_with_bytes(program_id, &current_data, update_metas(state, payer));
	let current_result = process_success(
		&mollusk,
		"migrations_program/update_current",
		&current_update,
		&[
			(authority, system_account(1_000_000_000)),
			(referrer, system_account(1)),
			(
				state,
				Account {
					lamports: 1_000_000,
					data: current_state.clone(),
					owner: program_id,
					executable: false,
					rent_epoch: 0,
				},
			),
			(payer, system_account(1_000_000_000)),
			keyed_account_for_system_program(),
		],
	);

	let mut historical_state = vec![0_u8; 42];
	historical_state[0] = MigrationAccount::State as u8;
	historical_state[1] = 0;
	historical_state[2..34].copy_from_slice(authority.as_ref());
	historical_state[34..42].copy_from_slice(&7_u64.to_le_bytes());
	let mut historical_data = [0_u8; 10];
	historical_data[0] = MigrationInstruction::Update as u8;
	historical_data[1] = 0;
	historical_data[2..].copy_from_slice(&42_u64.to_le_bytes());
	let migrating_update =
		Instruction::new_with_bytes(program_id, &historical_data, update_metas(state, payer));
	let migrating_result = process_success(
		&mollusk,
		"migrations_program/update_historical_migration",
		&migrating_update,
		&[
			(authority, system_account(1_000_000_000)),
			(referrer, system_account(1)),
			(
				state,
				Account {
					lamports: 1_000_000,
					data: historical_state,
					owner: program_id,
					executable: false,
					rent_epoch: 0,
				},
			),
			(payer, system_account(1_000_000_000)),
			keyed_account_for_system_program(),
		],
	);

	BTreeMap::from([
		(
			"migrations_program/update_current".to_owned(),
			Measurement::success(&current_result),
		),
		(
			"migrations_program/update_historical_migration".to_owned(),
			Measurement::success(&migrating_result),
		),
	])
}

fn sha256_file(path: &Path) -> String {
	let bytes = fs::read(path)
		.unwrap_or_else(|error| panic!("read provenance file {}: {error}", path.display()));
	let digest = Sha256::digest(bytes);
	let mut encoded = String::with_capacity(digest.len() * 2);
	for byte in digest {
		write!(&mut encoded, "{byte:02x}")
			.unwrap_or_else(|_| panic!("write SHA-256 digest to String"));
	}
	encoded
}

fn record_migrations_measurements() {
	let Some(elf_dir) = std::env::var_os("PINA_CU_ELF_DIR").map(PathBuf::from) else {
		eprintln!("skipping exact CU snapshot: PINA_CU_ELF_DIR is not set");
		return;
	};
	let output = std::env::var_os("PINA_CU_OUTPUT")
		.map(PathBuf::from)
		.unwrap_or_else(|| panic!("PINA_CU_OUTPUT must be set with PINA_CU_ELF_DIR"));

	let first = migration_measurements(&elf_dir);
	let second = migration_measurements(&elf_dir);
	assert_eq!(first, second, "Mollusk CU results must be deterministic");

	let cases = first
		.into_iter()
		.map(|(id, measurement)| {
			json!({
				"id": id,
				"computeUnits": measurement.compute_units,
				"succeeded": measurement.succeeded,
			})
		})
		.collect::<Vec<_>>();
	let path = elf_dir.join("migrations_program.so");
	let artifacts = [(
		"migrations_program",
		json!({
			"file": path.file_name().unwrap_or_default().to_string_lossy(),
			"sha256": sha256_file(&path),
		}),
	)]
	.into_iter()
	.collect::<BTreeMap<_, _>>();
	let lock_file = std::env::var_os("PINA_CU_SOURCE_LOCK_FILE")
		.map(PathBuf::from)
		.unwrap_or_else(|| panic!("PINA_CU_SOURCE_LOCK_FILE must be set"));
	let report = json!({
		"schemaVersion": 2,
		"provenance": {
			"artifacts": artifacts,
			"bpfToolchain": std::env::var("PINA_CU_TOOLCHAIN").unwrap_or_else(|_| "unknown".to_owned()),
			"cargoLockSha256": sha256_file(&lock_file),
			"harnessRevision": std::env::var("PINA_CU_HARNESS_REVISION").unwrap_or_else(|_| "unknown".to_owned()),
			"molluskVersion": MOLLUSK_VERSION,
			"repetitions": 2,
			"sourceRevision": std::env::var("PINA_CU_SOURCE_REVISION").unwrap_or_else(|_| "unknown".to_owned()),
		},
		"cases": cases,
	});

	if let Some(parent) = output.parent() {
		fs::create_dir_all(parent)
			.unwrap_or_else(|error| panic!("create CU output directory: {error}"));
	}
	fs::write(
		&output,
		serde_json::to_vec_pretty(&report)
			.unwrap_or_else(|error| panic!("serialize CU report: {error}")),
	)
	.unwrap_or_else(|error| panic!("write CU report {}: {error}", output.display()));
}

#[test]
fn measure_migrations_compute_units() {
	record_migrations_measurements();
}
