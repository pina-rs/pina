//! SBF regressions for the authenticated `anchor_realloc` lifecycle.
//!
//! Build the program before running these ignored tests:
//!
//! ```sh
//! cargo build-sbf --manifest-path examples/anchor_realloc/Cargo.toml \
//!     --sbf-out-dir target/deploy --features bpf-entrypoint
//! SBF_OUT_DIR=target/deploy \
//!     cargo test -p anchor_realloc --test e2e -- --include-ignored
//! ```

use anchor_realloc::ID;
use anchor_realloc::InitializeIx;
use anchor_realloc::Realloc2Ix;
use anchor_realloc::ReallocError;
use anchor_realloc::ReallocIx;
use anchor_realloc::Sample;
use anchor_realloc::SampleZc;
use mollusk_svm::Mollusk;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use mollusk_svm::result::InstructionResult;
use pina::ProgramError;
use pina::ZeroPodFixed;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

fn program_id() -> Pubkey {
	let bytes: &[u8] = ID.as_ref();
	let array: [u8; 32] = bytes
		.try_into()
		.unwrap_or_else(|_| panic!("program address must be 32 bytes"));
	Pubkey::new_from_array(array)
}

fn create_mollusk() -> Mollusk {
	let so_name = "anchor_realloc.so";
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
		"anchor_realloc SBF binary not found; build it before running ignored e2e tests"
	);

	Mollusk::new(&program_id(), "anchor_realloc")
}

fn derive_sample(authority: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"sample", authority.as_ref()], &program_id())
}

fn initialize_ix_data(bump: u8) -> Vec<u8> {
	let mut data = vec![0u8; InitializeIx::SIZE];
	InitializeIx::initialize(&mut data)
		.unwrap_or_else(|error| panic!("initialize instruction encoding failed: {error:?}"))
		.bump = bump;
	data
}

fn realloc_ix_data(len: usize) -> Vec<u8> {
	let len = u16::try_from(len).unwrap_or_else(|_| panic!("test length does not fit u16"));
	let mut data = vec![0u8; ReallocIx::SIZE];
	ReallocIx::initialize(&mut data)
		.unwrap_or_else(|error| panic!("realloc instruction encoding failed: {error:?}"))
		.len
		.set(len);
	data
}

fn realloc2_ix_data(len: usize) -> Vec<u8> {
	let len = u16::try_from(len).unwrap_or_else(|_| panic!("test length does not fit u16"));
	let mut data = vec![0u8; Realloc2Ix::SIZE];
	Realloc2Ix::initialize(&mut data)
		.unwrap_or_else(|error| panic!("realloc2 instruction encoding failed: {error:?}"))
		.len
		.set(len);
	data
}

fn initialize_accounts(authority: &Pubkey, sample: &Pubkey) -> Vec<(Pubkey, Account)> {
	vec![
		(
			*authority,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(*sample, Account::default()),
		keyed_account_for_system_program(),
	]
}

fn initialize_sample(
	mollusk: &Mollusk,
	authority: &Pubkey,
	sample: &Pubkey,
	bump: u8,
) -> InstructionResult {
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&initialize_ix_data(bump),
		vec![
			AccountMeta::new(*authority, true),
			AccountMeta::new(*sample, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	mollusk.process_and_validate_instruction(
		&instruction,
		&initialize_accounts(authority, sample),
		&[Check::success()],
	)
}

fn assert_sample(result: &InstructionResult, sample: &Pubkey, authority: &Pubkey, len: usize) {
	let account = result
		.get_account(sample)
		.unwrap_or_else(|| panic!("sample account {sample} not found"));
	assert_eq!(account.owner, program_id());
	assert_eq!(account.data.len(), len);

	let header = account
		.data
		.get(..Sample::SIZE)
		.unwrap_or_else(|| panic!("sample account is shorter than its header"));
	let state: &SampleZc = <Sample as ZeroPodFixed>::from_bytes(header)
		.unwrap_or_else(|error| panic!("sample header must remain valid: {error:?}"));
	assert_eq!(state.authority, authority.to_bytes().into());
}

/// The supported lifecycle creates an authority-bound PDA, grows it with
/// rent-aware reallocation, then shrinks it while preserving its header.
#[test]
#[ignore = "requires the anchor_realloc SBF binary"]
fn authority_can_initialize_grow_and_shrink_its_sample() {
	let mollusk = create_mollusk();
	let authority = Pubkey::new_unique();
	let (sample, bump) = derive_sample(&authority);
	let mut result = initialize_sample(&mollusk, &authority, &sample, bump);

	assert_sample(&result, &sample, &authority, Sample::SIZE);

	let grown_len = Sample::SIZE + 64;
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&realloc_ix_data(grown_len),
		vec![
			AccountMeta::new(authority, true),
			AccountMeta::new(sample, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);
	let accounts = result.resulting_accounts;
	result = mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);
	assert_sample(&result, &sample, &authority, grown_len);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&realloc_ix_data(Sample::SIZE),
		vec![
			AccountMeta::new(authority, true),
			AccountMeta::new(sample, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);
	result = mollusk.process_and_validate_instruction(
		&instruction,
		&result.resulting_accounts,
		&[Check::success()],
	);
	assert_sample(&result, &sample, &authority, Sample::SIZE);
}

/// An attacker cannot use their signer to resize a victim's authenticated
/// sample: the victim address is not the PDA derived for the attacker.
#[test]
#[ignore = "requires the anchor_realloc SBF binary"]
fn unrelated_signer_cannot_resize_a_victim_sample() {
	let mollusk = create_mollusk();
	let victim = Pubkey::new_unique();
	let attacker = Pubkey::new_unique();
	let (victim_sample, bump) = derive_sample(&victim);
	let result = initialize_sample(&mollusk, &victim, &victim_sample, bump);
	let original_len = result
		.get_account(&victim_sample)
		.unwrap_or_else(|| panic!("victim sample missing"))
		.data
		.len();

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&realloc_ix_data(Sample::SIZE + 64),
		vec![
			AccountMeta::new(attacker, true),
			AccountMeta::new(victim_sample, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);
	let mut accounts = result.resulting_accounts;
	accounts.push((
		attacker,
		Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
	));
	let rejected = mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(ProgramError::InvalidSeeds)],
	);
	let sample = rejected
		.get_account(&victim_sample)
		.unwrap_or_else(|| panic!("victim sample missing after rejection"));
	assert_eq!(sample.data.len(), original_len);
}

/// A program-owned, correctly typed account is still rejected unless it is the
/// canonical PDA for the signer. This is the regression for the prior
/// arbitrary-program-owned-account resize vulnerability.
#[test]
#[ignore = "requires the anchor_realloc SBF binary"]
fn canonical_pda_check_rejects_an_arbitrary_program_owned_sample() {
	let mollusk = create_mollusk();
	let authority = Pubkey::new_unique();
	let forged_sample = Pubkey::new_unique();
	let mut data = vec![0u8; Sample::SIZE];
	let state = Sample::initialize(&mut data)
		.unwrap_or_else(|error| panic!("sample data setup failed: {error:?}"));
	state.bump = 0;
	state.authority = authority.to_bytes().into();

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&realloc_ix_data(Sample::SIZE + 64),
		vec![
			AccountMeta::new(authority, true),
			AccountMeta::new(forged_sample, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);
	let accounts = vec![
		(
			authority,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(
			forged_sample,
			Account::new(1_000_000, data.len(), &program_id()),
		),
		keyed_account_for_system_program(),
	];
	let mut accounts = accounts;
	accounts[1].1.data = data.clone();

	let rejected = mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(ProgramError::InvalidSeeds)],
	);
	let sample = rejected
		.get_account(&forged_sample)
		.unwrap_or_else(|| panic!("forged sample missing after rejection"));
	assert_eq!(sample.data, data);
}

/// Realloc2 intentionally retains Anchor's duplicate-account failure. It
/// validates the authenticated target first, then rejects before mutation.
#[test]
#[ignore = "requires the anchor_realloc SBF binary"]
fn realloc2_rejects_duplicate_authenticated_targets_without_mutation() {
	let mollusk = create_mollusk();
	let authority = Pubkey::new_unique();
	let (sample, bump) = derive_sample(&authority);
	let result = initialize_sample(&mollusk, &authority, &sample, bump);
	let original_data = result
		.get_account(&sample)
		.unwrap_or_else(|| panic!("sample missing"))
		.data
		.clone();

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&realloc2_ix_data(Sample::SIZE + 64),
		vec![
			AccountMeta::new(authority, true),
			AccountMeta::new(sample, false),
			AccountMeta::new(sample, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);
	let rejected = mollusk.process_and_validate_instruction(
		&instruction,
		&result.resulting_accounts,
		&[Check::err(ProgramError::Custom(
			ReallocError::AccountDuplicateReallocs as u32,
		))],
	);
	let unchanged = rejected
		.get_account(&sample)
		.unwrap_or_else(|| panic!("sample missing after rejection"));
	assert_eq!(unchanged.data, original_data);
}
