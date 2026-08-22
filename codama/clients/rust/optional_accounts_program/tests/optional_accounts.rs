//! Client-level tests for optional account slots in the generated Rust
//! client.
//!
//! These tests lock in the emitted account-meta contract:
//!
//! - Omitted optional accounts keep the account count fixed by emitting a
//!   readonly meta that points at the executing program's own address.
//! - Provided optional accounts emit their declared writability/signer roles.
//! - `Init::new` keeps deriving PDA defaults for required accounts.

use optional_accounts_program_client::generated::instructions::Init;
use optional_accounts_program_client::generated::instructions::InitInstructionData;
use optional_accounts_program_client::generated::instructions::Inspect;
use optional_accounts_program_client::generated::instructions::InspectInstructionData;
use optional_accounts_program_client::generated::instructions::Note;
use optional_accounts_program_client::generated::instructions::NoteInstructionData;
use optional_accounts_program_client::generated::instructions::Touch;
use optional_accounts_program_client::generated::instructions::TouchInstructionData;
use solana_instruction::AccountMeta;
use solana_pubkey::Pubkey;

const AUTHORITY: Pubkey = Pubkey::new_from_array([7u8; 32]);
const STORE: Pubkey = Pubkey::new_from_array([9u8; 32]);
const WITNESS: Pubkey = Pubkey::new_from_array([11u8; 32]);
const NOTE: Pubkey = Pubkey::new_from_array([13u8; 32]);

#[test]
fn omitted_optional_mutable_account_fills_with_readonly_program_id() {
	let ix = Touch::new(AUTHORITY).instruction(TouchInstructionData::new(|_| {}).unwrap());

	assert_eq!(ix.accounts.len(), 2);
	assert_eq!(ix.accounts[0].pubkey, AUTHORITY);
	assert!(ix.accounts[0].is_signer);
	assert!(!ix.accounts[0].is_writable);

	// The filler slot points at the executing program and stays readonly so
	// the transaction remains valid.
	assert_eq!(
		ix.accounts[1].pubkey,
		optional_accounts_program_client::generated::programs::OPTIONAL_ACCOUNTS_PROGRAM_ID
	);
	assert!(!ix.accounts[1].is_signer);
	assert!(!ix.accounts[1].is_writable);
}

#[test]
fn provided_optional_mutable_account_emits_writable_meta() {
	let mut touch = Touch::new(AUTHORITY);
	touch.store = Some(STORE);
	let ix = touch.instruction(TouchInstructionData::new(|_| {}).unwrap());

	assert_eq!(ix.accounts.len(), 2);
	assert_eq!(ix.accounts[1].pubkey, STORE);
	assert!(!ix.accounts[1].is_signer);
	assert!(ix.accounts[1].is_writable);
}

#[test]
fn inspect_covers_every_optional_presence_combination() {
	let build = |store: Option<Pubkey>, witness: Option<Pubkey>| {
		let mut inspect = Inspect::new(AUTHORITY);
		inspect.store = store;
		inspect.witness = witness;
		inspect.instruction(InspectInstructionData::new(|_| {}).unwrap())
	};

	// Both absent: fixed three-slot layout with program-address fillers.
	let absent_ix = build(None, None);
	assert_eq!(absent_ix.accounts.len(), 3);
	for meta in &absent_ix.accounts[1..] {
		assert_eq!(
			meta.pubkey,
			optional_accounts_program_client::generated::programs::OPTIONAL_ACCOUNTS_PROGRAM_ID
		);
		assert!(!meta.is_writable);
		assert!(!meta.is_signer);
	}

	// Store present only: writable store slot, filler witness.
	let store_only = build(Some(STORE), None);
	assert_eq!(store_only.accounts[1].pubkey, STORE);
	assert!(!store_only.accounts[1].is_signer);
	assert_eq!(
		store_only.accounts[2].pubkey,
		optional_accounts_program_client::generated::programs::OPTIONAL_ACCOUNTS_PROGRAM_ID
	);

	// Witness present only: the signer assertion is reflected in the IDL so
	// clients mark it as a readonly signer when provided.
	let witness_only = build(None, Some(WITNESS));
	assert_eq!(
		witness_only.accounts[1].pubkey,
		optional_accounts_program_client::generated::programs::OPTIONAL_ACCOUNTS_PROGRAM_ID
	);
	assert_eq!(witness_only.accounts[2].pubkey, WITNESS);
	assert!(witness_only.accounts[2].is_signer);
	assert!(!witness_only.accounts[2].is_writable);

	// Both present: each slot retains its independently inferred privileges.
	let both_present = build(Some(STORE), Some(WITNESS));
	assert_eq!(both_present.accounts[1].pubkey, STORE);
	assert!(!both_present.accounts[1].is_writable);
	assert!(!both_present.accounts[1].is_signer);
	assert_eq!(both_present.accounts[2].pubkey, WITNESS);
	assert!(both_present.accounts[2].is_signer);
	assert!(!both_present.accounts[2].is_writable);
}

#[test]
fn note_accepts_arbitrary_readonly_accounts_or_nothing() {
	let mut with_note = Note::new(AUTHORITY);
	with_note.note = Some(NOTE);
	let provided_ix = with_note.instruction(NoteInstructionData::new(|_| {}).unwrap());

	assert_eq!(provided_ix.accounts.len(), 2);
	assert_eq!(provided_ix.accounts[1].pubkey, NOTE);
	assert!(!provided_ix.accounts[1].is_writable);
	assert!(!provided_ix.accounts[1].is_signer);

	let without_ix = Note::new(AUTHORITY).instruction(NoteInstructionData::new(|_| {}).unwrap());
	assert_eq!(without_ix.accounts.len(), 2);
	assert_eq!(
		without_ix.accounts[1].pubkey,
		optional_accounts_program_client::generated::programs::OPTIONAL_ACCOUNTS_PROGRAM_ID
	);
	assert!(!without_ix.accounts[1].is_writable);
}

#[test]
fn init_keeps_the_required_baseline_layout_with_derived_defaults() {
	let ix = Init::new(AUTHORITY).instruction(InitInstructionData::new(|_| {}).unwrap());

	assert_eq!(ix.accounts.len(), 3);
	assert!(ix.accounts[0].is_signer && ix.accounts[0].is_writable);
	assert!(ix.accounts[1].is_writable && !ix.accounts[1].is_signer);
	assert!(!ix.accounts[2].is_writable && !ix.accounts[2].is_signer);

	// The store default is the canonical PDA for the authority.
	let (expected_pda, _bump) = Pubkey::find_program_address(
		&["store".as_bytes(), AUTHORITY.as_ref()],
		&optional_accounts_program_client::generated::programs::OPTIONAL_ACCOUNTS_PROGRAM_ID,
	);
	assert_eq!(ix.accounts[1].pubkey, expected_pda);
	assert_eq!(ix.accounts[0].pubkey, AUTHORITY);
}

#[test]
fn discriminators_match_the_program_layouts() {
	assert_eq!(
		optional_accounts_program_client::generated::instructions::INIT_DISCRIMINATOR,
		0u8
	);
	assert_eq!(
		optional_accounts_program_client::generated::instructions::TOUCH_DISCRIMINATOR,
		1u8
	);
	assert_eq!(
		optional_accounts_program_client::generated::instructions::INSPECT_DISCRIMINATOR,
		2u8
	);
	assert_eq!(
		optional_accounts_program_client::generated::instructions::NOTE_DISCRIMINATOR,
		3u8
	);
}

/// Keep an explicit handle on the meta type used throughout these tests so
/// refactors of the generated client surface show up here first.
#[test]
fn account_meta_shape_is_stable() {
	let meta = AccountMeta::new(STORE, false);
	assert!(meta.is_writable);
	assert!(!meta.is_signer);
}
