//! Exact SBF compute-unit snapshots for the PinaPod migration.
//!
//! The compute-unit workflow compiles this harness from the pull-request head,
//! then runs it once with base ELFs and once with head ELFs. The instruction
//! bytes and account fixtures therefore stay constant while only the program
//! implementation changes.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use account_realloc_program::InitializeIx as ReallocInitializeIx;
use account_realloc_program::ReallocIx;
use account_realloc_program::Sample;
use counter_program::CounterInstruction;
use mollusk_svm::Mollusk;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::program::loader_keys::LOADER_V3;
use mollusk_svm::result::InstructionResult;
use profile_program::AddTagInstruction;
use profile_program::InitializeInstruction as ProfileInitializeInstruction;
use profile_program::RemoveTagInstruction;
use profile_program::UpdateProfileInstruction;
use serde_json::json;
use sha2::Digest;
use sha2::Sha256;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

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

	fn from_result(result: &InstructionResult) -> Self {
		Self {
			compute_units: result.compute_units_consumed,
			succeeded: result.program_result.is_ok(),
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

fn process_loader_case(
	mollusk: &Mollusk,
	operation: u8,
	accounts: &[(Pubkey, Account)],
) -> Measurement {
	let instruction = Instruction::new_with_bytes(
		Pubkey::new_from_array([77; 32]),
		&[operation],
		accounts
			.iter()
			.map(|(address, _)| AccountMeta::new_readonly(*address, false))
			.collect(),
	);
	let result = mollusk.process_instruction(&instruction, accounts);
	Measurement::from_result(&result)
}

fn token_mint_data() -> Vec<u8> {
	let mut data = vec![0; 82];
	data[44] = 6;
	data[45] = 1;
	data
}

fn token_account_data(mint: &Pubkey, owner: &Pubkey) -> Vec<u8> {
	let mut data = vec![0; 165];
	data[..32].copy_from_slice(mint.as_ref());
	data[32..64].copy_from_slice(owner.as_ref());
	data[64..72].copy_from_slice(&55_u64.to_le_bytes());
	data[108] = 1;
	data
}

fn readonly_account(owner: &Pubkey, data: Vec<u8>) -> Account {
	Account {
		lamports: 1,
		data,
		owner: *owner,
		executable: false,
		rent_epoch: 0,
	}
}

fn token_loader_measurements(elf_dir: &Path) -> BTreeMap<String, Measurement> {
	let program_id = Pubkey::new_from_array([77; 32]);
	let mollusk = load_program(elf_dir, "token_loader_cu_program", &program_id);
	let legacy_program = as_pubkey(pina::token::ID);
	let token_2022_program = as_pubkey(pina::token_2022::ID);
	let wallet = Pubkey::new_from_array([78; 32]);
	let other_wallet = Pubkey::new_from_array([79; 32]);
	let mint = Pubkey::new_from_array([80; 32]);
	let wrong_address = Pubkey::new_from_array([81; 32]);
	let wrong_owner = solana_sdk_ids::system_program::id();
	let ata_program = as_pubkey(pina::associated_token_account::ID);
	let (legacy_ata, _) = Pubkey::find_program_address(
		&[wallet.as_ref(), legacy_program.as_ref(), mint.as_ref()],
		&ata_program,
	);
	let (token_2022_ata, _) = Pubkey::find_program_address(
		&[wallet.as_ref(), token_2022_program.as_ref(), mint.as_ref()],
		&ata_program,
	);
	let mint_data = token_mint_data();
	let account_data = token_account_data(&mint, &wallet);
	let wallet_account = Account::default();
	let mint_address_account = Account::default();

	let mut measurements = BTreeMap::new();
	let mut measure = |case: &str, operation: u8, accounts: Vec<(Pubkey, Account)>| {
		let id = format!("token_loader_cu_program/{case}");
		let measurement = process_loader_case(&mollusk, operation, &accounts);
		measurements.insert(id, measurement);
	};

	measure(
		"legacy_mint_success",
		0,
		vec![(mint, readonly_account(&legacy_program, mint_data.clone()))],
	);
	measure(
		"legacy_account_success",
		1,
		vec![(
			legacy_ata,
			readonly_account(&legacy_program, account_data.clone()),
		)],
	);
	measure(
		"token_2022_mint_success",
		2,
		vec![(
			mint,
			readonly_account(&token_2022_program, mint_data.clone()),
		)],
	);
	measure(
		"token_2022_account_success",
		3,
		vec![(
			token_2022_ata,
			readonly_account(&token_2022_program, account_data.clone()),
		)],
	);
	measure(
		"legacy_ata_success",
		4,
		vec![
			(
				legacy_ata,
				readonly_account(&legacy_program, account_data.clone()),
			),
			(wallet, wallet_account.clone()),
			(mint, mint_address_account.clone()),
		],
	);
	measure(
		"token_2022_ata_success",
		5,
		vec![
			(
				token_2022_ata,
				readonly_account(&token_2022_program, account_data.clone()),
			),
			(wallet, wallet_account.clone()),
			(mint, mint_address_account.clone()),
		],
	);
	measure(
		"explicit_owner_assert_then_load",
		6,
		vec![(
			legacy_ata,
			readonly_account(&legacy_program, account_data.clone()),
		)],
	);
	measure(
		"legacy_mint_wrong_owner",
		0,
		vec![(mint, readonly_account(&wrong_owner, mint_data.clone()))],
	);
	measure(
		"legacy_account_wrong_owner",
		1,
		vec![(
			legacy_ata,
			readonly_account(&wrong_owner, account_data.clone()),
		)],
	);
	measure(
		"token_2022_mint_wrong_owner",
		2,
		vec![(mint, readonly_account(&wrong_owner, mint_data.clone()))],
	);
	measure(
		"token_2022_account_wrong_owner",
		3,
		vec![(
			token_2022_ata,
			readonly_account(&wrong_owner, account_data.clone()),
		)],
	);
	measure(
		"legacy_ata_wrong_address",
		4,
		vec![
			(
				wrong_address,
				readonly_account(&legacy_program, account_data.clone()),
			),
			(wallet, wallet_account.clone()),
			(mint, mint_address_account.clone()),
		],
	);
	measure(
		"legacy_ata_wrong_stored_authority",
		4,
		vec![
			(
				legacy_ata,
				readonly_account(&legacy_program, token_account_data(&mint, &other_wallet)),
			),
			(wallet, wallet_account),
			(mint, mint_address_account),
		],
	);

	measurements
}

fn counter_measurements(elf_dir: &Path) -> BTreeMap<String, Measurement> {
	let program_id = as_pubkey(counter_program::ID);
	let mollusk = load_program(elf_dir, "counter_program", &program_id);
	let authority = Pubkey::new_from_array([1; 32]);
	let (counter, bump) =
		Pubkey::find_program_address(&[b"counter", authority.as_ref()], &program_id);

	let initialize = Instruction::new_with_bytes(
		program_id,
		&[CounterInstruction::Initialize as u8, bump],
		vec![
			AccountMeta::new(authority, true),
			AccountMeta::new(counter, true),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);
	let initialize_result = process_success(
		&mollusk,
		"counter_program/initialize",
		&initialize,
		&[
			(authority, system_account(1_000_000_000)),
			(counter, Account::default()),
			keyed_account_for_system_program(),
		],
	);

	let increment = Instruction::new_with_bytes(
		program_id,
		&[CounterInstruction::Increment as u8],
		vec![
			AccountMeta::new_readonly(authority, true),
			AccountMeta::new(counter, false),
		],
	);
	let authority_account = initialize_result
		.get_account(&authority)
		.cloned()
		.unwrap_or_else(|| panic!("counter authority must remain available"));
	let counter_account = initialize_result
		.get_account(&counter)
		.cloned()
		.unwrap_or_else(|| panic!("initialized counter must be available"));
	let increment_result = process_success(
		&mollusk,
		"counter_program/increment",
		&increment,
		&[(authority, authority_account), (counter, counter_account)],
	);

	BTreeMap::from([
		(
			"counter_program/increment".to_owned(),
			Measurement::success(&increment_result),
		),
		(
			"counter_program/initialize".to_owned(),
			Measurement::success(&initialize_result),
		),
	])
}

fn profile_initialize_data(bump: u8, name: &str, bio: &str) -> Vec<u8> {
	let mut data = vec![0; ProfileInitializeInstruction::SIZE];
	ProfileInitializeInstruction::initialize(&mut data, |instruction| {
		instruction.bump = bump;
		instruction.name.try_set(name)?;
		instruction.bio.try_set(bio)?;
		Ok(())
	})
	.unwrap_or_else(|error| panic!("encode profile initialize: {error:?}"));
	data
}

fn profile_update_data(name: &str, bio: &str) -> Vec<u8> {
	let mut data = vec![0; UpdateProfileInstruction::SIZE];
	UpdateProfileInstruction::initialize(&mut data, |instruction| {
		instruction.name.try_set(name)?;
		instruction.bio.try_set(bio)?;
		Ok(())
	})
	.unwrap_or_else(|error| panic!("encode profile update: {error:?}"));
	data
}

fn profile_tag_data(tag: u64) -> Vec<u8> {
	let mut data = vec![0; AddTagInstruction::SIZE];
	AddTagInstruction::initialize(&mut data, |instruction| {
		instruction.tag.set(tag);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("encode profile tag: {error:?}"));
	data
}

fn profile_remove_data(index: u64) -> Vec<u8> {
	let mut data = vec![0; RemoveTagInstruction::SIZE];
	RemoveTagInstruction::initialize(&mut data, |instruction| {
		instruction.index.set(index);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("encode profile removal: {error:?}"));
	data
}

fn profile_measurements(elf_dir: &Path) -> BTreeMap<String, Measurement> {
	let program_id = as_pubkey(profile_program::ID);
	let mollusk = load_program(elf_dir, "profile_program", &program_id);
	let authority = Pubkey::new_from_array([2; 32]);
	let (profile, bump) =
		Pubkey::find_program_address(&[b"profile", authority.as_ref()], &program_id);

	let initialize = Instruction::new_with_bytes(
		program_id,
		&profile_initialize_data(bump, "alice", "hello world"),
		vec![
			AccountMeta::new(authority, true),
			AccountMeta::new(profile, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);
	let initialize_result = process_success(
		&mollusk,
		"profile_program/initialize",
		&initialize,
		&[
			(authority, system_account(1_000_000_000)),
			(profile, Account::default()),
			keyed_account_for_system_program(),
		],
	);

	let profile_accounts = || {
		vec![
			AccountMeta::new_readonly(authority, true),
			AccountMeta::new(profile, false),
		]
	};
	let update = Instruction::new_with_bytes(
		program_id,
		&profile_update_data("alice2", "updated bio"),
		profile_accounts(),
	);
	let update_result = process_success(
		&mollusk,
		"profile_program/update_profile",
		&update,
		&initialize_result.resulting_accounts,
	);
	let add_tag =
		Instruction::new_with_bytes(program_id, &profile_tag_data(10), profile_accounts());
	let add_tag_result = process_success(
		&mollusk,
		"profile_program/add_tag",
		&add_tag,
		&update_result.resulting_accounts,
	);
	let remove_tag =
		Instruction::new_with_bytes(program_id, &profile_remove_data(0), profile_accounts());
	let remove_tag_result = process_success(
		&mollusk,
		"profile_program/remove_tag",
		&remove_tag,
		&add_tag_result.resulting_accounts,
	);

	BTreeMap::from([
		(
			"profile_program/add_tag".to_owned(),
			Measurement::success(&add_tag_result),
		),
		(
			"profile_program/initialize".to_owned(),
			Measurement::success(&initialize_result),
		),
		(
			"profile_program/remove_tag".to_owned(),
			Measurement::success(&remove_tag_result),
		),
		(
			"profile_program/update_profile".to_owned(),
			Measurement::success(&update_result),
		),
	])
}

fn realloc_initialize_data(bump: u8) -> Vec<u8> {
	let mut data = vec![0; ReallocInitializeIx::SIZE];
	ReallocInitializeIx::initialize(&mut data, |instruction| {
		instruction.bump = bump;
		Ok(())
	})
	.unwrap_or_else(|error| panic!("encode account_realloc_program initialize: {error:?}"));
	data
}

fn realloc_data(len: usize) -> Vec<u8> {
	let len = u16::try_from(len).unwrap_or_else(|_| panic!("sample size must fit in u16"));
	let mut data = vec![0; ReallocIx::SIZE];
	ReallocIx::initialize(&mut data, |instruction| {
		instruction.len.set(len);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("encode account_realloc_program instruction: {error:?}"));
	data
}

fn realloc_measurements(elf_dir: &Path) -> BTreeMap<String, Measurement> {
	let program_id = as_pubkey(account_realloc_program::ID);
	let mollusk = load_program(elf_dir, "account_realloc_program", &program_id);
	let authority = Pubkey::new_from_array([3; 32]);
	let (sample, bump) =
		Pubkey::find_program_address(&[b"sample", authority.as_ref()], &program_id);

	let initialize = Instruction::new_with_bytes(
		program_id,
		&realloc_initialize_data(bump),
		vec![
			AccountMeta::new(authority, true),
			AccountMeta::new(sample, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);
	let initialize_result = process_success(
		&mollusk,
		"account_realloc_program/initialize",
		&initialize,
		&[
			(authority, system_account(1_000_000_000)),
			(sample, Account::default()),
			keyed_account_for_system_program(),
		],
	);

	let resize_accounts = || {
		vec![
			AccountMeta::new(authority, true),
			AccountMeta::new(sample, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		]
	};
	let grown_size = Sample::projected_bytes(8)
		.unwrap_or_else(|error| panic!("project eight-value sample: {error:?}"));
	let grow =
		Instruction::new_with_bytes(program_id, &realloc_data(grown_size), resize_accounts());
	let grow_result = process_success(
		&mollusk,
		"account_realloc_program/grow_0_to_8",
		&grow,
		&initialize_result.resulting_accounts,
	);
	let rewrite =
		Instruction::new_with_bytes(program_id, &realloc_data(grown_size), resize_accounts());
	let rewrite_result = process_success(
		&mollusk,
		"account_realloc_program/rewrite_8_to_8",
		&rewrite,
		&grow_result.resulting_accounts,
	);
	let shrink = Instruction::new_with_bytes(
		program_id,
		&realloc_data(Sample::MIN_SIZE),
		resize_accounts(),
	);
	let shrink_result = process_success(
		&mollusk,
		"account_realloc_program/shrink_8_to_0",
		&shrink,
		&rewrite_result.resulting_accounts,
	);

	BTreeMap::from([
		(
			"account_realloc_program/grow_0_to_8".to_owned(),
			Measurement::success(&grow_result),
		),
		(
			"account_realloc_program/initialize".to_owned(),
			Measurement::success(&initialize_result),
		),
		(
			"account_realloc_program/rewrite_8_to_8".to_owned(),
			Measurement::success(&rewrite_result),
		),
		(
			"account_realloc_program/shrink_8_to_0".to_owned(),
			Measurement::success(&shrink_result),
		),
	])
}

fn measure_all(elf_dir: &Path) -> BTreeMap<String, Measurement> {
	let mut measurements = BTreeMap::new();
	measurements.extend(counter_measurements(elf_dir));
	measurements.extend(profile_measurements(elf_dir));
	measurements.extend(realloc_measurements(elf_dir));
	measurements.extend(token_loader_measurements(elf_dir));
	measurements
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

#[test]
fn measure_runtime_compute_units() {
	let Some(elf_dir) = std::env::var_os("PINA_CU_ELF_DIR").map(PathBuf::from) else {
		eprintln!("skipping exact CU snapshot: PINA_CU_ELF_DIR is not set");
		return;
	};
	let output = std::env::var_os("PINA_CU_OUTPUT")
		.map(PathBuf::from)
		.unwrap_or_else(|| panic!("PINA_CU_OUTPUT must be set with PINA_CU_ELF_DIR"));

	let first = measure_all(&elf_dir);
	let second = measure_all(&elf_dir);
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
	let artifacts = [
		"account_realloc_program",
		"counter_program",
		"profile_program",
		"token_loader_cu_program",
	]
	.into_iter()
	.map(|program| {
		let path = elf_dir.join(format!("{program}.so"));
		(
			program,
			json!({
				"file": path.file_name().unwrap_or_default().to_string_lossy(),
				"sha256": sha256_file(&path),
			}),
		)
	})
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
