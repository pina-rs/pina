//! End-to-end tests for the vesting program.
//!
//! These tests exercise the Cancel instruction (which does no CPI beyond
//! validation), the Claim instruction's state updates, and various error
//! paths through `mollusk-svm`.
//!
//! ## Prerequisites
//!
//! The vesting program must be compiled to an SBF binary before running these
//! tests:
//!
//! ```sh
//! cargo build --release --target bpfel-unknown-none -p vesting_program \
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
//!     cargo test -p vesting_program --test e2e -- --nocapture
//! ```

use mollusk_svm::Mollusk;
use mollusk_svm::result::Check;
use pina::PodBool;
use pina::PodU64;
use pina::bytemuck;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;
use vesting_program::CancelInstruction;
use vesting_program::ClaimInstruction;
use vesting_program::InitializeInstruction;
use vesting_program::VestingError;
use vesting_program::VestingState;

// ---------------------------------------------------------------------------
// Well-known program IDs
// ---------------------------------------------------------------------------

/// SPL Token program ID: `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`
///
/// Uses `pina::token::ID` because `solana-sdk-ids` v3 does not expose an
/// `spl_token` module.  `Pubkey` and `pina::Address` are both re-exports of
/// `solana_address::Address`, so the value is directly assignment-compatible.
fn spl_token_program_id() -> Pubkey {
	pina::token::ID
}

/// SPL Associated Token Account program ID:
/// `ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL`
fn spl_ata_program_id() -> Pubkey {
	pina::associated_token_account::ID
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn program_id() -> Pubkey {
	let id = vesting_program::ID;
	let bytes: &[u8] = id.as_ref();
	let array: [u8; 32] = bytes
		.try_into()
		.unwrap_or_else(|_| panic!("address must be 32 bytes"));
	Pubkey::new_from_array(array)
}

/// Try to create a mollusk instance for the vesting program.
///
/// Returns `None` if the BPF binary cannot be found. This allows the tests to
/// be skipped gracefully without triggering a panic-abort from the `no_std`
/// panic handler.
fn try_create_mollusk() -> Option<Mollusk> {
	let so_name = "vesting_program.so";
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

	Some(Mollusk::new(&program_id(), "vesting_program"))
}

/// Derive the vesting PDA for the given admin, beneficiary, and mint.
fn derive_vesting_pda(admin: &Pubkey, beneficiary: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(
		&[
			b"vesting",
			admin.as_ref(),
			beneficiary.as_ref(),
			mint.as_ref(),
		],
		&program_id(),
	)
}

/// Derive the associated token account address for a given wallet and mint
/// under the SPL Token program.
fn derive_ata(wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
	let token_program = spl_token_program_id();
	let ata_program = spl_ata_program_id();
	let (ata, _bump) = Pubkey::find_program_address(
		&[
			wallet.as_ref(),
			token_program.as_ref(),
			mint.as_ref(),
		],
		&ata_program,
	);
	ata
}

/// Build a serialized `VestingState` account with the given parameters.
fn vesting_state_account(
	admin: &Pubkey,
	beneficiary: &Pubkey,
	mint: &Pubkey,
	total_amount: u64,
	claimed_amount: u64,
	start_ts: u64,
	cliff_ts: u64,
	end_ts: u64,
	cancelled: bool,
	bump: u8,
	lamports: u64,
) -> Account {
	let state = VestingState::builder()
		.admin(pubkey_to_address(admin))
		.beneficiary(pubkey_to_address(beneficiary))
		.mint(pubkey_to_address(mint))
		.total_amount(PodU64::from_primitive(total_amount))
		.claimed_amount(PodU64::from_primitive(claimed_amount))
		.start_ts(PodU64::from_primitive(start_ts))
		.cliff_ts(PodU64::from_primitive(cliff_ts))
		.end_ts(PodU64::from_primitive(end_ts))
		.cancelled(PodBool::from_bool(cancelled))
		.bump(bump)
		.build();
	let data = bytemuck::bytes_of(&state).to_vec();
	Account {
		lamports,
		data,
		owner: program_id(),
		executable: false,
		rent_epoch: 0,
	}
}

fn pubkey_to_address(pk: &Pubkey) -> pina::Address {
	let bytes: [u8; 32] = pk.to_bytes();
	bytes.into()
}

/// Create a minimal mock SPL mint account (44 bytes, owned by the SPL Token
/// program). The actual data layout doesn't matter for our tests since the
/// program only checks the owner and address.
fn mock_mint_account(lamports: u64) -> Account {
	Account {
		lamports,
		data: vec![0u8; 82], // SPL Mint is 82 bytes
		owner: spl_token_program_id(),
		executable: false,
		rent_epoch: 0,
	}
}

/// Create a minimal mock ATA account (165 bytes, owned by the SPL Token
/// program).
fn mock_ata_account(lamports: u64) -> Account {
	Account {
		lamports,
		data: vec![0u8; 165], // SPL Token Account is 165 bytes
		owner: spl_token_program_id(),
		executable: false,
		rent_epoch: 0,
	}
}

/// Build instruction data for Cancel (just discriminator byte 2).
fn cancel_ix_data() -> Vec<u8> {
	let ix = CancelInstruction::builder().build();
	ix.to_bytes().to_vec()
}

/// Build instruction data for Claim (discriminator byte 1 + amount).
fn claim_ix_data(amount: u64) -> Vec<u8> {
	let ix = ClaimInstruction::builder()
		.amount(PodU64::from_primitive(amount))
		.build();
	ix.to_bytes().to_vec()
}

/// Build instruction data for Initialize.
fn initialize_ix_data(total_amount: u64, start_ts: u64, cliff_ts: u64, end_ts: u64, bump: u8) -> Vec<u8> {
	let ix = InitializeInstruction::builder()
		.total_amount(PodU64::from_primitive(total_amount))
		.start_ts(PodU64::from_primitive(start_ts))
		.cliff_ts(PodU64::from_primitive(cliff_ts))
		.end_ts(PodU64::from_primitive(end_ts))
		.bump(bump)
		.build();
	ix.to_bytes().to_vec()
}

/// Token program account (non-executable stub — only needs the right address
/// for `assert_addresses`). mollusk needs to know the token program exists.
fn token_program_account() -> (Pubkey, Account) {
	(
		spl_token_program_id(),
		Account {
			lamports: 1,
			data: vec![],
			owner: solana_sdk_ids::bpf_loader::ID,
			executable: true,
			rent_epoch: 0,
		},
	)
}

const SKIP_MSG: &str = "[SKIP] vesting_program SBF binary not found. Build it first with \
	`cargo build --release --target bpfel-unknown-none -p vesting_program -Z build-std -F \
	bpf-entrypoint`.";

// ---------------------------------------------------------------------------
// Cancel Tests
// ---------------------------------------------------------------------------

#[test]
fn cancel_sets_cancelled_flag() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let admin = Pubkey::new_unique();
	let beneficiary = Pubkey::new_unique();
	let mint = Pubkey::new_unique();
	let (vesting_pda, bump) = derive_vesting_pda(&admin, &beneficiary, &mint);
	let vault = derive_ata(&vesting_pda, &mint);

	let lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<VestingState>());

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&cancel_ix_data(),
		vec![
			AccountMeta::new_readonly(admin, true),          // admin (signer)
			AccountMeta::new_readonly(mint, false),           // mint
			AccountMeta::new(vesting_pda, false),             // vesting_state (writable)
			AccountMeta::new(vault, false),                   // vault (writable)
			AccountMeta::new_readonly(spl_token_program_id(), false), // token_program
		],
	);

	let accounts = vec![
		(admin, Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id())),
		(mint, mock_mint_account(1_000_000)),
		(
			vesting_pda,
			vesting_state_account(
				&admin,
				&beneficiary,
				&mint,
				1_000_000, // total_amount
				0,         // claimed_amount
				100,       // start_ts
				200,       // cliff_ts
				300,       // end_ts
				false,     // cancelled
				bump,
				lamports,
			),
		),
		(vault, mock_ata_account(1_000_000)),
		token_program_account(),
	];

	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);

	// Verify the cancelled flag was set to true.
	let vesting_account = result
		.get_account(&vesting_pda)
		.expect("vesting_state account should exist after cancel");
	let vesting_state: &VestingState = bytemuck::from_bytes(&vesting_account.data);
	assert!(
		bool::from(vesting_state.cancelled),
		"cancelled flag should be true after Cancel"
	);

	eprintln!(
		"[CU] Cancel: {} compute units consumed",
		result.compute_units_consumed
	);
}

#[test]
fn cancel_already_cancelled_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let admin = Pubkey::new_unique();
	let beneficiary = Pubkey::new_unique();
	let mint = Pubkey::new_unique();
	let (vesting_pda, bump) = derive_vesting_pda(&admin, &beneficiary, &mint);
	let vault = derive_ata(&vesting_pda, &mint);

	let lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<VestingState>());

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&cancel_ix_data(),
		vec![
			AccountMeta::new_readonly(admin, true),
			AccountMeta::new_readonly(mint, false),
			AccountMeta::new(vesting_pda, false),
			AccountMeta::new(vault, false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
		],
	);

	let accounts = vec![
		(admin, Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id())),
		(mint, mock_mint_account(1_000_000)),
		(
			vesting_pda,
			vesting_state_account(
				&admin,
				&beneficiary,
				&mint,
				1_000_000,
				0,
				100,
				200,
				300,
				true, // already cancelled
				bump,
				lamports,
			),
		),
		(vault, mock_ata_account(1_000_000)),
		token_program_account(),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(VestingError::AlreadyCancelled.into())],
	);
}

#[test]
fn cancel_wrong_admin_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let admin = Pubkey::new_unique();
	let wrong_admin = Pubkey::new_unique();
	let beneficiary = Pubkey::new_unique();
	let mint = Pubkey::new_unique();
	let (vesting_pda, bump) = derive_vesting_pda(&admin, &beneficiary, &mint);
	let vault = derive_ata(&vesting_pda, &mint);

	let lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<VestingState>());

	// The wrong_admin signs but the vesting_state's admin field points to `admin`.
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&cancel_ix_data(),
		vec![
			AccountMeta::new_readonly(wrong_admin, true),
			AccountMeta::new_readonly(mint, false),
			AccountMeta::new(vesting_pda, false),
			AccountMeta::new(vault, false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
		],
	);

	let accounts = vec![
		(wrong_admin, Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id())),
		(mint, mock_mint_account(1_000_000)),
		(
			vesting_pda,
			vesting_state_account(
				&admin,
				&beneficiary,
				&mint,
				1_000_000,
				0,
				100,
				200,
				300,
				false,
				bump,
				lamports,
			),
		),
		(vault, mock_ata_account(1_000_000)),
		token_program_account(),
	];

	// The program checks `admin.assert_address(&vesting_state.admin)` which
	// should fail because wrong_admin != admin.
	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::instruction_err(
			solana_instruction::error::InstructionError::Custom(0xC001_0001),
		)],
	);
}

// ---------------------------------------------------------------------------
// Claim Tests (state-update focused — CPI to ATA program will fail, so we
// test only the validation/error paths that fail *before* the CPI)
// ---------------------------------------------------------------------------

#[test]
fn claim_already_cancelled_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let admin = Pubkey::new_unique();
	let beneficiary = Pubkey::new_unique();
	let mint = Pubkey::new_unique();
	let (vesting_pda, bump) = derive_vesting_pda(&admin, &beneficiary, &mint);
	let vault = derive_ata(&vesting_pda, &mint);
	let beneficiary_ata = derive_ata(&beneficiary, &mint);

	let lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<VestingState>());

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&claim_ix_data(100),
		vec![
			AccountMeta::new_readonly(beneficiary, true),     // beneficiary (signer)
			AccountMeta::new_readonly(mint, false),            // mint
			AccountMeta::new(vesting_pda, false),              // vesting_state (writable)
			AccountMeta::new(beneficiary_ata, false),          // beneficiary_ata (writable)
			AccountMeta::new(vault, false),                    // vault (writable)
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
		],
	);

	let accounts = vec![
		(beneficiary, Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id())),
		(mint, mock_mint_account(1_000_000)),
		(
			vesting_pda,
			vesting_state_account(
				&admin,
				&beneficiary,
				&mint,
				1_000_000,
				0,
				100,
				200,
				300,
				true, // cancelled
				bump,
				lamports,
			),
		),
		(beneficiary_ata, mock_ata_account(0)),
		(vault, mock_ata_account(1_000_000)),
		mollusk_svm::program::keyed_account_for_system_program(),
		token_program_account(),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(VestingError::AlreadyCancelled.into())],
	);
}

#[test]
fn claim_too_large_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let admin = Pubkey::new_unique();
	let beneficiary = Pubkey::new_unique();
	let mint = Pubkey::new_unique();
	let (vesting_pda, bump) = derive_vesting_pda(&admin, &beneficiary, &mint);
	let vault = derive_ata(&vesting_pda, &mint);
	let beneficiary_ata = derive_ata(&beneficiary, &mint);

	let lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<VestingState>());

	// Try to claim 600 when total is 1000 and already claimed 500 → next = 1100 > 1000.
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&claim_ix_data(600),
		vec![
			AccountMeta::new_readonly(beneficiary, true),
			AccountMeta::new_readonly(mint, false),
			AccountMeta::new(vesting_pda, false),
			AccountMeta::new(beneficiary_ata, false),
			AccountMeta::new(vault, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
		],
	);

	let accounts = vec![
		(beneficiary, Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id())),
		(mint, mock_mint_account(1_000_000)),
		(
			vesting_pda,
			vesting_state_account(
				&admin,
				&beneficiary,
				&mint,
				1_000,  // total_amount
				500,    // already claimed
				100,
				200,
				300,
				false,
				bump,
				lamports,
			),
		),
		(beneficiary_ata, mock_ata_account(0)),
		(vault, mock_ata_account(1_000_000)),
		mollusk_svm::program::keyed_account_for_system_program(),
		token_program_account(),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(VestingError::ClaimTooLarge.into())],
	);
}

#[test]
fn claim_wrong_beneficiary_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let admin = Pubkey::new_unique();
	let beneficiary = Pubkey::new_unique();
	let wrong_beneficiary = Pubkey::new_unique();
	let mint = Pubkey::new_unique();
	let (vesting_pda, bump) = derive_vesting_pda(&admin, &beneficiary, &mint);
	let vault = derive_ata(&vesting_pda, &mint);
	let wrong_beneficiary_ata = derive_ata(&wrong_beneficiary, &mint);

	let lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<VestingState>());

	// wrong_beneficiary signs, but vesting_state has `beneficiary` as the real one.
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&claim_ix_data(100),
		vec![
			AccountMeta::new_readonly(wrong_beneficiary, true),
			AccountMeta::new_readonly(mint, false),
			AccountMeta::new(vesting_pda, false),
			AccountMeta::new(wrong_beneficiary_ata, false),
			AccountMeta::new(vault, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
		],
	);

	let accounts = vec![
		(wrong_beneficiary, Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id())),
		(mint, mock_mint_account(1_000_000)),
		(
			vesting_pda,
			vesting_state_account(
				&admin,
				&beneficiary,
				&mint,
				1_000_000,
				0,
				100,
				200,
				300,
				false,
				bump,
				lamports,
			),
		),
		(wrong_beneficiary_ata, mock_ata_account(0)),
		(vault, mock_ata_account(1_000_000)),
		mollusk_svm::program::keyed_account_for_system_program(),
		token_program_account(),
	];

	// `beneficiary.assert_address(&vesting_state.beneficiary)` should fail.
	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::instruction_err(
			solana_instruction::error::InstructionError::Custom(0xC001_0001),
		)],
	);
}

// ---------------------------------------------------------------------------
// Initialize Tests (validate_schedule error paths — full Initialize requires
// token CPI so we can only test early validation failures)
// ---------------------------------------------------------------------------

#[test]
fn initialize_invalid_schedule_start_after_cliff() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let admin = Pubkey::new_unique();
	let beneficiary = Pubkey::new_unique();
	let mint = Pubkey::new_unique();
	let (vesting_pda, bump) = derive_vesting_pda(&admin, &beneficiary, &mint);
	let vault = derive_ata(&vesting_pda, &mint);

	// start_ts (300) > cliff_ts (200) → InvalidSchedule
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&initialize_ix_data(1_000_000, 300, 200, 400, bump),
		vec![
			AccountMeta::new(admin, true),
			AccountMeta::new_readonly(beneficiary, false),
			AccountMeta::new_readonly(mint, false),
			AccountMeta::new(vesting_pda, false),
			AccountMeta::new(vault, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
		],
	);

	let accounts = vec![
		(admin, Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id())),
		(beneficiary, Account::new(0, 0, &solana_sdk_ids::system_program::id())),
		(mint, mock_mint_account(1_000_000)),
		(vesting_pda, Account::default()),
		(vault, Account::default()),
		mollusk_svm::program::keyed_account_for_system_program(),
		token_program_account(),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(VestingError::InvalidSchedule.into())],
	);
}

#[test]
fn initialize_invalid_schedule_cliff_after_end() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let admin = Pubkey::new_unique();
	let beneficiary = Pubkey::new_unique();
	let mint = Pubkey::new_unique();
	let (vesting_pda, bump) = derive_vesting_pda(&admin, &beneficiary, &mint);
	let vault = derive_ata(&vesting_pda, &mint);

	// cliff_ts (500) > end_ts (400) → InvalidSchedule
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&initialize_ix_data(1_000_000, 100, 500, 400, bump),
		vec![
			AccountMeta::new(admin, true),
			AccountMeta::new_readonly(beneficiary, false),
			AccountMeta::new_readonly(mint, false),
			AccountMeta::new(vesting_pda, false),
			AccountMeta::new(vault, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
		],
	);

	let accounts = vec![
		(admin, Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id())),
		(beneficiary, Account::new(0, 0, &solana_sdk_ids::system_program::id())),
		(mint, mock_mint_account(1_000_000)),
		(vesting_pda, Account::default()),
		(vault, Account::default()),
		mollusk_svm::program::keyed_account_for_system_program(),
		token_program_account(),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(VestingError::InvalidSchedule.into())],
	);
}

// ---------------------------------------------------------------------------
// Cancel CU benchmark
// ---------------------------------------------------------------------------

#[test]
fn benchmark_cu_cancel() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let admin = Pubkey::new_unique();
	let beneficiary = Pubkey::new_unique();
	let mint = Pubkey::new_unique();
	let (vesting_pda, bump) = derive_vesting_pda(&admin, &beneficiary, &mint);
	let vault = derive_ata(&vesting_pda, &mint);

	let lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<VestingState>());

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&cancel_ix_data(),
		vec![
			AccountMeta::new_readonly(admin, true),
			AccountMeta::new_readonly(mint, false),
			AccountMeta::new(vesting_pda, false),
			AccountMeta::new(vault, false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
		],
	);

	let accounts = vec![
		(admin, Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id())),
		(mint, mock_mint_account(1_000_000)),
		(
			vesting_pda,
			vesting_state_account(
				&admin,
				&beneficiary,
				&mint,
				1_000_000,
				250_000,
				100,
				200,
				300,
				false,
				bump,
				lamports,
			),
		),
		(vault, mock_ata_account(750_000)),
		token_program_account(),
	];

	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);

	eprintln!(
		"[CU BENCHMARK] Cancel vesting: {} compute units consumed",
		result.compute_units_consumed
	);
}
