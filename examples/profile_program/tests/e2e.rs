//! End-to-end tests for the profile_program.
//!
//! These tests exercise the full instruction flow through `mollusk-svm`:
//! Initialize, UpdateProfile, AddTag, and RemoveTag. Because the program only
//! CPIs to the system program (no token CPIs), the entire lifecycle —
//! including account creation — can be tested end-to-end.
//!
//! ## Prerequisites
//!
//! The profile_program must be compiled to an SBF binary before running these
//! tests:
//!
//! ```sh
//! cargo build --release --target bpfel-unknown-none -p profile_program \
//!     -Z build-std -F bpf-entrypoint
//! ```
//!
//! Then set `SBF_OUT_DIR` to the directory containing the `.so` file, or place
//! it in `tests/fixtures/`.
//!
//! ## Running
//!
//! ```sh
//! SBF_OUT_DIR=target/bpfel-unknown-none/release \
//!     cargo test -p profile_program --test e2e -- --nocapture
//! ```

use std::mem::size_of;

use mollusk_svm::Mollusk;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use mollusk_svm::result::InstructionResult;
use pina::PodU64;
use pina::ProgramError;
use profile_program::AddTagInstruction;
use profile_program::ID;
use profile_program::InitializeInstruction;
use profile_program::ProfileError;
use profile_program::ProfileInstruction;
use profile_program::ProfileState;
use profile_program::RemoveTagInstruction;
use profile_program::UpdateProfileInstruction;
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

/// Try to create a mollusk instance for the profile_program.
///
/// Returns `None` if the SBF binary cannot be found (e.g. the program has not
/// been compiled yet). This allows tests to be skipped gracefully without
/// triggering a panic-abort from the `no_std` panic handler.
fn try_create_mollusk() -> Option<Mollusk> {
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

	let found = search_dirs.iter().any(|dir| dir.join(so_name).is_file());
	if !found {
		return None;
	}

	Some(Mollusk::new(&program_id(), "profile_program"))
}

/// Derive the profile PDA for a given authority pubkey.
fn derive_profile_pda(authority: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"profile", authority.as_ref()], &program_id())
}

const SKIP_MSG: &str = "[SKIP] profile_program SBF binary not found. Build it first with `cargo \
                        build --release --target bpfel-unknown-none -p profile_program -Z \
                        build-std -F bpf-entrypoint`.";

// ---------------------------------------------------------------------------
// Instruction data builders
// ---------------------------------------------------------------------------

/// Build `Initialize` instruction data: discriminator + bump + name + bio.
fn initialize_ix_data(bump: u8, name: &str, bio: &str) -> Vec<u8> {
	assert!(name.len() <= 32, "name exceeds PodString<32> capacity");
	assert!(bio.len() <= 128, "bio exceeds PodString<128> capacity");
	let mut data = vec![ProfileInstruction::Initialize as u8, bump];
	data.extend_from_slice(&[name.len() as u8]);
	data.extend_from_slice(name.as_bytes());
	data.extend_from_slice(&vec![0u8; 32 - name.len()]);
	data.extend_from_slice(&[bio.len() as u8]);
	data.extend_from_slice(bio.as_bytes());
	data.extend_from_slice(&vec![0u8; 128 - bio.len()]);
	data
}

/// Build `UpdateProfile` instruction data: discriminator + name + bio.
fn update_profile_ix_data(name: &str, bio: &str) -> Vec<u8> {
	assert!(name.len() <= 32, "name exceeds PodString<32> capacity");
	assert!(bio.len() <= 128, "bio exceeds PodString<128> capacity");
	let mut data = vec![ProfileInstruction::UpdateProfile as u8];
	data.extend_from_slice(&[name.len() as u8]);
	data.extend_from_slice(name.as_bytes());
	data.extend_from_slice(&vec![0u8; 32 - name.len()]);
	data.extend_from_slice(&[bio.len() as u8]);
	data.extend_from_slice(bio.as_bytes());
	data.extend_from_slice(&vec![0u8; 128 - bio.len()]);
	data
}

/// Build `AddTag` instruction data: discriminator + tag.
fn add_tag_ix_data(tag: u64) -> Vec<u8> {
	let ix = AddTagInstruction::builder().tag(PodU64::from(tag)).build();
	ix.to_bytes().to_vec()
}

/// Build `RemoveTag` instruction data: discriminator + index.
fn remove_tag_ix_data(index: u64) -> Vec<u8> {
	let ix = RemoveTagInstruction::builder()
		.index(PodU64::from(index))
		.build();
	ix.to_bytes().to_vec()
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

/// The account set for the `Initialize` instruction.

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
	let state: &ProfileState =
		<ProfileState as pina::ZeroPodFixed>::from_bytes(&account.data).unwrap();
	assert_eq!(state.name.as_str(), expected_name);
	assert_eq!(state.bio.as_str(), expected_bio);
	let tags: Vec<u64> = state
		.tags
		.as_slice()
		.iter()
		.map(|t| u64::from(*t))
		.collect();
	assert_eq!(tags, expected_tags);
	assert_eq!(bool::from(state.active), expected_active);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Run the full lifecycle: Initialize → UpdateProfile → AddTag ×3 →
/// RemoveTag, verifying the stored `PodString`/`PodVec` state at each step.
///
/// Mollusk does not persist account state between
/// `process_and_validate_instruction` calls, so each instruction is fed the
/// previous result's accounts.
#[test]
fn full_lifecycle() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

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

/// `Initialize` must reject invalid UTF-8 in the name and leave no account
/// behind.
#[test]
fn initialize_rejects_invalid_utf8() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let authority = Pubkey::new_unique();
	let (profile, bump) = derive_profile_pda(&authority);

	// Name with an invalid UTF-8 byte (0xff)
	let mut data = vec![ProfileInstruction::Initialize as u8, bump];
	data.extend_from_slice(&[1u8, 0xff]);
	data.extend_from_slice(&vec![0u8; 31]);
	data.extend_from_slice(&vec![0u8; 129]);

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
	// Account must not have been created
	assert!(result.get_account(&profile).is_none());
}

/// `AddTag` must reject a 9th tag once the `PodVec` capacity (8) is reached.
#[test]
fn add_tag_rejects_overflow() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

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
fn remove_tag_rejects_out_of_range() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

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
fn requires_signer() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

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
	assert_eq!(size_of::<ProfileState>(), 231);
	assert_eq!(size_of::<InitializeInstruction>(), 164);
	assert_eq!(size_of::<UpdateProfileInstruction>(), 163);
	assert_eq!(size_of::<AddTagInstruction>(), 9);
	assert_eq!(size_of::<RemoveTagInstruction>(), 9);
}
