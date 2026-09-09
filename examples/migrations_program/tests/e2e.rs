//! SBF compatibility and rollback tests for first-class migrations.
//!
//! Build the program before running these ignored tests:
//!
//! ```sh
//! cargo build-sbf --manifest-path examples/migrations_program/Cargo.toml \
//!     --sbf-out-dir target/deploy --features bpf-entrypoint
//! SBF_OUT_DIR=target/deploy \
//!     cargo test -p migrations_program --test e2e -- --include-ignored
//! ```

use migrations_program::ID;
use migrations_program::MigrationAccount;
use migrations_program::MigrationInstruction;
use migrations_program::RelayInstruction;
use migrations_program::State;
use mollusk_svm::Mollusk;
use mollusk_svm::program::create_program_account_loader_v3;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use mollusk_svm::result::InstructionResult;
use pina::MigratableAccount;
use pina::PinaProgramError;
use pina::ProgramError;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

const HISTORICAL_STATE_SIZE: usize = 42;
const HISTORICAL_UPDATE_SIZE: usize = 10;

fn program_id() -> Pubkey {
	let bytes: &[u8] = ID.as_ref();
	let array: [u8; 32] = bytes
		.try_into()
		.unwrap_or_else(|_| panic!("program address must be 32 bytes"));
	Pubkey::new_from_array(array)
}

fn create_mollusk() -> Mollusk {
	let so_name = "migrations_program.so";
	let search_dirs: Vec<std::path::PathBuf> = [
		std::env::var("SBF_OUT_DIR").ok(),
		std::env::var("BPF_OUT_DIR").ok(),
		Some("tests/fixtures".to_owned()),
	]
	.into_iter()
	.flatten()
	.map(std::path::PathBuf::from)
	.collect();

	assert!(
		search_dirs.iter().any(|dir| dir.join(so_name).is_file()),
		"migrations_program SBF binary not found; build it before running ignored e2e tests"
	);

	Mollusk::new(&program_id(), "migrations_program")
}

fn historical_update_data(value: u64) -> [u8; HISTORICAL_UPDATE_SIZE] {
	let mut data = [0_u8; HISTORICAL_UPDATE_SIZE];
	data[0] = MigrationInstruction::Update as u8;
	data[1] = 0;
	data[2..].copy_from_slice(&value.to_le_bytes());
	data
}

fn relay_data(value: u64) -> Vec<u8> {
	let mut data = vec![0_u8; RelayInstruction::SIZE];
	RelayInstruction::initialize(&mut data, |instruction| {
		instruction.value.set(value);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("relay instruction encoding failed: {error:?}"));
	data
}

fn historical_state_data(authority: &Pubkey, value: u64) -> Vec<u8> {
	let mut data = vec![0_u8; HISTORICAL_STATE_SIZE];
	data[0] = MigrationAccount::State as u8;
	data[1] = 0;
	data[2..34].copy_from_slice(authority.as_ref());
	data[34..].copy_from_slice(&value.to_le_bytes());
	data
}

fn system_account(lamports: u64) -> Account {
	Account::new(lamports, 0, &solana_sdk_ids::system_program::id())
}

fn stored_state(data: Vec<u8>, lamports: u64) -> Account {
	Account {
		lamports,
		data,
		owner: program_id(),
		executable: false,
		rent_epoch: 0,
	}
}

fn account<'a>(result: &'a InstructionResult, address: &Pubkey) -> &'a Account {
	&result
		.resulting_accounts
		.iter()
		.find(|(candidate, _)| candidate == address)
		.unwrap_or_else(|| panic!("account {address} missing from result"))
		.1
}

fn update_instruction(
	authority: Pubkey,
	referrer: Pubkey,
	state: Pubkey,
	payer: Pubkey,
	data: &[u8],
) -> Instruction {
	Instruction::new_with_bytes(
		program_id(),
		data,
		vec![
			AccountMeta::new_readonly(authority, true),
			AccountMeta::new_readonly(referrer, false),
			AccountMeta::new(state, false),
			AccountMeta::new(payer, true),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	)
}

fn migration_accounts(
	mollusk: &Mollusk,
	authority: Pubkey,
	referrer: Pubkey,
	state: Pubkey,
	payer: Pubkey,
	state_authority: &Pubkey,
) -> (Vec<(Pubkey, Account)>, Vec<u8>, u64, u64) {
	let old_data = historical_state_data(state_authority, 7);
	let old_lamports = mollusk.sysvars.rent.minimum_balance(HISTORICAL_STATE_SIZE);
	let payer_lamports = 1_000_000_000;
	let accounts = vec![
		(authority, system_account(1_000_000)),
		(referrer, system_account(1)),
		(state, stored_state(old_data.clone(), old_lamports)),
		(payer, system_account(payer_lamports)),
		keyed_account_for_system_program(),
	];

	(accounts, old_data, old_lamports, payer_lamports)
}

fn assert_current_state(result: &InstructionResult, state: &Pubkey, value: u64) {
	let stored = account(result, state);
	assert_eq!(stored.data.len(), State::SIZE);
	State::validate_current_migration(&stored.data)
		.unwrap_or_else(|error| panic!("migrated state validation failed: {error:?}"));
	let current = State::try_from_bytes(&stored.data)
		.unwrap_or_else(|error| panic!("migrated state decoding failed: {error:?}"));
	assert_eq!(current.value.get(), value);
	assert!(bool::from(current.enabled));
}

#[test]
#[ignore = "requires the migrations_program SBF binary"]
fn old_client_can_omit_every_appended_optional_account() {
	let mollusk = create_mollusk();
	let authority = Pubkey::new_unique();
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&historical_update_data(42),
		vec![AccountMeta::new_readonly(authority, true)],
	);

	mollusk.process_and_validate_instruction(
		&instruction,
		&[(authority, system_account(1_000_000))],
		&[Check::success()],
	);
}

#[test]
#[ignore = "requires the migrations_program SBF binary"]
fn historical_payload_migrates_and_resizes_stale_state_before_business_logic() {
	let mollusk = create_mollusk();
	let authority = Pubkey::new_unique();
	let referrer = Pubkey::new_unique();
	let state = Pubkey::new_unique();
	let payer = Pubkey::new_unique();
	let (accounts, _, old_lamports, payer_lamports) =
		migration_accounts(&mollusk, authority, referrer, state, payer, &authority);
	let instruction = update_instruction(
		authority,
		referrer,
		state,
		payer,
		&historical_update_data(88),
	);

	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);
	assert_current_state(&result, &state, 88);

	let required_lamports = mollusk.sysvars.rent.minimum_balance(State::SIZE);
	let transfer = required_lamports.saturating_sub(old_lamports);
	assert_eq!(account(&result, &state).lamports, required_lamports);
	assert_eq!(account(&result, &payer).lamports, payer_lamports - transfer);
}

#[test]
#[ignore = "requires the migrations_program SBF binary"]
fn authorization_failure_after_migration_rolls_back_bytes_length_and_lamports() {
	let mollusk = create_mollusk();
	let authority = Pubkey::new_unique();
	let victim = Pubkey::new_unique();
	let referrer = Pubkey::new_unique();
	let state = Pubkey::new_unique();
	let payer = Pubkey::new_unique();
	let (accounts, old_data, old_lamports, payer_lamports) =
		migration_accounts(&mollusk, authority, referrer, state, payer, &victim);
	let instruction = update_instruction(
		authority,
		referrer,
		state,
		payer,
		&historical_update_data(88),
	);

	let result = mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(ProgramError::InvalidAccountData)],
	);
	assert_eq!(account(&result, &state).data, old_data);
	assert_eq!(account(&result, &state).lamports, old_lamports);
	assert_eq!(account(&result, &payer).lamports, payer_lamports);
}

#[test]
#[ignore = "requires the migrations_program SBF binary"]
fn malformed_historical_state_cannot_smuggle_a_trailing_byte() {
	let mollusk = create_mollusk();
	let authority = Pubkey::new_unique();
	let referrer = Pubkey::new_unique();
	let state = Pubkey::new_unique();
	let payer = Pubkey::new_unique();
	let (mut accounts, mut old_data, old_lamports, payer_lamports) =
		migration_accounts(&mollusk, authority, referrer, state, payer, &authority);
	old_data.push(0xff);
	accounts[2].1.data = old_data.clone();
	let instruction = update_instruction(
		authority,
		referrer,
		state,
		payer,
		&historical_update_data(88),
	);

	let result = mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(ProgramError::InvalidAccountData)],
	);
	assert_eq!(account(&result, &state).data, old_data);
	assert_eq!(account(&result, &state).lamports, old_lamports);
	assert_eq!(account(&result, &payer).lamports, payer_lamports);
}

#[test]
#[ignore = "requires the migrations_program SBF binary"]
fn self_cpi_migrates_callee_owned_state_and_caller_reloads_after_resize() {
	let mollusk = create_mollusk();
	let authority = Pubkey::new_unique();
	let referrer = Pubkey::new_unique();
	let state = Pubkey::new_unique();
	let payer = Pubkey::new_unique();
	let (mut accounts, ..) =
		migration_accounts(&mollusk, authority, referrer, state, payer, &authority);
	accounts.push((
		program_id(),
		create_program_account_loader_v3(&program_id()),
	));
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&relay_data(144),
		vec![
			AccountMeta::new_readonly(authority, true),
			AccountMeta::new_readonly(referrer, false),
			AccountMeta::new(state, false),
			AccountMeta::new(payer, true),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
			AccountMeta::new_readonly(program_id(), false),
		],
	);

	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);
	assert_current_state(&result, &state, 144);
}

#[test]
#[ignore = "requires the migrations_program SBF binary"]
fn migration_budget_is_checked_before_funding_or_resize() {
	let mollusk = create_mollusk();
	let authority = Pubkey::new_unique();
	let referrer = Pubkey::new_unique();
	let state = Pubkey::new_unique();
	let payer = Pubkey::new_unique();
	let (mut accounts, old_data, old_lamports, payer_lamports) =
		migration_accounts(&mollusk, authority, referrer, state, payer, &authority);
	let underfunded_lamports = old_lamports.saturating_sub(100_000);
	accounts[2].1.lamports = underfunded_lamports;
	let instruction = update_instruction(
		authority,
		referrer,
		state,
		payer,
		&historical_update_data(88),
	);

	let result = mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(PinaProgramError::MigrationBudgetExceeded.into())],
	);
	assert_eq!(account(&result, &state).data, old_data);
	assert_eq!(account(&result, &state).lamports, underfunded_lamports);
	assert_eq!(account(&result, &payer).lamports, payer_lamports);
}

#[test]
#[ignore = "requires the migrations_program SBF binary"]
fn foreign_owned_historical_bytes_are_not_a_migratable_account() {
	let mollusk = create_mollusk();
	let authority = Pubkey::new_unique();
	let referrer = Pubkey::new_unique();
	let state = Pubkey::new_unique();
	let payer = Pubkey::new_unique();
	let (mut accounts, old_data, old_lamports, payer_lamports) =
		migration_accounts(&mollusk, authority, referrer, state, payer, &authority);
	accounts[2].1.owner = solana_sdk_ids::system_program::id();
	let instruction = update_instruction(
		authority,
		referrer,
		state,
		payer,
		&historical_update_data(88),
	);

	let result = mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(ProgramError::InvalidAccountOwner)],
	);
	assert_eq!(account(&result, &state).data, old_data);
	assert_eq!(account(&result, &state).lamports, old_lamports);
	assert_eq!(account(&result, &payer).lamports, payer_lamports);
}

#[test]
#[ignore = "requires the migrations_program SBF binary"]
fn readonly_state_is_rejected_before_the_migration_runtime() {
	let mollusk = create_mollusk();
	let authority = Pubkey::new_unique();
	let referrer = Pubkey::new_unique();
	let state = Pubkey::new_unique();
	let payer = Pubkey::new_unique();
	let (accounts, old_data, old_lamports, payer_lamports) =
		migration_accounts(&mollusk, authority, referrer, state, payer, &authority);
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&historical_update_data(88),
		vec![
			AccountMeta::new_readonly(authority, true),
			AccountMeta::new_readonly(referrer, false),
			AccountMeta::new_readonly(state, false),
			AccountMeta::new(payer, true),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let result = mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(ProgramError::InvalidAccountData)],
	);
	assert_eq!(account(&result, &state).data, old_data);
	assert_eq!(account(&result, &state).lamports, old_lamports);
	assert_eq!(account(&result, &payer).lamports, payer_lamports);
}

#[test]
#[ignore = "requires the migrations_program SBF binary"]
fn migration_target_and_payer_may_not_alias() {
	let mollusk = create_mollusk();
	let authority = Pubkey::new_unique();
	let referrer = Pubkey::new_unique();
	let state = Pubkey::new_unique();
	let (mut accounts, old_data, old_lamports, _) =
		migration_accounts(&mollusk, authority, referrer, state, state, &authority);
	accounts.remove(3);
	let instruction = update_instruction(
		authority,
		referrer,
		state,
		state,
		&historical_update_data(88),
	);

	let result = mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(PinaProgramError::DuplicateMutableAccount.into())],
	);
	assert_eq!(account(&result, &state).data, old_data);
	assert_eq!(account(&result, &state).lamports, old_lamports);
}

#[test]
#[ignore = "requires the migrations_program SBF binary"]
fn future_account_version_is_rejected_without_trial_decoding() {
	let mollusk = create_mollusk();
	let authority = Pubkey::new_unique();
	let referrer = Pubkey::new_unique();
	let state = Pubkey::new_unique();
	let payer = Pubkey::new_unique();
	let (mut accounts, _, old_lamports, payer_lamports) =
		migration_accounts(&mollusk, authority, referrer, state, payer, &authority);
	let mut future_data = vec![0_u8; State::SIZE];
	future_data[0] = MigrationAccount::State as u8;
	future_data[1] = 2;
	future_data[2..34].copy_from_slice(authority.as_ref());
	accounts[2].1.data = future_data.clone();
	let instruction = update_instruction(
		authority,
		referrer,
		state,
		payer,
		&historical_update_data(88),
	);

	let result = mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(PinaProgramError::InvalidMigrationVersion.into())],
	);
	assert_eq!(account(&result, &state).data, future_data);
	assert_eq!(account(&result, &state).lamports, old_lamports);
	assert_eq!(account(&result, &payer).lamports, payer_lamports);
}
