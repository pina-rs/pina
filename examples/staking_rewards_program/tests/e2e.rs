//! End-to-end tests for the staking_rewards_program.
//!
//! These tests exercise the OpenPosition instruction (which performs a
//! system-program CPI to create the position PDA), the Deposit and Withdraw
//! instructions (which move stake tokens through the pool's stake vault via a
//! token-program CPI), and various validation error paths for Deposit,
//! Withdraw, and Claim.
//!
//! ## Prerequisites
//!
//! The staking_rewards_program must be compiled to an SBF binary before
//! running these tests:
//!
//! ```sh
//! cargo build-staking-rewards-program
//! ```
//!
//! Then set `SBF_OUT_DIR` to the directory containing the `.so` file, or
//! place it in `tests/fixtures/`.
//!
//! ## Running
//!
//! ```sh
//! SBF_OUT_DIR=target/deploy \
//!     cargo test -p staking_rewards_program --test e2e -- --nocapture
//! ```

use mollusk_svm::Mollusk;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use pina::ProgramError;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;
use staking_rewards_program::ClaimInstruction;
use staking_rewards_program::DepositInstruction;
use staking_rewards_program::OpenPositionInstruction;
use staking_rewards_program::PoolState;
use staking_rewards_program::PoolStateZc;
use staking_rewards_program::PositionState;
use staking_rewards_program::PositionStateZc;
use staking_rewards_program::REWARD_INDEX_SCALE;
use staking_rewards_program::StakingError;
use staking_rewards_program::WithdrawInstruction;

// ---------------------------------------------------------------------------
// Well-known program IDs
// ---------------------------------------------------------------------------

/// SPL Token program ID: `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`
///
/// Uses the same constant as pina internally to guarantee ATA derivation
/// agreement between test helpers and on-chain validation.
fn spl_token_program_id() -> Pubkey {
	// pina::token::ID is pinocchio_token::ID.  Since Pubkey = Address in this
	// SDK generation, the value is assignment-compatible.
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

/// Convert the staking program's on-chain `Address` to a `Pubkey`.
fn program_id() -> Pubkey {
	let id = staking_rewards_program::ID;
	let bytes: &[u8] = id.as_ref();
	let array: [u8; 32] = bytes
		.try_into()
		.unwrap_or_else(|_| panic!("address must be 32 bytes"));
	Pubkey::new_from_array(array)
}

/// Try to create a Mollusk instance for the staking_rewards_program.
///
/// Returns `None` when the SBF binary cannot be found so that tests skip
/// gracefully instead of panicking (the `no_std` panic handler would abort
/// the whole process otherwise).
fn try_create_mollusk() -> Option<Mollusk> {
	let so_name = "staking_rewards_program.so";
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

	let mut mollusk = Mollusk::new(&program_id(), "staking_rewards_program");
	// The custody-path fixtures run the deposit and withdrawal CPIs against the
	// real programs; Mollusk resolves an ELF by name from `SBF_OUT_DIR`, so one
	// is available exactly when the test task staged it next to the program.
	for (program_id, name) in staged_cpi_programs() {
		mollusk.add_program(&program_id, name);
	}

	Some(mollusk)
}

/// The real CPI programs the custody-path fixtures need, when staged.
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

/// Whether the real programs the stake custody transfers need are staged.
fn transfer_cpis_available() -> bool {
	staged_cpi_programs().len() == 2
}

/// Derive the position PDA for a given pool / owner pair.
///
/// Seeds: `[b"position", pool, owner]`
fn derive_position_pda(pool: &Pubkey, owner: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"position", pool.as_ref(), owner.as_ref()], &program_id())
}

/// Derive the pool PDA for a mint pair.
///
/// Seeds: `[b"pool", stake_mint, reward_mint]`. The custody-path fixtures need
/// the canonical address and its bump because `Withdraw` signs the principal
/// transfer with the stored pool seeds.
fn derive_pool_pda(stake_mint: &Pubkey, reward_mint: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(
		&[b"pool", stake_mint.as_ref(), reward_mint.as_ref()],
		&program_id(),
	)
}

/// Derive the Associated Token Account address for a given wallet and mint
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

/// Convert a `Pubkey` (= `solana_address::Address`) to `pina::Address`.
///
/// Both types are the same underlying newtype; this helper makes the intent
/// explicit when building typed account structs.
fn pubkey_to_address(pk: &Pubkey) -> pina::Address {
	let bytes: [u8; 32] = pk.to_bytes();
	bytes.into()
}

/// Build a pre-populated `PoolState` `Account` ready for use in tests.
fn pool_state_account(
	admin: &Pubkey,
	stake_mint: &Pubkey,
	reward_mint: &Pubkey,
	total_staked: u64,
	paused: bool,
	bump: u8,
	lamports: u64,
	reward_index: u64,
) -> Account {
	let mut data = vec![0u8; PoolState::SIZE];
	PoolState::initialize(&mut data, |state| {
		state.admin = pubkey_to_address(admin);
		state.stake_mint = pubkey_to_address(stake_mint);
		state.reward_mint = pubkey_to_address(reward_mint);
		state.total_staked.set(total_staked);
		state.reward_index.set(reward_index);
		state.paused.set(paused);
		state.bump = bump;
		Ok(())
	})
	.unwrap_or_else(|error| panic!("pool initialization failed: {error:?}"));
	Account {
		lamports,
		data,
		owner: program_id(),
		executable: false,
		rent_epoch: 0,
	}
}

/// Build a pre-populated `PositionState` `Account` ready for use in tests.
fn position_state_account(
	pool: &Pubkey,
	owner: &Pubkey,
	staked_amount: u64,
	reward_debt: u64,
	pending_rewards: u64,
	bump: u8,
	lamports: u64,
) -> Account {
	let mut data = vec![0u8; PositionState::SIZE];
	PositionState::initialize(&mut data, |state| {
		state.pool = pubkey_to_address(pool);
		state.owner = pubkey_to_address(owner);
		state.staked_amount.set(staked_amount);
		state.reward_debt.set(reward_debt);
		state.pending_rewards.set(pending_rewards);
		state.bump = bump;
		Ok(())
	})
	.unwrap_or_else(|error| panic!("position initialization failed: {error:?}"));
	Account {
		lamports,
		data,
		owner: program_id(),
		executable: false,
		rent_epoch: 0,
	}
}

/// Minimal SPL mint stub — 82 bytes of zeroes, owned by the SPL Token program.
///
/// Tests only need the account's *owner* to pass `assert_owners`, so the
/// internal layout doesn't matter here.
fn mock_mint_account(lamports: u64) -> Account {
	Account {
		lamports,
		data: vec![0u8; 82],
		owner: spl_token_program_id(),
		executable: false,
		rent_epoch: 0,
	}
}

/// Build a real SPL mint image for a fixture whose instruction runs a token
/// CPI.
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
/// token CPI.
///
/// The custody paths execute against the actual SPL Token program, which
/// deserializes this layout: `mint`, `owner`, the little-endian amount, the
/// absent-delegate slot, then the account state. A zeroed buffer fails that
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

/// Token program stub: executable, owned by the BPF loader, at the SPL Token
/// program address.  Mollusk's `assert_addresses` only checks the key.
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

/// Instruction bytes for `OpenPosition`.
fn open_position_ix_data(bump: u8) -> Vec<u8> {
	let mut data = vec![0u8; OpenPositionInstruction::SIZE];
	OpenPositionInstruction::initialize(&mut data, |instruction| {
		instruction.bump = bump;
		Ok(())
	})
	.unwrap_or_else(|error| panic!("open-position initialization failed: {error:?}"));
	data
}

/// Instruction bytes for `Deposit`.
fn deposit_ix_data(amount: u64) -> Vec<u8> {
	let mut data = vec![0u8; DepositInstruction::SIZE];
	DepositInstruction::initialize(&mut data, |instruction| {
		instruction.amount.set(amount);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("deposit initialization failed: {error:?}"));
	data
}

/// Instruction bytes for `Withdraw`.
fn withdraw_ix_data(amount: u64) -> Vec<u8> {
	let mut data = vec![0u8; WithdrawInstruction::SIZE];
	WithdrawInstruction::initialize(&mut data, |instruction| {
		instruction.amount.set(amount);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("withdraw initialization failed: {error:?}"));
	data
}

/// Instruction bytes for `Claim`.
fn claim_ix_data() -> Vec<u8> {
	let mut data = vec![0u8; ClaimInstruction::SIZE];
	ClaimInstruction::initialize(&mut data, |_| Ok(()))
		.unwrap_or_else(|error| panic!("claim initialization failed: {error:?}"));
	data
}

const SKIP_MSG: &str = "[SKIP] staking_rewards_program SBF binary not found. Build it first with \
                        `cargo build --release --target bpfel-unknown-none -p \
                        staking_rewards_program -Z build-std -F bpf-entrypoint`.";

/// Reported when a fixture needs the real CPI programs and the harness has not
/// staged them. The Surfpool suite covers the same transfers on a real runtime.
const CPI_SKIP_MSG: &str = "[SKIP] the real SPL Token and associated-token programs are not \
                            staged; place spl_token.so and spl_ata.so in SBF_OUT_DIR to run the \
                            custody-path fixtures";

/// The token balance of a real SPL token account image, at the fixed amount
/// offset every `TransferChecked` touches.
fn token_amount(account: &Account) -> u64 {
	u64::from_le_bytes(account.data[64..72].try_into().expect("token amount"))
}

// ---------------------------------------------------------------------------
// OpenPosition Tests
// ---------------------------------------------------------------------------

/// Verify that `OpenPosition` creates a properly initialised `PositionState`
/// account via a CPI to the system program.
///
/// The test pre-builds a `PoolState` account so that no `InitializePool`
/// instruction (which requires token CPIs) is needed.
#[test]
fn open_position_creates_position_state() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let user = Pubkey::new_unique();
	let pool_state_key = Pubkey::new_unique(); // arbitrary address for the pre-built pool
	let admin = Pubkey::new_unique();
	let stake_mint = Pubkey::new_unique();
	let reward_mint = Pubkey::new_unique();

	// Derive the canonical position PDA using the pool address and user address.
	let (position_pda, bump) = derive_position_pda(&pool_state_key, &user);

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(PoolState::SIZE);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&open_position_ix_data(bump),
		vec![
			AccountMeta::new(user, true), // user — signer, pays for PDA creation
			AccountMeta::new(pool_state_key, false), // pool_state — writable (asserted by program)
			AccountMeta::new(position_pda, false), // position_state — empty PDA
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let accounts = vec![
		(
			user,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(
			pool_state_key,
			pool_state_account(
				&admin,
				&stake_mint,
				&reward_mint,
				0,
				false,
				0,
				pool_lamports,
				0,
			),
		),
		(position_pda, Account::default()), // empty — will be created by the CPI
		keyed_account_for_system_program(),
	];

	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);

	// Verify the position_state account was created with the correct data.
	let pos_account = result
		.get_account(&position_pda)
		.expect("position_state account should exist after OpenPosition");
	assert_eq!(
		pos_account.data.len(),
		PositionState::SIZE,
		"position_state data should be exactly PositionState::SIZE bytes"
	);

	let pos_state: &PositionStateZc =
		<PositionState as pina::PinaPodFixed>::read_exact(&pos_account.data).unwrap();
	assert_eq!(
		pos_state.pool.as_ref(),
		pool_state_key.as_ref(),
		"position.pool should reference the pool_state"
	);
	assert_eq!(
		pos_state.owner.as_ref(),
		user.as_ref(),
		"position.owner should be the user"
	);
	assert_eq!(
		pos_state.staked_amount.get(),
		0,
		"staked_amount should start at zero"
	);
	assert_eq!(
		pos_state.reward_debt.get(),
		0,
		"reward_debt should start at zero"
	);
	assert_eq!(
		pos_state.pending_rewards.get(),
		0,
		"pending_rewards should start at zero"
	);
	assert_eq!(
		pos_state.bump, bump,
		"stored bump should match the derived bump"
	);

	eprintln!(
		"[CU] OpenPosition: {} compute units consumed",
		result.compute_units_consumed
	);
}

// ---------------------------------------------------------------------------
// Withdraw Tests
// ---------------------------------------------------------------------------

/// Verify that `Withdraw` decreases `staked_amount` and `total_staked` by the
/// requested amount and transfers the principal out of the stake vault.
///
/// The principal is signed out by the pool PDA, so the fixture needs the real
/// SPL Token program staged; without it the test skips and the Surfpool suite
/// covers the same transfer on a real runtime.
#[test]
fn withdraw_updates_balances() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};
	if !transfer_cpis_available() {
		eprintln!("{CPI_SKIP_MSG}");
		return;
	}

	let user = Pubkey::new_unique();
	let stake_mint = Pubkey::new_unique();
	let reward_mint = Pubkey::new_unique();
	let admin = Pubkey::new_unique();
	// The canonical pool PDA and its bump: `Withdraw` signs the principal
	// transfer with the stored seeds, so the runtime must be able to derive the
	// same address.
	let (pool_state_key, pool_bump) = derive_pool_pda(&stake_mint, &reward_mint);
	let position_state_key = Pubkey::new_unique();
	let user_stake_ata = derive_ata(&user, &stake_mint);
	let stake_vault = derive_ata(&pool_state_key, &stake_mint);

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(PoolState::SIZE);
	let pos_lamports = mollusk.sysvars.rent.minimum_balance(PositionState::SIZE);

	// Withdraw 100 from a position with 200 staked; the pool has 500 total and
	// the vault holds the 200 tokens custody actually backed.
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&withdraw_ix_data(100),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_stake_ata, false),
			AccountMeta::new(stake_vault, false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let accounts = vec![
		(
			user,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(stake_mint, initialized_mint_account(6, 200)),
		(
			pool_state_key,
			pool_state_account(
				&admin,
				&stake_mint,
				&reward_mint,
				500,
				false,
				pool_bump,
				pool_lamports,
				0,
			),
		),
		(
			position_state_key,
			position_state_account(&pool_state_key, &user, 200, 0, 0, 0, pos_lamports),
		),
		(user_stake_ata, token_account(&stake_mint, &user, 0)),
		(
			stake_vault,
			token_account(&stake_mint, &pool_state_key, 200),
		),
		token_program_account(),
		keyed_account_for_system_program(),
	];

	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);

	// position.staked_amount: 200 − 100 = 100
	let pos_account = result
		.get_account(&position_state_key)
		.expect("position_state should exist after Withdraw");
	let pos_state: &PositionStateZc =
		<PositionState as pina::PinaPodFixed>::read_exact(&pos_account.data).unwrap();
	assert_eq!(
		pos_state.staked_amount.get(),
		100,
		"staked_amount should be 200 - 100 = 100"
	);

	// pool.total_staked: 500 − 100 = 400
	let pool_account = result
		.get_account(&pool_state_key)
		.expect("pool_state should exist after Withdraw");
	let pool_st: &PoolStateZc =
		<PoolState as pina::PinaPodFixed>::read_exact(&pool_account.data).unwrap();
	assert_eq!(
		pool_st.total_staked.get(),
		400,
		"total_staked should be 500 - 100 = 400"
	);

	// The principal moved: the user's ATA received it, the vault released it.
	assert_eq!(
		token_amount(
			result
				.get_account(&user_stake_ata)
				.expect("user ATA after Withdraw")
		),
		100,
		"the withdrawn principal must land in the user's stake ATA"
	);
	assert_eq!(
		token_amount(
			result
				.get_account(&stake_vault)
				.expect("stake vault after Withdraw")
		),
		100,
		"the withdrawn principal must leave the stake vault"
	);

	eprintln!(
		"[CU] Withdraw: {} compute units consumed",
		result.compute_units_consumed
	);
}

/// Trying to withdraw more than the position's `staked_amount` must fail with
/// `InsufficientBalance`.
#[test]
fn withdraw_insufficient_balance_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let user = Pubkey::new_unique();
	let stake_mint = Pubkey::new_unique();
	let pool_state_key = Pubkey::new_unique();
	let position_state_key = Pubkey::new_unique();
	let admin = Pubkey::new_unique();
	let reward_mint = Pubkey::new_unique();
	let user_stake_ata = derive_ata(&user, &stake_mint);
	let stake_vault = derive_ata(&pool_state_key, &stake_mint);

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(PoolState::SIZE);
	let pos_lamports = mollusk.sysvars.rent.minimum_balance(PositionState::SIZE);

	// Position only has 100 staked; attempt to withdraw 200.
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&withdraw_ix_data(200),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_stake_ata, false),
			AccountMeta::new(stake_vault, false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let accounts = vec![
		(
			user,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(stake_mint, mock_mint_account(1_000_000)),
		(
			pool_state_key,
			pool_state_account(
				&admin,
				&stake_mint,
				&reward_mint,
				100,
				false,
				0,
				pool_lamports,
				0,
			),
		),
		(
			position_state_key,
			position_state_account(&pool_state_key, &user, 100, 0, 0, 0, pos_lamports),
		),
		(
			user_stake_ata,
			Account::new(1, 165, &spl_token_program_id()),
		),
		(stake_vault, Account::new(1, 165, &spl_token_program_id())),
		token_program_account(),
		keyed_account_for_system_program(),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(StakingError::InsufficientBalance.into())],
	);
}

/// Calling `Withdraw` on a paused pool must fail with `PoolPaused`.
#[test]
fn withdraw_from_paused_pool_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let user = Pubkey::new_unique();
	let stake_mint = Pubkey::new_unique();
	let pool_state_key = Pubkey::new_unique();
	let position_state_key = Pubkey::new_unique();
	let admin = Pubkey::new_unique();
	let reward_mint = Pubkey::new_unique();
	let user_stake_ata = derive_ata(&user, &stake_mint);
	let stake_vault = derive_ata(&pool_state_key, &stake_mint);

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(PoolState::SIZE);
	let pos_lamports = mollusk.sysvars.rent.minimum_balance(PositionState::SIZE);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&withdraw_ix_data(50),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_stake_ata, false),
			AccountMeta::new(stake_vault, false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let accounts = vec![
		(
			user,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(stake_mint, mock_mint_account(1_000_000)),
		(
			pool_state_key,
			// paused = true
			pool_state_account(
				&admin,
				&stake_mint,
				&reward_mint,
				200,
				true,
				0,
				pool_lamports,
				0,
			),
		),
		(
			position_state_key,
			position_state_account(&pool_state_key, &user, 100, 0, 0, 0, pos_lamports),
		),
		(
			user_stake_ata,
			Account::new(1, 165, &spl_token_program_id()),
		),
		(stake_vault, Account::new(1, 165, &spl_token_program_id())),
		token_program_account(),
		keyed_account_for_system_program(),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(StakingError::PoolPaused.into())],
	);
}

/// Withdrawing zero tokens must fail with `InvalidAmount` before any state
/// mutation occurs.
#[test]
fn withdraw_zero_amount_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let user = Pubkey::new_unique();
	let stake_mint = Pubkey::new_unique();
	let pool_state_key = Pubkey::new_unique();
	let position_state_key = Pubkey::new_unique();
	let admin = Pubkey::new_unique();
	let reward_mint = Pubkey::new_unique();
	let user_stake_ata = derive_ata(&user, &stake_mint);
	let stake_vault = derive_ata(&pool_state_key, &stake_mint);

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(PoolState::SIZE);
	let pos_lamports = mollusk.sysvars.rent.minimum_balance(PositionState::SIZE);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&withdraw_ix_data(0),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_stake_ata, false),
			AccountMeta::new(stake_vault, false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let accounts = vec![
		(
			user,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(stake_mint, mock_mint_account(1_000_000)),
		(
			pool_state_key,
			pool_state_account(
				&admin,
				&stake_mint,
				&reward_mint,
				200,
				false,
				0,
				pool_lamports,
				0,
			),
		),
		(
			position_state_key,
			position_state_account(&pool_state_key, &user, 100, 0, 0, 0, pos_lamports),
		),
		(
			user_stake_ata,
			Account::new(1, 165, &spl_token_program_id()),
		),
		(stake_vault, Account::new(1, 165, &spl_token_program_id())),
		token_program_account(),
		keyed_account_for_system_program(),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(StakingError::InvalidAmount.into())],
	);
}

/// Trying to withdraw from a position owned by a different signer must fail
/// with `Unauthorized`.
#[test]
fn withdraw_wrong_owner_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let user_a = Pubkey::new_unique();
	let user_b = Pubkey::new_unique();
	let stake_mint = Pubkey::new_unique();
	let pool_state_key = Pubkey::new_unique();
	let position_state_key = Pubkey::new_unique();
	let admin = Pubkey::new_unique();
	let reward_mint = Pubkey::new_unique();
	let user_b_stake_ata = derive_ata(&user_b, &stake_mint);
	let stake_vault = derive_ata(&pool_state_key, &stake_mint);

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(PoolState::SIZE);
	let pos_lamports = mollusk.sysvars.rent.minimum_balance(PositionState::SIZE);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&withdraw_ix_data(25),
		vec![
			AccountMeta::new(user_b, true),
			AccountMeta::new_readonly(stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_b_stake_ata, false),
			AccountMeta::new(stake_vault, false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let accounts = vec![
		(
			user_b,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(stake_mint, mock_mint_account(1_000_000)),
		(
			pool_state_key,
			pool_state_account(
				&admin,
				&stake_mint,
				&reward_mint,
				200,
				false,
				0,
				pool_lamports,
				0,
			),
		),
		(
			position_state_key,
			position_state_account(&pool_state_key, &user_a, 100, 0, 0, 0, pos_lamports),
		),
		(
			user_b_stake_ata,
			Account::new(1, 165, &spl_token_program_id()),
		),
		(stake_vault, Account::new(1, 165, &spl_token_program_id())),
		token_program_account(),
		keyed_account_for_system_program(),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(StakingError::Unauthorized.into())],
	);
}

/// Trying to withdraw from a position bound to a different pool must fail
/// with `InvalidPool`.
#[test]
fn withdraw_wrong_pool_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let user = Pubkey::new_unique();
	let stake_mint = Pubkey::new_unique();
	let pool_state_key = Pubkey::new_unique();
	let other_pool_key = Pubkey::new_unique();
	let position_state_key = Pubkey::new_unique();
	let admin = Pubkey::new_unique();
	let reward_mint = Pubkey::new_unique();
	let user_stake_ata = derive_ata(&user, &stake_mint);
	let stake_vault = derive_ata(&pool_state_key, &stake_mint);

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(PoolState::SIZE);
	let pos_lamports = mollusk.sysvars.rent.minimum_balance(PositionState::SIZE);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&withdraw_ix_data(25),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_stake_ata, false),
			AccountMeta::new(stake_vault, false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let accounts = vec![
		(
			user,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(stake_mint, mock_mint_account(1_000_000)),
		(
			pool_state_key,
			pool_state_account(
				&admin,
				&stake_mint,
				&reward_mint,
				200,
				false,
				0,
				pool_lamports,
				0,
			),
		),
		(
			position_state_key,
			position_state_account(&other_pool_key, &user, 100, 0, 0, 0, pos_lamports),
		),
		(
			user_stake_ata,
			Account::new(1, 165, &spl_token_program_id()),
		),
		(stake_vault, Account::new(1, 165, &spl_token_program_id())),
		token_program_account(),
		keyed_account_for_system_program(),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(StakingError::InvalidPool.into())],
	);
}

// ---------------------------------------------------------------------------
// Deposit Error Path Tests
//
// Full `Deposit` execution fails because the instruction ends with an ATA CPI
// (`CreateIdempotent`) that requires a real token program binary.  We test
// only the validation paths that fire *before* that CPI.
// ---------------------------------------------------------------------------

/// A `Deposit` against a paused pool must fail with `PoolPaused` before any
/// token CPI is attempted.
#[test]
fn deposit_paused_pool_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let user = Pubkey::new_unique();
	let stake_mint = Pubkey::new_unique();
	let pool_state_key = Pubkey::new_unique();
	let position_state_key = Pubkey::new_unique();
	let admin = Pubkey::new_unique();
	let reward_mint = Pubkey::new_unique();
	let user_stake_ata = derive_ata(&user, &stake_mint);
	let stake_vault = derive_ata(&pool_state_key, &stake_mint);

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(PoolState::SIZE);
	let pos_lamports = mollusk.sysvars.rent.minimum_balance(PositionState::SIZE);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&deposit_ix_data(100),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_stake_ata, false),
			AccountMeta::new(stake_vault, false),
			AccountMeta::new_readonly(spl_ata_program_id(), false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let accounts = vec![
		(
			user,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(stake_mint, mock_mint_account(1_000_000)),
		(
			pool_state_key,
			// paused = true — the program returns PoolPaused before calling CPI
			pool_state_account(
				&admin,
				&stake_mint,
				&reward_mint,
				0,
				true,
				0,
				pool_lamports,
				0,
			),
		),
		(
			position_state_key,
			position_state_account(&pool_state_key, &user, 0, 0, 0, 0, pos_lamports),
		),
		(
			user_stake_ata,
			Account::new(1, 165, &spl_token_program_id()),
		),
		(stake_vault, Account::new(1, 165, &spl_token_program_id())),
		associated_token_program_account(),
		token_program_account(),
		keyed_account_for_system_program(),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(StakingError::PoolPaused.into())],
	);
}

/// Depositing zero tokens must fail with `InvalidAmount` before the ATA CPI is
/// attempted.
#[test]
fn deposit_zero_amount_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let user = Pubkey::new_unique();
	let stake_mint = Pubkey::new_unique();
	let pool_state_key = Pubkey::new_unique();
	let position_state_key = Pubkey::new_unique();
	let admin = Pubkey::new_unique();
	let reward_mint = Pubkey::new_unique();
	let user_stake_ata = derive_ata(&user, &stake_mint);
	let stake_vault = derive_ata(&pool_state_key, &stake_mint);

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(PoolState::SIZE);
	let pos_lamports = mollusk.sysvars.rent.minimum_balance(PositionState::SIZE);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&deposit_ix_data(0),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_stake_ata, false),
			AccountMeta::new(stake_vault, false),
			AccountMeta::new_readonly(spl_ata_program_id(), false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let accounts = vec![
		(
			user,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(stake_mint, mock_mint_account(1_000_000)),
		(
			pool_state_key,
			pool_state_account(
				&admin,
				&stake_mint,
				&reward_mint,
				0,
				false,
				0,
				pool_lamports,
				0,
			),
		),
		(
			position_state_key,
			position_state_account(&pool_state_key, &user, 0, 0, 0, 0, pos_lamports),
		),
		(
			user_stake_ata,
			Account::new(1, 165, &spl_token_program_id()),
		),
		(stake_vault, Account::new(1, 165, &spl_token_program_id())),
		associated_token_program_account(),
		token_program_account(),
		keyed_account_for_system_program(),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(StakingError::InvalidAmount.into())],
	);
}

/// Depositing with a stake mint that does not match the pool configuration
/// must fail with `InvalidPool`.
#[test]
fn deposit_wrong_stake_mint_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let user = Pubkey::new_unique();
	let pool_stake_mint = Pubkey::new_unique();
	let wrong_stake_mint = Pubkey::new_unique();
	let pool_state_key = Pubkey::new_unique();
	let position_state_key = Pubkey::new_unique();
	let admin = Pubkey::new_unique();
	let reward_mint = Pubkey::new_unique();
	// The vault follows the instruction's mint so the derived-address check
	// passes and the mismatch with the pool's stored mint is what fires.
	let user_stake_ata = derive_ata(&user, &wrong_stake_mint);
	let stake_vault = derive_ata(&pool_state_key, &wrong_stake_mint);

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(PoolState::SIZE);
	let pos_lamports = mollusk.sysvars.rent.minimum_balance(PositionState::SIZE);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&deposit_ix_data(10),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(wrong_stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_stake_ata, false),
			AccountMeta::new(stake_vault, false),
			AccountMeta::new_readonly(spl_ata_program_id(), false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let accounts = vec![
		(
			user,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(wrong_stake_mint, mock_mint_account(1_000_000)),
		(
			pool_state_key,
			pool_state_account(
				&admin,
				&pool_stake_mint,
				&reward_mint,
				0,
				false,
				0,
				pool_lamports,
				0,
			),
		),
		(
			position_state_key,
			position_state_account(&pool_state_key, &user, 0, 0, 0, 0, pos_lamports),
		),
		(
			user_stake_ata,
			Account::new(1, 165, &spl_token_program_id()),
		),
		(stake_vault, Account::new(1, 165, &spl_token_program_id())),
		associated_token_program_account(),
		token_program_account(),
		keyed_account_for_system_program(),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(StakingError::InvalidPool.into())],
	);
}

/// Depositing into a position that belongs to a different owner must fail with
/// `Unauthorized`.
///
/// The pool check (`position.pool == pool_state.address`) passes because we
/// store the correct pool address in the position.  The owner check then fails
/// because `position.owner == user_a != user_b == signer`.
#[test]
fn deposit_wrong_owner_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let user_a = Pubkey::new_unique(); // real owner of the position
	let user_b = Pubkey::new_unique(); // attacker — signs the transaction
	let stake_mint = Pubkey::new_unique();
	let pool_state_key = Pubkey::new_unique();
	let position_state_key = Pubkey::new_unique();
	let admin = Pubkey::new_unique();
	let reward_mint = Pubkey::new_unique();
	// ATA derived for user_b (the signer) — must match what the program checks.
	let user_b_stake_ata = derive_ata(&user_b, &stake_mint);
	let stake_vault = derive_ata(&pool_state_key, &stake_mint);

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(PoolState::SIZE);
	let pos_lamports = mollusk.sysvars.rent.minimum_balance(PositionState::SIZE);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&deposit_ix_data(50),
		vec![
			AccountMeta::new(user_b, true), // user_b signs
			AccountMeta::new_readonly(stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_b_stake_ata, false),
			AccountMeta::new(stake_vault, false),
			AccountMeta::new_readonly(spl_ata_program_id(), false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let accounts = vec![
		(
			user_b,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(stake_mint, mock_mint_account(1_000_000)),
		(
			pool_state_key,
			pool_state_account(
				&admin,
				&stake_mint,
				&reward_mint,
				0,
				false,
				0,
				pool_lamports,
				0,
			),
		),
		(
			position_state_key,
			// pool matches pool_state_key so the pool check passes;
			// owner is user_a so the owner check fires → Unauthorized.
			position_state_account(&pool_state_key, &user_a, 0, 0, 0, 0, pos_lamports),
		),
		(
			user_b_stake_ata,
			Account::new(1, 165, &spl_token_program_id()),
		),
		(stake_vault, Account::new(1, 165, &spl_token_program_id())),
		associated_token_program_account(),
		token_program_account(),
		keyed_account_for_system_program(),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(StakingError::Unauthorized.into())],
	);
}

/// Verify that a funded `Deposit` transfers the stake into the pool's stake
/// vault and credits the position only alongside that transfer.
///
/// The depositor's ATA already exists and holds the tokens, so
/// `CreateIdempotent` takes its idempotent branch and the custody
/// `TransferChecked` does the moving. The fixture needs the real SPL Token and
/// associated-token programs staged; without them the test skips and the
/// Surfpool suite covers the same transfer on a real runtime.
#[test]
fn deposit_transfers_stake_into_vault() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};
	if !transfer_cpis_available() {
		eprintln!("{CPI_SKIP_MSG}");
		return;
	}

	let user = Pubkey::new_unique();
	let stake_mint = Pubkey::new_unique();
	let reward_mint = Pubkey::new_unique();
	let admin = Pubkey::new_unique();
	let pool_state_key = derive_pool_pda(&stake_mint, &reward_mint).0;
	let position_state_key = Pubkey::new_unique();
	let user_stake_ata = derive_ata(&user, &stake_mint);
	let stake_vault = derive_ata(&pool_state_key, &stake_mint);

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(PoolState::SIZE);
	let pos_lamports = mollusk.sysvars.rent.minimum_balance(PositionState::SIZE);

	// Deposit 100 of the 100 tokens the depositor holds.
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&deposit_ix_data(100),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_stake_ata, false),
			AccountMeta::new(stake_vault, false),
			AccountMeta::new_readonly(spl_ata_program_id(), false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let accounts = vec![
		(
			user,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(stake_mint, initialized_mint_account(6, 100)),
		(
			pool_state_key,
			pool_state_account(
				&admin,
				&stake_mint,
				&reward_mint,
				0,
				false,
				0,
				pool_lamports,
				0,
			),
		),
		(
			position_state_key,
			position_state_account(&pool_state_key, &user, 0, 0, 0, 0, pos_lamports),
		),
		(user_stake_ata, token_account(&stake_mint, &user, 100)),
		(stake_vault, token_account(&stake_mint, &pool_state_key, 0)),
		associated_token_program_account(),
		token_program_account(),
		keyed_account_for_system_program(),
	];

	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);

	// The ledger credited exactly what moved into custody.
	let pos_account = result
		.get_account(&position_state_key)
		.expect("position_state should exist after Deposit");
	let pos_state: &PositionStateZc =
		<PositionState as pina::PinaPodFixed>::read_exact(&pos_account.data).unwrap();
	assert_eq!(
		pos_state.staked_amount.get(),
		100,
		"staked_amount should equal the deposited amount"
	);
	let pool_account = result
		.get_account(&pool_state_key)
		.expect("pool_state should exist after Deposit");
	let pool_st: &PoolStateZc =
		<PoolState as pina::PinaPodFixed>::read_exact(&pool_account.data).unwrap();
	assert_eq!(
		pool_st.total_staked.get(),
		100,
		"total_staked should equal the deposited amount"
	);
	assert_eq!(
		token_amount(
			result
				.get_account(&stake_vault)
				.expect("stake vault after Deposit")
		),
		100,
		"the deposit must arrive in the stake vault"
	);
	assert_eq!(
		token_amount(
			result
				.get_account(&user_stake_ata)
				.expect("user ATA after Deposit")
		),
		0,
		"the deposit must leave the depositor's ATA"
	);

	eprintln!(
		"[CU] Deposit: {} compute units consumed",
		result.compute_units_consumed
	);
}

// ---------------------------------------------------------------------------
// Claim Error Path Tests
//
// Full `Claim` execution also ends with an ATA CPI (`CreateIdempotent`), so we
// exercise only validation paths that fire before that CPI.
// ---------------------------------------------------------------------------

/// Claiming rewards for a position bound to a different pool must fail with
/// `InvalidPool` before any ATA CPI is attempted.
#[test]
fn claim_wrong_pool_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let user = Pubkey::new_unique();
	let reward_mint = Pubkey::new_unique();
	let pool_state_key = Pubkey::new_unique();
	let other_pool_key = Pubkey::new_unique();
	let position_state_key = Pubkey::new_unique();
	let admin = Pubkey::new_unique();
	let stake_mint = Pubkey::new_unique();
	let user_reward_ata = derive_ata(&user, &reward_mint);
	let reward_vault = derive_ata(&pool_state_key, &reward_mint);

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(PoolState::SIZE);
	let pos_lamports = mollusk.sysvars.rent.minimum_balance(PositionState::SIZE);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&claim_ix_data(),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(reward_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_reward_ata, false),
			AccountMeta::new(reward_vault, false),
			AccountMeta::new_readonly(spl_ata_program_id(), false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let accounts = vec![
		(
			user,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(reward_mint, mock_mint_account(1_000_000)),
		(
			pool_state_key,
			pool_state_account(
				&admin,
				&stake_mint,
				&reward_mint,
				0,
				false,
				0,
				pool_lamports,
				0,
			),
		),
		(
			position_state_key,
			position_state_account(&other_pool_key, &user, 0, 0, 0, 0, pos_lamports),
		),
		(
			user_reward_ata,
			Account::new(1, 165, &spl_token_program_id()),
		),
		(reward_vault, Account::new(1, 165, &spl_token_program_id())),
		associated_token_program_account(),
		token_program_account(),
		keyed_account_for_system_program(),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(StakingError::InvalidPool.into())],
	);
}

/// Claiming with a reward mint that does not match the pool configuration must
/// fail with `InvalidPool` before any ATA CPI is attempted.
#[test]
fn claim_wrong_reward_mint_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let user = Pubkey::new_unique();
	let pool_reward_mint = Pubkey::new_unique();
	let wrong_reward_mint = Pubkey::new_unique();
	let pool_state_key = Pubkey::new_unique();
	let position_state_key = Pubkey::new_unique();
	let admin = Pubkey::new_unique();
	let stake_mint = Pubkey::new_unique();
	let user_reward_ata = derive_ata(&user, &wrong_reward_mint);
	let reward_vault = derive_ata(&pool_state_key, &wrong_reward_mint);

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(PoolState::SIZE);
	let pos_lamports = mollusk.sysvars.rent.minimum_balance(PositionState::SIZE);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&claim_ix_data(),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(wrong_reward_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_reward_ata, false),
			AccountMeta::new(reward_vault, false),
			AccountMeta::new_readonly(spl_ata_program_id(), false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let accounts = vec![
		(
			user,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(wrong_reward_mint, mock_mint_account(1_000_000)),
		(
			pool_state_key,
			pool_state_account(
				&admin,
				&stake_mint,
				&pool_reward_mint,
				0,
				false,
				0,
				pool_lamports,
				0,
			),
		),
		(
			position_state_key,
			position_state_account(&pool_state_key, &user, 0, 0, 0, 0, pos_lamports),
		),
		(
			user_reward_ata,
			Account::new(1, 165, &spl_token_program_id()),
		),
		(reward_vault, Account::new(1, 165, &spl_token_program_id())),
		associated_token_program_account(),
		token_program_account(),
		keyed_account_for_system_program(),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(StakingError::InvalidPool.into())],
	);
}

/// Claiming against a reward vault that is not the pool's derived ATA must be
/// rejected.
///
/// The payout is signed by the pool PDA, so the source account is the one
/// place a caller could redirect value: without the derived-address check a
/// substituted vault — another wallet's token account, or the pool's own stake
/// vault when a pool is misconfigured with `stake_mint == reward_mint` — would
/// be drained under the pool's signature.
#[test]
fn claim_wrong_reward_vault_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let user = Pubkey::new_unique();
	let reward_mint = Pubkey::new_unique();
	let pool_state_key = Pubkey::new_unique();
	let position_state_key = Pubkey::new_unique();
	let admin = Pubkey::new_unique();
	let stake_mint = Pubkey::new_unique();
	let user_reward_ata = derive_ata(&user, &reward_mint);
	// A vault the attacker controls, rather than the pool's derived ATA.
	let attacker_vault = Pubkey::new_unique();
	// A position with accrued rewards, so the payout computation succeeds and
	// the vault identity check is the assertion under test rather than an
	// incidental "nothing to claim".
	let staked = 1_000u64;

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(PoolState::SIZE);
	let pos_lamports = mollusk.sysvars.rent.minimum_balance(PositionState::SIZE);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&claim_ix_data(),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(reward_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_reward_ata, false),
			AccountMeta::new(attacker_vault, false),
			AccountMeta::new_readonly(spl_ata_program_id(), false),
			AccountMeta::new_readonly(spl_token_program_id(), false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let accounts = vec![
		(
			user,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(reward_mint, mock_mint_account(1_000_000)),
		(
			pool_state_key,
			pool_state_account(
				&admin,
				&stake_mint,
				&reward_mint,
				staked,
				false,
				0,
				pool_lamports,
				// One full index unit: the position accrues `staked` rewards, so
				// the payout computation succeeds.
				REWARD_INDEX_SCALE,
			),
		),
		(
			position_state_key,
			position_state_account(&pool_state_key, &user, staked, 0, 0, 0, pos_lamports),
		),
		(
			user_reward_ata,
			Account::new(1, 165, &spl_token_program_id()),
		),
		(
			attacker_vault,
			Account::new(1, 165, &spl_token_program_id()),
		),
		associated_token_program_account(),
		token_program_account(),
		keyed_account_for_system_program(),
	];

	// The substituted vault fails the derived-address check before the payout
	// computation, so the instruction never reaches the transfer.
	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(ProgramError::InvalidSeeds)],
	);
}
