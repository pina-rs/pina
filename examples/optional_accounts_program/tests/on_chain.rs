//! On-chain execution tests for optional account slots.
//!
//! These tests run the compiled SBF program through `mollusk-svm` and verify
//! the full optional-account contract at the runtime level:
//!
//! - Omitted optional slots keep the account count fixed and parse as `None`.
//! - Provided optional mutable slots mutate state only when present.
//! - Optional signers are enforced when provided, skippable when omitted.
//!
//! ## Prerequisites
//!
//! The program must be compiled to an SBF binary first:
//!
//! ```sh
//! devenv shell -- cargo build-optional-accounts-program
//! ```
//!
//! Then set `SBF_OUT_DIR`, or place the `.so` in `tests/fixtures/`.

use mollusk_svm::Mollusk;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

const PROGRAM_ID: Pubkey = Pubkey::new_from_array([
	0x09, 0x1f, 0x9c, 0x91, 0xe1, 0xc7, 0xd6, 0x6f, 0x43, 0x97, 0x82, 0x72, 0x49, 0xaf, 0x65, 0x8b,
	0x9a, 0xf7, 0x8d, 0xe0, 0x55, 0x00, 0x69, 0xf4, 0x42, 0x44, 0x30, 0x63, 0xef, 0xd2, 0xb1, 0x3b,
]);

/// Discriminators mirror `OptionalInstruction` in the program crate.
const INIT: u8 = 0;
const TOUCH: u8 = 1;
const INSPECT: u8 = 2;

fn create_mollusk() -> Mollusk {
	let so_name = "optional_accounts_program.so";
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
		"optional_accounts_program SBF binary not found; build it before running ignored on-chain \
		 tests"
	);

	Mollusk::new(&PROGRAM_ID, "optional_accounts_program")
}

fn derive_store(authority: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"store", authority.as_ref()], &PROGRAM_ID)
}

fn init_ix(authority: &Pubkey, store: &Pubkey, bump: u8) -> Instruction {
	Instruction::new_with_bytes(
		PROGRAM_ID,
		&[INIT, bump],
		vec![
			AccountMeta::new(*authority, true),
			AccountMeta::new(*store, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	)
}

fn touch_ix(authority: &Pubkey, store: Option<&Pubkey>) -> Instruction {
	let store_meta = match store {
		Some(store) => AccountMeta::new(*store, false),
		// Exactly what the generated clients emit for omitted optional
		// slots: a readonly meta holding the program ID.
		None => AccountMeta::new_readonly(PROGRAM_ID, false),
	};

	Instruction::new_with_bytes(
		PROGRAM_ID,
		&[TOUCH],
		vec![AccountMeta::new_readonly(*authority, true), store_meta],
	)
}

fn inspect_ix(authority: &Pubkey, store: Option<&Pubkey>, witness: Option<&Pubkey>) -> Instruction {
	let mut metas = vec![
		AccountMeta::new_readonly(*authority, true),
		match store {
			Some(store) => AccountMeta::new_readonly(*store, false),
			None => AccountMeta::new_readonly(PROGRAM_ID, false),
		},
	];
	match witness {
		Some(witness) => metas.push(AccountMeta::new_readonly(*witness, true)),
		None => metas.push(AccountMeta::new_readonly(PROGRAM_ID, false)),
	}
	Instruction::new_with_bytes(PROGRAM_ID, &[INSPECT], metas)
}

fn payer_account() -> Account {
	Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id())
}

fn store_account(count: u64, lamports: u64) -> Account {
	let mut data = vec![0u8; 10];
	data[0] = 1; // StoreState discriminator.
	data[2..10].copy_from_slice(&count.to_le_bytes());
	Account {
		lamports,
		data,
		owner: PROGRAM_ID,
		executable: false,
		rent_epoch: 0,
	}
}

#[test]
#[ignore = "requires the optional_accounts_program SBF binary"]
fn touch_with_omitted_optional_slot_parses_as_none_on_chain() {
	let mollusk = create_mollusk();

	let authority = Pubkey::new_unique();
	let (store, bump) = derive_store(&authority);
	let lamports = mollusk.sysvars.rent.minimum_balance(10);

	mollusk.process_and_validate_instruction(
		&init_ix(&authority, &store, bump),
		&vec![
			(authority, payer_account()),
			(store, Account::default()),
			keyed_account_for_system_program(),
		],
		&[Check::success()],
	);

	// Touch with the slot filled by the readonly program address must
	// succeed and leave the counter untouched.
	let result = mollusk.process_and_validate_instruction(
		&touch_ix(&authority, None),
		&vec![
			(authority, payer_account()),
			(store, store_account(0, lamports)),
		],
		&[Check::success()],
	);

	let stored = &result
		.resulting_accounts
		.iter()
		.find(|(key, _)| *key == store)
		.expect("store account in results")
		.1;
	assert_eq!(stored.data[2..10], 0u64.to_le_bytes());
}

#[test]
#[ignore = "requires the optional_accounts_program SBF binary"]
fn touch_with_provided_optional_slot_increments_the_counter() {
	let mollusk = create_mollusk();

	let authority = Pubkey::new_unique();
	let (store, bump) = derive_store(&authority);
	let lamports = mollusk.sysvars.rent.minimum_balance(10);

	mollusk.process_and_validate_instruction(
		&init_ix(&authority, &store, bump),
		&vec![
			(authority, payer_account()),
			(store, Account::default()),
			keyed_account_for_system_program(),
		],
		&[Check::success()],
	);

	let result = mollusk.process_and_validate_instruction(
		&touch_ix(&authority, Some(&store)),
		&vec![
			(authority, payer_account()),
			(store, store_account(0, lamports)),
		],
		&[Check::success()],
	);

	let stored = &result
		.resulting_accounts
		.iter()
		.find(|(key, _)| *key == store)
		.expect("store account in results")
		.1;
	assert_eq!(stored.data[2..10], 1u64.to_le_bytes());
}

#[test]
#[ignore = "requires the optional_accounts_program SBF binary"]
fn inspect_enforces_signer_only_when_witness_is_provided() {
	let mollusk = create_mollusk();

	let authority = Pubkey::new_unique();
	let unsigned_witness = Pubkey::new_unique();
	let (store, bump) = derive_store(&authority);
	let lamports = mollusk.sysvars.rent.minimum_balance(10);

	mollusk.process_and_validate_instruction(
		&init_ix(&authority, &store, bump),
		&vec![
			(authority, payer_account()),
			(store, Account::default()),
			keyed_account_for_system_program(),
		],
		&[Check::success()],
	);

	// Omitted witness succeeds.
	mollusk.process_and_validate_instruction(
		&inspect_ix(&authority, Some(&store), None),
		&vec![
			(authority, payer_account()),
			(store, store_account(0, lamports)),
		],
		&[Check::success()],
	);

	// An unsigned witness in the optional slot is rejected by the runtime.
	mollusk.process_and_validate_instruction(
		&inspect_ix(&authority, Some(&store), Some(&unsigned_witness)),
		&vec![
			(authority, payer_account()),
			(store, store_account(0, lamports)),
			(unsigned_witness, payer_account()),
		],
		&[Check::err(
			solana_program_error::ProgramError::MissingRequiredSignature,
		)],
	);

	// A valid StoreState account derived for a different authority must not be
	// accepted merely because its owner and discriminator match.
	let other_authority = Pubkey::new_unique();
	let (wrong_store, _) = derive_store(&other_authority);
	mollusk.process_and_validate_instruction(
		&inspect_ix(&authority, Some(&wrong_store), None),
		&vec![
			(authority, payer_account()),
			(wrong_store, store_account(0, lamports)),
		],
		&[Check::err(solana_program_error::ProgramError::InvalidSeeds)],
	);
}
