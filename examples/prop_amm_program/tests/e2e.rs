//! End-to-end tests for the prop_amm_program example.
//!
//! These tests cover the semantic port of Anchor v2's `prop-amm` benchmark:
//!
//! - initializing a fresh oracle account
//! - publishing prices through the fixed global updater
//! - rotating the stored oracle authority
//! - rejecting unauthorized update and authority-rotation attempts
//!
//! ## Prerequisites
//!
//! The prop_amm_program must be compiled to an SBF binary before running these
//! tests:
//!
//! ```sh
//! cargo build --release --target bpfel-unknown-none -p prop_amm_program \
//!     -Z build-std -F bpf-entrypoint
//! ```
//!
//! Then set `SBF_OUT_DIR` to the directory containing the `.so` file, or place
//! it in `tests/fixtures/`.

use mollusk_svm::Mollusk;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use pina::PodU64;
use pina::ProgramError;
use pina::bytemuck;
use prop_amm_program::ID;
use prop_amm_program::InitializeInstruction;
use prop_amm_program::OracleState;
use prop_amm_program::PropAmmError;
use prop_amm_program::RotateAuthorityInstruction;
use prop_amm_program::UPDATE_AUTHORITY;
use prop_amm_program::UpdateInstruction;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

const SKIP_MSG: &str = "[SKIP] prop_amm_program SBF binary not found. Build it first with `cargo \
                        build --release --target bpfel-unknown-none -p prop_amm_program -Z \
                        build-std -F bpf-entrypoint`.";

fn program_id() -> Pubkey {
	let id = ID;
	let bytes: &[u8] = id.as_ref();
	let array: [u8; 32] = bytes
		.try_into()
		.unwrap_or_else(|_| panic!("address must be 32 bytes"));
	Pubkey::new_from_array(array)
}

fn try_create_mollusk() -> Option<Mollusk> {
	let so_name = "prop_amm_program.so";
	let search_dirs: Vec<std::path::PathBuf> = [
		std::env::var("SBF_OUT_DIR").ok(),
		std::env::var("BPF_OUT_DIR").ok(),
		Some("tests/fixtures".to_owned()),
	]
	.into_iter()
	.flatten()
	.map(std::path::PathBuf::from)
	.collect();

	let found = search_dirs.iter().any(|dir| dir.join(so_name).is_file());
	if !found {
		return None;
	}

	Some(Mollusk::new(&program_id(), "prop_amm_program"))
}

fn update_authority_pubkey() -> Pubkey {
	let bytes: [u8; 32] = UPDATE_AUTHORITY
		.as_ref()
		.try_into()
		.unwrap_or_else(|_| panic!("address must be 32 bytes"));
	Pubkey::new_from_array(bytes)
}

fn system_account(lamports: u64) -> Account {
	Account::new(lamports, 0, &solana_sdk_ids::system_program::id())
}

fn read_oracle(account: &Account) -> &OracleState {
	bytemuck::from_bytes::<OracleState>(&account.data)
}

fn initialize_ix_data() -> Vec<u8> {
	InitializeInstruction::builder().build().to_bytes().to_vec()
}

fn update_ix_data(new_price: u64) -> Vec<u8> {
	UpdateInstruction::builder()
		.new_price(PodU64::from_primitive(new_price))
		.build()
		.to_bytes()
		.to_vec()
}

fn rotate_authority_ix_data(new_authority: &Pubkey) -> Vec<u8> {
	RotateAuthorityInstruction::builder()
		.new_authority(new_authority.to_bytes().into())
		.build()
		.to_bytes()
		.to_vec()
}

fn initialize_oracle(
	mollusk: &Mollusk,
	payer: &Pubkey,
	oracle: &Pubkey,
) -> mollusk_svm::result::InstructionResult {
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&initialize_ix_data(),
		vec![
			AccountMeta::new(*payer, true),
			AccountMeta::new(*oracle, true),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let accounts = vec![
		(*payer, system_account(1_000_000_000)),
		(*oracle, Account::default()),
		keyed_account_for_system_program(),
	];

	mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()])
}

#[test]
fn initialize_creates_oracle() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let payer = Pubkey::new_unique();
	let oracle = Pubkey::new_unique();

	let result = initialize_oracle(&mollusk, &payer, &oracle);
	let oracle_account = result
		.get_account(&oracle)
		.expect("oracle account should exist after initialize");
	let oracle_state = read_oracle(&oracle_account);

	assert_eq!(oracle_state.authority.as_ref(), payer.as_ref());
	assert_eq!(u64::from(oracle_state.price), 0);
}

#[test]
fn rotate_authority_updates_oracle_authority() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let payer = Pubkey::new_unique();
	let oracle = Pubkey::new_unique();
	let initialize_result = initialize_oracle(&mollusk, &payer, &oracle);
	let oracle_account = initialize_result
		.get_account(&oracle)
		.expect("oracle account should exist after initialize");
	let new_authority = Pubkey::new_unique();

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&rotate_authority_ix_data(&new_authority),
		vec![
			AccountMeta::new(oracle, false),
			AccountMeta::new_readonly(payer, true),
		],
	);

	let accounts = vec![
		(oracle, oracle_account.clone()),
		(payer, system_account(1_000_000_000)),
	];

	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);
	let updated_oracle = result
		.get_account(&oracle)
		.expect("oracle account should remain after rotate");
	let oracle_state = read_oracle(&updated_oracle);

	assert_eq!(oracle_state.authority.as_ref(), new_authority.as_ref());
	assert_eq!(u64::from(oracle_state.price), 0);
}

#[test]
fn rotate_authority_rejects_wrong_authority() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let payer = Pubkey::new_unique();
	let oracle = Pubkey::new_unique();
	let wrong_authority = Pubkey::new_unique();
	let initialize_result = initialize_oracle(&mollusk, &payer, &oracle);
	let oracle_account = initialize_result
		.get_account(&oracle)
		.expect("oracle account should exist after initialize");

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&rotate_authority_ix_data(&Pubkey::new_unique()),
		vec![
			AccountMeta::new(oracle, false),
			AccountMeta::new_readonly(wrong_authority, true),
		],
	);

	let accounts = vec![
		(oracle, oracle_account.clone()),
		(wrong_authority, system_account(1_000_000_000)),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(PropAmmError::UnauthorizedOracleAuthority.into())],
	);
}

#[test]
fn update_accepts_global_update_authority() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let payer = Pubkey::new_unique();
	let oracle = Pubkey::new_unique();
	let initialize_result = initialize_oracle(&mollusk, &payer, &oracle);
	let oracle_account = initialize_result
		.get_account(&oracle)
		.expect("oracle account should exist after initialize");
	let update_authority = update_authority_pubkey();

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&update_ix_data(1_234),
		vec![
			AccountMeta::new(oracle, false),
			AccountMeta::new_readonly(update_authority, true),
		],
	);

	let accounts = vec![
		(oracle, oracle_account.clone()),
		(update_authority, system_account(1_000_000_000)),
	];

	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);
	let updated_oracle = result
		.get_account(&oracle)
		.expect("oracle account should remain after update");
	let oracle_state = read_oracle(&updated_oracle);

	eprintln!(
		"[CU] prop_amm update with authorized updater: {} compute units consumed",
		result.compute_units_consumed
	);
	assert_eq!(u64::from(oracle_state.price), 1_234);
	assert_eq!(oracle_state.authority.as_ref(), payer.as_ref());
}

#[test]
fn update_rejects_wrong_update_authority() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let payer = Pubkey::new_unique();
	let oracle = Pubkey::new_unique();
	let initialize_result = initialize_oracle(&mollusk, &payer, &oracle);
	let oracle_account = initialize_result
		.get_account(&oracle)
		.expect("oracle account should exist after initialize");

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&update_ix_data(9_999),
		vec![
			AccountMeta::new(oracle, false),
			AccountMeta::new_readonly(payer, true),
		],
	);

	let accounts = vec![
		(oracle, oracle_account.clone()),
		(payer, system_account(1_000_000_000)),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(PropAmmError::UnauthorizedUpdateAuthority.into())],
	);
}

#[test]
fn update_rejects_missing_signer_flag() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let payer = Pubkey::new_unique();
	let oracle = Pubkey::new_unique();
	let initialize_result = initialize_oracle(&mollusk, &payer, &oracle);
	let oracle_account = initialize_result
		.get_account(&oracle)
		.expect("oracle account should exist after initialize");
	let update_authority = update_authority_pubkey();

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&update_ix_data(5_555),
		vec![
			AccountMeta::new(oracle, false),
			AccountMeta::new_readonly(update_authority, false),
		],
	);

	let accounts = vec![
		(oracle, oracle_account.clone()),
		(update_authority, system_account(1_000_000_000)),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(ProgramError::MissingRequiredSignature)],
	);
}
