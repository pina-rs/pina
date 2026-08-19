//! End-to-end tests for the `profile_program`.
//!
//! These tests exercise the full instruction flow through `mollusk-svm`:
//! `Initialize`, `UpdateProfile`, `AddTag`, and `RemoveTag`. Because the program only
//! CPIs to the system program (no token CPIs), the entire lifecycle —
//! including account creation — can be tested end-to-end.
//!
//! ## Prerequisites
//!
//! The `profile_program` must be compiled to an SBF binary before running these
//! tests:
//!
//! ```sh
//! cargo build-sbf --manifest-path examples/profile_program/Cargo.toml \
//!     --sbf-out-dir target/deploy --features bpf-entrypoint
//! ```
//!
//! Then set `SBF_OUT_DIR` to the directory containing the `.so` file, or place
//! it in `tests/fixtures/`.
//!
//! ## Running
//!
//! ```sh
//! SBF_OUT_DIR=target/deploy \
//!     cargo test -p profile_program --test e2e -- --include-ignored --nocapture
//! ```

use mollusk_svm::Mollusk;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use mollusk_svm::result::InstructionResult;
use pina::ProgramError;
use profile_program::AddTagInstruction;
use profile_program::ID;
use profile_program::InitializeInstruction;
use profile_program::ProfileError;
use profile_program::ProfileInstruction;
use profile_program::ProfileState;
use profile_program::ProfileStateZc;
use profile_program::RemoveTagInstruction;
use profile_program::UpdateProfileInstruction;
use profile_program::encode_bounded_text;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn program_id() -> Pubkey {
	let id = ID;
	let bytes: &[u8] = id.as_ref();
	let array: [u8; 32] = bytes
		.try_into()
		.unwrap_or_else(|_| panic!("address must be 32 bytes"));
	Pubkey::new_from_array(array)
}

/// Create a Mollusk instance for the compiled profile program.
fn create_mollusk() -> Mollusk {
	let so_name = "profile_program.so";
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
		"profile_program SBF binary not found; build it before running ignored e2e tests"
	);

	Mollusk::new(&program_id(), "profile_program")
}

/// Derive the profile PDA for a given authority pubkey.
fn derive_profile_pda(authority: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"profile", authority.as_ref()], &program_id())
}

// ---------------------------------------------------------------------------
// Instruction data builders
// ---------------------------------------------------------------------------

/// Build `Initialize` instruction data: discriminator + bump + name + bio.
fn initialize_ix_data(bump: u8, name: &str, bio: &str) -> Vec<u8> {
	let name = encode_bounded_text::<33>(name)
		.unwrap_or_else(|error| panic!("invalid profile name: {error:?}"));
	let bio = encode_bounded_text::<129>(bio)
		.unwrap_or_else(|error| panic!("invalid profile bio: {error:?}"));
	let mut data = vec![ProfileInstruction::Initialize as u8, bump];
	data.extend_from_slice(&name);
	data.extend_from_slice(&bio);
	data
}

/// Build `UpdateProfile` instruction data: discriminator + name + bio.
fn update_profile_ix_data(name: &str, bio: &str) -> Vec<u8> {
	let name = encode_bounded_text::<33>(name)
		.unwrap_or_else(|error| panic!("invalid profile name: {error:?}"));
	let bio = encode_bounded_text::<129>(bio)
		.unwrap_or_else(|error| panic!("invalid profile bio: {error:?}"));
	let mut data = vec![ProfileInstruction::UpdateProfile as u8];
	data.extend_from_slice(&name);
	data.extend_from_slice(&bio);
	data
}

/// Build `AddTag` instruction data: discriminator + tag.
fn add_tag_ix_data(tag: u64) -> Vec<u8> {
	let mut data = vec![0u8; AddTagInstruction::SIZE];
	AddTagInstruction::initialize(&mut data)
		.unwrap_or_else(|error| panic!("add-tag initialization failed: {error:?}"))
		.tag
		.set(tag);
	data
}

/// Build `RemoveTag` instruction data: discriminator + index.
fn remove_tag_ix_data(index: u64) -> Vec<u8> {
	let mut data = vec![0u8; RemoveTagInstruction::SIZE];
	RemoveTagInstruction::initialize(&mut data)
		.unwrap_or_else(|error| panic!("remove-tag initialization failed: {error:?}"))
		.index
		.set(index);
	data
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// The account set for the `Initialize` instruction.
fn initialize_accounts(authority: &Pubkey, profile: &Pubkey) -> Vec<(Pubkey, Account)> {
	vec![
		(
			*authority,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(*profile, Account::default()),
		keyed_account_for_system_program(),
	]
}

/// Assert that a profile's name/bio/tags match expectations.
fn assert_profile(
	result: &InstructionResult,
	profile: &Pubkey,
	expected_name: &str,
	expected_bio: &str,
	expected_tags: &[u64],
	expected_active: bool,
) {
	let account = result
		.get_account(profile)
		.unwrap_or_else(|| panic!("profile account {profile} not found"));
	let state: &ProfileStateZc = ProfileState::try_from_bytes(&account.data).unwrap();
	assert_eq!(state.name_text().unwrap(), expected_name);
	assert_eq!(state.bio_text().unwrap(), expected_bio);
	let tags: Vec<u64> = (0..state.tag_count().unwrap())
		.map(|index| state.tag(index).unwrap().unwrap())
		.collect();
	assert_eq!(tags, expected_tags);
	assert!(state.favorite_tag.is_none());
	assert_eq!(state.active.get(), expected_active);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Run the full lifecycle: `Initialize` → `UpdateProfile` → `AddTag` ×3 →
/// `RemoveTag`, verifying the bounded text and tag state at each step.
///
/// Mollusk does not persist account state between
/// `process_and_validate_instruction` calls, so each instruction is fed the
/// previous result's accounts.
#[test]
#[ignore = "requires the profile_program SBF binary"]
fn full_lifecycle() {
	let mollusk = create_mollusk();

	let authority = Pubkey::new_unique();
	let (profile, bump) = derive_profile_pda(&authority);

	// Initialize
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&initialize_ix_data(bump, "alice", "hello world"),
		vec![
			AccountMeta::new(authority, true),
			AccountMeta::new(profile, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);
	let mut accounts = initialize_accounts(&authority, &profile);
	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);
	assert!(
		result.program_result.is_ok(),
		"initialize failed: {:?}",
		result.program_result
	);
	assert_profile(&result, &profile, "alice", "hello world", &[], true);

	// UpdateProfile
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&update_profile_ix_data("alice2", "updated bio"),
		vec![
			AccountMeta::new_readonly(authority, true),
			AccountMeta::new(profile, false),
		],
	);
	accounts = result.resulting_accounts;
	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);
	assert!(
		result.program_result.is_ok(),
		"update failed: {:?}",
		result.program_result
	);
	assert_profile(&result, &profile, "alice2", "updated bio", &[], true);

	// AddTag ×3
	let mut result = result;
	for tag in [10u64, 20, 30] {
		let instruction = Instruction::new_with_bytes(
			program_id(),
			&add_tag_ix_data(tag),
			vec![
				AccountMeta::new_readonly(authority, true),
				AccountMeta::new(profile, false),
			],
		);
		accounts = result.resulting_accounts;
		result =
			mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);
		assert!(
			result.program_result.is_ok(),
			"add tag {tag} failed: {:?}",
			result.program_result
		);
	}
	assert_profile(
		&result,
		&profile,
		"alice2",
		"updated bio",
		&[10, 20, 30],
		true,
	);

	// RemoveTag (middle element)
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&remove_tag_ix_data(1),
		vec![
			AccountMeta::new_readonly(authority, true),
			AccountMeta::new(profile, false),
		],
	);
	accounts = result.resulting_accounts;
	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);
	assert!(
		result.program_result.is_ok(),
		"remove tag failed: {:?}",
		result.program_result
	);
	assert_profile(&result, &profile, "alice2", "updated bio", &[10, 30], true);
}

/// `Initialize` must reject invalid UTF-8 in the name without creating the
/// profile account.
#[test]
#[ignore = "requires the profile_program SBF binary"]
fn initialize_rejects_invalid_utf8() {
	let mollusk = create_mollusk();

	let authority = Pubkey::new_unique();
	let (profile, bump) = derive_profile_pda(&authority);

	// Name with an invalid UTF-8 byte (0xff)
	let mut data = vec![ProfileInstruction::Initialize as u8, bump];
	data.extend_from_slice(&[1u8, 0xff]);
	data.extend_from_slice(&[0u8; 31]);
	data.extend_from_slice(&[0u8; 129]);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&data,
		vec![
			AccountMeta::new(authority, true),
			AccountMeta::new(profile, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);
	let result = mollusk.process_and_validate_instruction(
		&instruction,
		&initialize_accounts(&authority, &profile),
		&[Check::err(ProfileError::InvalidUtf8.into())],
	);
	// Mollusk retains the input placeholder account after a failed instruction.
	let account = result
		.get_account(&profile)
		.unwrap_or_else(|| panic!("profile input account missing"));
	assert_eq!(account.lamports, 0);
	assert!(account.data.is_empty());
	assert_eq!(account.owner, solana_sdk_ids::system_program::id());
}

/// `AddTag` must reject a 9th tag once the bounded capacity (8) is reached.
#[test]
#[ignore = "requires the profile_program SBF binary"]
fn add_tag_rejects_overflow() {
	let mollusk = create_mollusk();

	let authority = Pubkey::new_unique();
	let (profile, bump) = derive_profile_pda(&authority);

	// Initialize
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&initialize_ix_data(bump, "alice", ""),
		vec![
			AccountMeta::new(authority, true),
			AccountMeta::new(profile, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);
	let mut accounts = initialize_accounts(&authority, &profile);
	let mut result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);
	assert!(
		result.program_result.is_ok(),
		"initialize failed: {:?}",
		result.program_result
	);

	// Fill the tag list to capacity (8)
	for tag in 0..8u64 {
		let instruction = Instruction::new_with_bytes(
			program_id(),
			&add_tag_ix_data(tag),
			vec![
				AccountMeta::new_readonly(authority, true),
				AccountMeta::new(profile, false),
			],
		);
		accounts = result.resulting_accounts;
		result =
			mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);
		assert!(
			result.program_result.is_ok(),
			"add tag {tag} failed: {:?}",
			result.program_result
		);
	}

	// The 9th tag overflows
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&add_tag_ix_data(99),
		vec![
			AccountMeta::new_readonly(authority, true),
			AccountMeta::new(profile, false),
		],
	);
	accounts = result.resulting_accounts;
	let _result = mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(ProfileError::TagOverflow.into())],
	);
}

/// `RemoveTag` must reject an index into an empty tag list.
#[test]
#[ignore = "requires the profile_program SBF binary"]
fn remove_tag_rejects_out_of_range() {
	let mollusk = create_mollusk();

	let authority = Pubkey::new_unique();
	let (profile, bump) = derive_profile_pda(&authority);

	// Initialize
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&initialize_ix_data(bump, "alice", ""),
		vec![
			AccountMeta::new(authority, true),
			AccountMeta::new(profile, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);
	let mut accounts = initialize_accounts(&authority, &profile);
	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);
	assert!(
		result.program_result.is_ok(),
		"initialize failed: {:?}",
		result.program_result
	);

	// Remove from an empty list
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&remove_tag_ix_data(0),
		vec![
			AccountMeta::new_readonly(authority, true),
			AccountMeta::new(profile, false),
		],
	);
	accounts = result.resulting_accounts;
	let _result = mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(ProfileError::TagNotFound.into())],
	);
}

/// `Initialize` must require the authority's signature.
#[test]
#[ignore = "requires the profile_program SBF binary"]
fn requires_signer() {
	let mollusk = create_mollusk();

	let authority = Pubkey::new_unique();
	let (profile, bump) = derive_profile_pda(&authority);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&initialize_ix_data(bump, "alice", ""),
		vec![
			AccountMeta::new(authority, false),
			AccountMeta::new(profile, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);
	let _result = mollusk.process_and_validate_instruction(
		&instruction,
		&initialize_accounts(&authority, &profile),
		&[Check::err(ProgramError::MissingRequiredSignature)],
	);
}

/// The account and instruction layouts must match the documented sizes.
#[test]
fn account_layout_matches_state() {
	assert_eq!(ProfileState::SIZE, 240);
	assert_eq!(InitializeInstruction::SIZE, 164);
	assert_eq!(UpdateProfileInstruction::SIZE, 163);
	assert_eq!(AddTagInstruction::SIZE, 9);
	assert_eq!(RemoveTagInstruction::SIZE, 9);
}
