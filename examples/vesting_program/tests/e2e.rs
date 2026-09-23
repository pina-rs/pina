//! End-to-end tests for the vesting program.
//!
//! These tests exercise the validation and error paths of Cancel and Claim
//! through `mollusk-svm`. Both instructions CPI into the token program on their
//! success paths — Claim releases the vested amount, Cancel refunds the
//! unclaimed balance and closes the vault — so the two fixtures that assert a
//! successful release require the real SPL programs, and they are skipped when
//! the harness has not staged their ELFs beside the program binary. The
//! end-to-end coverage of those releases lives in the Surfpool suite, which
//! runs a real runtime and asserts the exact token movements.
//!
//! ## Prerequisites
//!
//! The vesting program must be compiled to an SBF binary before running these
//! tests:
//!
//! ```sh
//! cargo build-vesting-program
//! ```
//!
//! Then set `SBF_OUT_DIR` to the directory containing the `.so` file, or place
//! it in `tests/fixtures/`.
//!
//! ## Running
//!
//! ```sh
//! SBF_OUT_DIR=target/deploy \
//!     cargo test -p vesting_program --test e2e -- --nocapture
//! ```

use mollusk_svm::Mollusk;
use mollusk_svm::result::Check;
use pina::ProgramError;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;
use vesting_program::CancelInstruction;
use vesting_program::ClaimInstruction;
use vesting_program::InitializeInstruction;
use vesting_program::VestingError;
use vesting_program::VestingState;
use vesting_program::VestingStateZc;

// ---------------------------------------------------------------------------
// Well-known program IDs
// ---------------------------------------------------------------------------

/// SPL Token program ID: `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`
///
/// Uses `pina::token::ID` because `solana-sdk-ids` v3 does not expose an
/// `spl_token` module.  `Pubkey` and `pina::Address` are both re-exports of
/// `solana_address::Address`, so the value is directly assignment-compatible.
fn spl_token_2022_program_id() -> Pubkey {
	Pubkey::from_str_const("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb")
}

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

	let mut mollusk = Mollusk::new(&program_id(), "vesting_program");
	// A success-path fixture runs the release and refund CPIs against the real
	// programs; Mollusk resolves an ELF by name from `SBF_OUT_DIR`, so one is
	// available exactly when the test task staged it next to the program.
	for (program_id, name) in staged_cpi_programs() {
		mollusk.add_program(&program_id, name);
	}

	Some(mollusk)
}

/// The real CPI programs the success-path fixtures need, when staged.
fn staged_cpi_programs() -> Vec<(Pubkey, &'static str)> {
	let dirs: Vec<std::path::PathBuf> = ["SBF_OUT_DIR", "BPF_OUT_DIR"]
		.into_iter()
		.filter_map(|key| std::env::var(key).ok())
		.map(std::path::PathBuf::from)
		.chain(std::iter::once(std::path::PathBuf::from("tests/fixtures")))
		.collect();

	[
		(spl_token_program_id(), "spl_token"),
		(spl_ata_program_id(), "spl_ata"),
	]
	.into_iter()
	.filter(|(_, name)| {
		dirs.iter()
			.any(|dir| dir.join(format!("{name}.so")).is_file())
	})
	.collect()
}

/// Whether the real programs a successful release needs are staged.
fn release_cpis_available() -> bool {
	staged_cpi_programs().len() == 2
}

/// The Clock sysvar account, carrying a timestamp the schedules are compared
/// against. Its layout is five little-endian fields; `Claim` reads the last.
fn clock_sysvar_account(mollusk: &Mollusk, unix_timestamp: i64) -> (Pubkey, Account) {
	let clock = &mollusk.sysvars.clock;
	let mut data = Vec::with_capacity(40);
	data.extend_from_slice(&clock.slot.to_le_bytes());
	data.extend_from_slice(&clock.epoch_start_timestamp.to_le_bytes());
	data.extend_from_slice(&clock.epoch.to_le_bytes());
	data.extend_from_slice(&clock.leader_schedule_epoch.to_le_bytes());
	data.extend_from_slice(&unix_timestamp.to_le_bytes());

	(
		solana_sdk_ids::sysvar::clock::id(),
		Account {
			lamports: mollusk.sysvars.rent.minimum_balance(data.len()),
			data,
			owner: solana_sdk_ids::sysvar::id(),
			executable: false,
			rent_epoch: 0,
		},
	)
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
		&[wallet.as_ref(), token_program.as_ref(), mint.as_ref()],
		&ata_program,
	);
	ata
}

/// Initialize caller-owned account storage with the given state.
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
	let mut data = vec![0u8; VestingState::SIZE];
	VestingState::initialize(&mut data, |state| {
		state.admin = pubkey_to_address(admin);
		state.beneficiary = pubkey_to_address(beneficiary);
		state.mint = pubkey_to_address(mint);
		state.total_amount.set(total_amount);
		state.claimed_amount.set(claimed_amount);
		state.start_ts.set(start_ts);
		state.cliff_ts.set(cliff_ts);
		state.end_ts.set(end_ts);
		state.cancelled.set(cancelled);
		state.bump = bump;
		Ok(())
	})
	.unwrap_or_else(|error| panic!("vesting initialization failed: {error:?}"));
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

/// Build a real SPL mint image for a fixture whose instruction runs a CPI.
///
/// The token program validates the mint's own state and decimals on every
/// `TransferChecked`, so a zeroed buffer is rejected even though the program
/// only reads the owner and address. Layout: the absent mint authority, supply,
/// decimals, the initialized flag, then the absent freeze authority.
fn initialized_mint_account(decimals: u8, supply: u64) -> Account {
	let mut data = vec![0u8; 82];
	data[36..44].copy_from_slice(&supply.to_le_bytes());
	data[44] = decimals;
	data[45] = 1; // COption::Some — is_initialized

	Account {
		lamports: 1_000_000_000,
		data,
		owner: spl_token_program_id(),
		executable: false,
		rent_epoch: 0,
	}
}

/// Build a real SPL token account image for a fixture whose instruction runs a
/// CPI.
///
/// The release and refund paths execute against the actual SPL Token program,
/// which deserializes this layout: `mint`, `owner`, the little-endian amount,
/// the absent-delegate slot, then the account state. A zeroed buffer fails that
/// parse with "stored owner or mint does not match".
fn token_account(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Account {
	let mut data = vec![0u8; 165];
	data[..32].copy_from_slice(mint.as_ref());
	data[32..64].copy_from_slice(owner.as_ref());
	data[64..72].copy_from_slice(&amount.to_le_bytes());
	data[108] = 1; // AccountState::Initialized

	Account {
		lamports: 1_000_000_000,
		data,
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

/// Account metas for `Claim`, in the program's declared order.
///
/// The clock sysvar is last because `Claim` reads it to enforce the cliff and
/// the linear unlock.
fn claim_account_metas(
	beneficiary: &Pubkey,
	mint: &Pubkey,
	vesting_pda: &Pubkey,
	beneficiary_ata: &Pubkey,
	vault: &Pubkey,
) -> Vec<AccountMeta> {
	vec![
		AccountMeta::new(*beneficiary, true),
		AccountMeta::new_readonly(*mint, false),
		AccountMeta::new(*vesting_pda, false),
		AccountMeta::new(*beneficiary_ata, false),
		AccountMeta::new(*vault, false),
		AccountMeta::new_readonly(spl_ata_program_id(), false),
		AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		AccountMeta::new_readonly(spl_token_program_id(), false),
		AccountMeta::new_readonly(solana_sdk_ids::sysvar::clock::id(), false),
	]
}

/// Account metas for `Cancel`, in the program's declared order.
///
/// `Cancel` refunds the unclaimed balance to the admin and closes the vault, so
/// the admin is writable and the admin ATA and ATA program take part in the
/// refund.
fn cancel_account_metas(
	admin: &Pubkey,
	mint: &Pubkey,
	vesting_pda: &Pubkey,
	admin_ata: &Pubkey,
	vault: &Pubkey,
	clock: &Pubkey,
	beneficiary_ata: &Pubkey,
) -> Vec<AccountMeta> {
	vec![
		AccountMeta::new(*admin, true),
		AccountMeta::new_readonly(*mint, false),
		AccountMeta::new(*vesting_pda, false),
		AccountMeta::new(*admin_ata, false),
		AccountMeta::new(*vault, false),
		AccountMeta::new_readonly(spl_ata_program_id(), false),
		AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		AccountMeta::new_readonly(spl_token_program_id(), false),
		AccountMeta::new_readonly(*clock, false),
		AccountMeta::new(*beneficiary_ata, false),
	]
}

/// Build instruction data for Cancel (just discriminator byte 2).
fn cancel_ix_data() -> Vec<u8> {
	let mut data = vec![0u8; CancelInstruction::SIZE];
	CancelInstruction::initialize(&mut data, |_| Ok(()))
		.unwrap_or_else(|error| panic!("cancel initialization failed: {error:?}"));
	data
}

/// Build instruction data for Claim (discriminator byte 1 + amount).
fn claim_ix_data(amount: u64) -> Vec<u8> {
	let mut data = vec![0u8; ClaimInstruction::SIZE];
	ClaimInstruction::initialize(&mut data, |instruction| {
		instruction.amount.set(amount);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("claim initialization failed: {error:?}"));
	data
}

/// Build instruction data for Initialize.
fn initialize_ix_data(
	total_amount: u64,
	start_ts: u64,
	cliff_ts: u64,
	end_ts: u64,
	bump: u8,
) -> Vec<u8> {
	let mut data = vec![0u8; InitializeInstruction::SIZE];
	InitializeInstruction::initialize(&mut data, |instruction| {
		instruction.total_amount.set(total_amount);
		instruction.start_ts.set(start_ts);
		instruction.cliff_ts.set(cliff_ts);
		instruction.end_ts.set(end_ts);
		instruction.bump = bump;
		Ok(())
	})
	.unwrap_or_else(|error| panic!("initialize instruction failed: {error:?}"));
	data
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

/// Associated-token program stub used by validation-only fixtures.
fn associated_token_program_account() -> (Pubkey, Account) {
	(
		spl_ata_program_id(),
		Account {
			lamports: 1,
			data: vec![],
			owner: solana_sdk_ids::bpf_loader::ID,
			executable: true,
			rent_epoch: 0,
		},
	)
}

const SKIP_MSG: &str =
	"[SKIP] vesting_program SBF binary not found. Build it with `cargo build-vesting-program`.";

/// Reported when a fixture needs the real CPI programs and the harness has not
/// staged them. The Surfpool suite covers the same releases on a real runtime.
const CPI_SKIP_MSG: &str = "[SKIP] the real SPL Token and associated-token programs are not \
                            staged; place spl_token.so and spl_ata.so in SBF_OUT_DIR to run the \
                            release-path fixtures";

// ---------------------------------------------------------------------------
// Cancel Tests
// ---------------------------------------------------------------------------

#[test]
fn cancel_sets_cancelled_flag() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};
	if !release_cpis_available() {
		eprintln!("{CPI_SKIP_MSG}");
		return;
	}

	let admin = Pubkey::new_unique();
	let beneficiary = Pubkey::new_unique();
	let mint = Pubkey::new_unique();
	let (vesting_pda, bump) = derive_vesting_pda(&admin, &beneficiary, &mint);
	let vault = derive_ata(&vesting_pda, &mint);
	let admin_ata = derive_ata(&admin, &mint);
	let beneficiary_ata = derive_ata(&beneficiary, &mint);
	let (clock_key, clock_account) = clock_sysvar_account(&mollusk, 1_700_000_000);

	let lamports = mollusk.sysvars.rent.minimum_balance(VestingState::SIZE);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&cancel_ix_data(),
		cancel_account_metas(
			&admin,
			&mint,
			&vesting_pda,
			&admin_ata,
			&vault,
			&clock_key,
			&beneficiary_ata,
		),
	);

	let accounts = vec![
		(
			admin,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(mint, initialized_mint_account(6, 1_000_000)),
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
		(vault, token_account(&mint, &vesting_pda, 1_000_000)),
		(admin_ata, token_account(&mint, &admin, 0)),
		(beneficiary_ata, token_account(&mint, &beneficiary, 0)),
		(clock_key, clock_account),
		associated_token_program_account(),
		mollusk_svm::program::keyed_account_for_system_program(),
		token_program_account(),
	];

	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);

	// Verify the cancelled flag was set to true.
	let vesting_account = result
		.get_account(&vesting_pda)
		.expect("vesting_state account should exist after cancel");
	let vesting_state: &VestingStateZc =
		<VestingState as pina::PinaPodFixed>::read_exact(&vesting_account.data).unwrap();
	assert!(
		vesting_state.cancelled.get(),
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
	let admin_ata = derive_ata(&admin, &mint);
	let beneficiary_ata = derive_ata(&beneficiary, &mint);
	let (clock_key, clock_account) = clock_sysvar_account(&mollusk, 1_700_000_000);

	let lamports = mollusk.sysvars.rent.minimum_balance(VestingState::SIZE);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&cancel_ix_data(),
		cancel_account_metas(
			&admin,
			&mint,
			&vesting_pda,
			&admin_ata,
			&vault,
			&clock_key,
			&beneficiary_ata,
		),
	);

	let accounts = vec![
		(
			admin,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(mint, initialized_mint_account(6, 1_000_000)),
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
		(vault, token_account(&mint, &vesting_pda, 1_000_000)),
		(admin_ata, token_account(&mint, &admin, 0)),
		(beneficiary_ata, token_account(&mint, &beneficiary, 0)),
		(clock_key, clock_account),
		associated_token_program_account(),
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
	let admin_ata = derive_ata(&admin, &mint);

	let lamports = mollusk.sysvars.rent.minimum_balance(VestingState::SIZE);

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
		(
			wrong_admin,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(mint, initialized_mint_account(6, 1_000_000)),
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
		(vault, token_account(&mint, &vesting_pda, 1_000_000)),
		clock_sysvar_account(&mollusk, 250),
		mollusk_svm::program::keyed_account_for_system_program(),
		associated_token_program_account(),
		token_program_account(),
	];

	// The program checks `admin.assert_address(&vesting_state.admin)` which
	// should fail because wrong_admin != admin.
	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(ProgramError::InvalidAccountData)],
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
	let admin_ata = derive_ata(&admin, &mint);
	let beneficiary_ata = derive_ata(&beneficiary, &mint);

	let lamports = mollusk.sysvars.rent.minimum_balance(VestingState::SIZE);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&claim_ix_data(100),
		claim_account_metas(&beneficiary, &mint, &vesting_pda, &beneficiary_ata, &vault),
	);

	let accounts = vec![
		(
			beneficiary,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(mint, initialized_mint_account(6, 1_000_000)),
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
		(beneficiary_ata, token_account(&mint, &beneficiary, 0)),
		(vault, token_account(&mint, &vesting_pda, 1_000_000)),
		clock_sysvar_account(&mollusk, 250),
		mollusk_svm::program::keyed_account_for_system_program(),
		associated_token_program_account(),
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
	let admin_ata = derive_ata(&admin, &mint);
	let beneficiary_ata = derive_ata(&beneficiary, &mint);

	let lamports = mollusk.sysvars.rent.minimum_balance(VestingState::SIZE);

	// Try to claim 600 when total is 1000 and already claimed 500 → next = 1100 > 1000.
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&claim_ix_data(600),
		claim_account_metas(&beneficiary, &mint, &vesting_pda, &beneficiary_ata, &vault),
	);

	let accounts = vec![
		(
			beneficiary,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(mint, initialized_mint_account(6, 1_000_000)),
		(
			vesting_pda,
			vesting_state_account(
				&admin,
				&beneficiary,
				&mint,
				1_000, // total_amount
				500,   // already claimed
				100,
				200,
				300,
				false,
				bump,
				lamports,
			),
		),
		(beneficiary_ata, token_account(&mint, &beneficiary, 0)),
		(vault, token_account(&mint, &vesting_pda, 1_000_000)),
		clock_sysvar_account(&mollusk, 250),
		mollusk_svm::program::keyed_account_for_system_program(),
		associated_token_program_account(),
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
	if !release_cpis_available() {
		eprintln!("{CPI_SKIP_MSG}");
		return;
	}

	let admin = Pubkey::new_unique();
	let beneficiary = Pubkey::new_unique();
	let wrong_beneficiary = Pubkey::new_unique();
	let mint = Pubkey::new_unique();
	let (vesting_pda, bump) = derive_vesting_pda(&admin, &beneficiary, &mint);
	let vault = derive_ata(&vesting_pda, &mint);
	let admin_ata = derive_ata(&admin, &mint);
	let wrong_beneficiary_ata = derive_ata(&wrong_beneficiary, &mint);

	let lamports = mollusk.sysvars.rent.minimum_balance(VestingState::SIZE);

	// wrong_beneficiary signs, but vesting_state has `beneficiary` as the real one.
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&claim_ix_data(100),
		claim_account_metas(
			&wrong_beneficiary,
			&mint,
			&vesting_pda,
			&wrong_beneficiary_ata,
			&vault,
		),
	);

	let accounts = vec![
		(
			wrong_beneficiary,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(mint, initialized_mint_account(6, 1_000_000)),
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
		(
			wrong_beneficiary_ata,
			token_account(&mint, &wrong_beneficiary, 0),
		),
		(vault, token_account(&mint, &vesting_pda, 1_000_000)),
		clock_sysvar_account(&mollusk, 250),
		mollusk_svm::program::keyed_account_for_system_program(),
		associated_token_program_account(),
		token_program_account(),
	];

	// `beneficiary.assert_address(&vesting_state.beneficiary)` should fail.
	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(ProgramError::InvalidAccountData)],
	);
}

// ---------------------------------------------------------------------------
// Initialize Tests (validate_schedule error paths — full Initialize requires
// token CPI so we can only test early validation failures)
// ---------------------------------------------------------------------------

#[test]
/// SEC-30 (audit regression): `Initialize` must reject a Token-2022 mint
/// carrying extensions. Every value-exit path rejects extended mints
/// (`assert_no_extensions`), so accepting one here would create a schedule
/// whose funded allocation can never leave.
#[test]
fn initialize_rejects_a_token_2022_mint_with_extensions() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let admin = Pubkey::new_unique();
	let beneficiary = Pubkey::new_unique();
	let mint = Pubkey::new_unique();
	let (vesting_pda, bump) = derive_vesting_pda(&admin, &beneficiary, &mint);
	let vault = derive_ata(&vesting_pda, &mint);
	let admin_ata = derive_ata(&admin, &mint);

	// A canonical extended Token-2022 mint: 82-byte base (initialized,
	// authority set), padding, the mint account-type byte, then the
	// NonTransferable TLV entry (type 9, zero-length payload).
	let mut data = vec![0u8; 171];
	data[0..4].copy_from_slice(&1u32.to_le_bytes());
	data[4..36].copy_from_slice(&[3u8; 32]);
	data[36..44].copy_from_slice(&0u64.to_le_bytes());
	data[44] = 6;
	data[45] = 1;
	data[46..50].copy_from_slice(&0u32.to_le_bytes());
	data[164] = 2; // account type: mint
	data[165..167].copy_from_slice(&9u16.to_le_bytes());
	data[167..171].copy_from_slice(&0u32.to_le_bytes());
	let extended_mint = Account {
		lamports: 1_000_000_000,
		data,
		owner: spl_token_2022_program_id(),
		executable: false,
		rent_epoch: 0,
	};

	let token_2022 = Account {
		lamports: 1,
		data: vec![],
		owner: solana_sdk_ids::system_program::id(),
		executable: true,
		rent_epoch: 0,
	};

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&initialize_ix_data(1_000_000, 0, 0, 0, bump),
		vec![
			AccountMeta::new(admin, true),
			AccountMeta::new_readonly(beneficiary, false),
			AccountMeta::new_readonly(mint, false),
			AccountMeta::new(vesting_pda, false),
			AccountMeta::new(vault, false),
			AccountMeta::new(admin_ata, false),
			AccountMeta::new_readonly(spl_ata_program_id(), false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
			AccountMeta::new_readonly(spl_token_2022_program_id(), false),
		],
	);

	let accounts = vec![
		(
			admin,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(
			beneficiary,
			Account::new(0, 0, &solana_sdk_ids::system_program::id()),
		),
		(mint, extended_mint),
		(vesting_pda, Account::default()),
		(vault, Account::default()),
		(admin_ata, mock_ata_account(1_000_000)),
		mollusk_svm::program::keyed_account_for_system_program(),
		associated_token_program_account(),
		(spl_token_2022_program_id(), token_2022),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(ProgramError::InvalidAccountData)],
	);
}

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
	let admin_ata = derive_ata(&admin, &mint);

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
			AccountMeta::new(admin_ata, false),
			AccountMeta::new_readonly(spl_ata_program_id(), false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
		],
	);

	let accounts = vec![
		(
			admin,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(
			beneficiary,
			Account::new(0, 0, &solana_sdk_ids::system_program::id()),
		),
		(mint, initialized_mint_account(6, 1_000_000)),
		(vesting_pda, Account::default()),
		(vault, Account::default()),
		(admin_ata, mock_ata_account(1_000_000)),
		mollusk_svm::program::keyed_account_for_system_program(),
		associated_token_program_account(),
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
	let admin_ata = derive_ata(&admin, &mint);

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
			AccountMeta::new(admin_ata, false),
			AccountMeta::new_readonly(spl_ata_program_id(), false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
		],
	);

	let accounts = vec![
		(
			admin,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(
			beneficiary,
			Account::new(0, 0, &solana_sdk_ids::system_program::id()),
		),
		(mint, initialized_mint_account(6, 1_000_000)),
		(vesting_pda, Account::default()),
		(vault, Account::default()),
		(admin_ata, mock_ata_account(1_000_000)),
		mollusk_svm::program::keyed_account_for_system_program(),
		associated_token_program_account(),
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
	if !release_cpis_available() {
		eprintln!("{CPI_SKIP_MSG}");
		return;
	}

	let admin = Pubkey::new_unique();
	let beneficiary = Pubkey::new_unique();
	let mint = Pubkey::new_unique();
	let (vesting_pda, bump) = derive_vesting_pda(&admin, &beneficiary, &mint);
	let vault = derive_ata(&vesting_pda, &mint);
	let admin_ata = derive_ata(&admin, &mint);
	let beneficiary_ata = derive_ata(&beneficiary, &mint);
	let (clock_key, clock_account) = clock_sysvar_account(&mollusk, 1_700_000_000);

	let lamports = mollusk.sysvars.rent.minimum_balance(VestingState::SIZE);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&cancel_ix_data(),
		cancel_account_metas(
			&admin,
			&mint,
			&vesting_pda,
			&admin_ata,
			&vault,
			&clock_key,
			&beneficiary_ata,
		),
	);

	let accounts = vec![
		(
			admin,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(mint, initialized_mint_account(6, 1_000_000)),
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
		(vault, token_account(&mint, &vesting_pda, 750_000)),
		(admin_ata, token_account(&mint, &admin, 0)),
		(beneficiary_ata, token_account(&mint, &beneficiary, 0)),
		(clock_key, clock_account),
		associated_token_program_account(),
		mollusk_svm::program::keyed_account_for_system_program(),
		token_program_account(),
	];

	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);

	eprintln!(
		"[CU BENCHMARK] Cancel vesting: {} compute units consumed",
		result.compute_units_consumed
	);
}
