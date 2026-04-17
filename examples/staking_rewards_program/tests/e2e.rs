//! End-to-end tests for the staking_rewards_program.
//!
//! These tests exercise the OpenPosition instruction (which performs a
//! system-program CPI to create the position PDA), the Withdraw instruction
//! (which only updates on-chain state — no token CPI), and various validation
//! error paths for Deposit, Withdraw, and Claim.
//!
//! ## Prerequisites
//!
//! The staking_rewards_program must be compiled to an SBF binary before
//! running these tests:
//!
//! ```sh
//! cargo build --release --target bpfel-unknown-none -p staking_rewards_program \
//!     -Z build-std -F bpf-entrypoint
//! ```
//!
//! Then set `SBF_OUT_DIR` to the directory containing the `.so` file, or
//! place it in `tests/fixtures/`.
//!
//! ## Running
//!
//! ```sh
//! SBF_OUT_DIR=target/bpfel-unknown-none/release \
//!     cargo test -p staking_rewards_program --test e2e -- --nocapture
//! ```

use core::mem::size_of;

use mollusk_svm::Mollusk;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use pina::PodBool;
use pina::PodU64;
use pina::bytemuck;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;
use staking_rewards_program::ClaimInstruction;
use staking_rewards_program::DepositInstruction;
use staking_rewards_program::OpenPositionInstruction;
use staking_rewards_program::PoolState;
use staking_rewards_program::PositionState;
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

	Some(Mollusk::new(&program_id(), "staking_rewards_program"))
}

/// Derive the position PDA for a given pool / owner pair.
///
/// Seeds: `[b"position", pool, owner]`
fn derive_position_pda(pool: &Pubkey, owner: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"position", pool.as_ref(), owner.as_ref()], &program_id())
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
) -> Account {
	let state = PoolState::builder()
		.admin(pubkey_to_address(admin))
		.stake_mint(pubkey_to_address(stake_mint))
		.reward_mint(pubkey_to_address(reward_mint))
		.total_staked(PodU64::from_primitive(total_staked))
		.reward_index(PodU64::from_primitive(0))
		.paused(PodBool::from_bool(paused))
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
	let state = PositionState::builder()
		.pool(pubkey_to_address(pool))
		.owner(pubkey_to_address(owner))
		.staked_amount(PodU64::from_primitive(staked_amount))
		.reward_debt(PodU64::from_primitive(reward_debt))
		.pending_rewards(PodU64::from_primitive(pending_rewards))
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

/// Instruction bytes for `OpenPosition`.
fn open_position_ix_data(bump: u8) -> Vec<u8> {
	let ix = OpenPositionInstruction::builder().bump(bump).build();
	ix.to_bytes().to_vec()
}

/// Instruction bytes for `Deposit`.
fn deposit_ix_data(amount: u64) -> Vec<u8> {
	let ix = DepositInstruction::builder()
		.amount(PodU64::from_primitive(amount))
		.build();
	ix.to_bytes().to_vec()
}

/// Instruction bytes for `Withdraw`.
fn withdraw_ix_data(amount: u64) -> Vec<u8> {
	let ix = WithdrawInstruction::builder()
		.amount(PodU64::from_primitive(amount))
		.build();
	ix.to_bytes().to_vec()
}

/// Instruction bytes for `Claim`.
fn claim_ix_data() -> Vec<u8> {
	let ix = ClaimInstruction::builder().build();
	ix.to_bytes().to_vec()
}

const SKIP_MSG: &str = "[SKIP] staking_rewards_program SBF binary not found. Build it first with \
                        `cargo build --release --target bpfel-unknown-none -p \
                        staking_rewards_program -Z build-std -F bpf-entrypoint`.";

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

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(size_of::<PoolState>());

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
		size_of::<PositionState>(),
		"position_state data should be exactly size_of::<PositionState>() bytes"
	);

	let pos_state: &PositionState = bytemuck::from_bytes(&pos_account.data);
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
		u64::from(pos_state.staked_amount),
		0,
		"staked_amount should start at zero"
	);
	assert_eq!(
		u64::from(pos_state.reward_debt),
		0,
		"reward_debt should start at zero"
	);
	assert_eq!(
		u64::from(pos_state.pending_rewards),
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
/// requested amount.
///
/// Withdraw only updates on-chain state — it issues no token CPI — so the
/// full instruction can be executed and its results verified.
#[test]
fn withdraw_updates_balances() {
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

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(size_of::<PoolState>());
	let pos_lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<PositionState>());

	// Withdraw 100 from a position with 200 staked; pool has 500 total.
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&withdraw_ix_data(100),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_stake_ata, false),
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
				500,
				false,
				0,
				pool_lamports,
			),
		),
		(
			position_state_key,
			position_state_account(&pool_state_key, &user, 200, 0, 0, 0, pos_lamports),
		),
		// ATA stub: only the address is checked — data contents don't matter.
		(
			user_stake_ata,
			Account::new(1, 165, &spl_token_program_id()),
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
	let pos_state: &PositionState = bytemuck::from_bytes(&pos_account.data);
	assert_eq!(
		u64::from(pos_state.staked_amount),
		100,
		"staked_amount should be 200 - 100 = 100"
	);

	// pool.total_staked: 500 − 100 = 400
	let pool_account = result
		.get_account(&pool_state_key)
		.expect("pool_state should exist after Withdraw");
	let pool_st: &PoolState = bytemuck::from_bytes(&pool_account.data);
	assert_eq!(
		u64::from(pool_st.total_staked),
		400,
		"total_staked should be 500 - 100 = 400"
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

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(size_of::<PoolState>());
	let pos_lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<PositionState>());

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

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(size_of::<PoolState>());
	let pos_lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<PositionState>());

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&withdraw_ix_data(50),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_stake_ata, false),
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

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(size_of::<PoolState>());
	let pos_lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<PositionState>());

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&withdraw_ix_data(0),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_stake_ata, false),
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

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(size_of::<PoolState>());
	let pos_lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<PositionState>());

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&withdraw_ix_data(25),
		vec![
			AccountMeta::new(user_b, true),
			AccountMeta::new_readonly(stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_b_stake_ata, false),
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

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(size_of::<PoolState>());
	let pos_lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<PositionState>());

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&withdraw_ix_data(25),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_stake_ata, false),
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

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(size_of::<PoolState>());
	let pos_lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<PositionState>());

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&deposit_ix_data(100),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_stake_ata, false),
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
			pool_state_account(&admin, &stake_mint, &reward_mint, 0, true, 0, pool_lamports),
		),
		(
			position_state_key,
			position_state_account(&pool_state_key, &user, 0, 0, 0, 0, pos_lamports),
		),
		(
			user_stake_ata,
			Account::new(1, 165, &spl_token_program_id()),
		),
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

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(size_of::<PoolState>());
	let pos_lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<PositionState>());

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&deposit_ix_data(0),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_stake_ata, false),
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
	let user_stake_ata = derive_ata(&user, &wrong_stake_mint);

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(size_of::<PoolState>());
	let pos_lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<PositionState>());

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&deposit_ix_data(10),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(wrong_stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_stake_ata, false),
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
/// `InvalidAmount`.
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

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(size_of::<PoolState>());
	let pos_lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<PositionState>());

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&deposit_ix_data(50),
		vec![
			AccountMeta::new(user_b, true), // user_b signs
			AccountMeta::new_readonly(stake_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_b_stake_ata, false),
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
			),
		),
		(
			position_state_key,
			// pool matches pool_state_key so the pool check passes;
			// owner is user_a so the owner check fires → InvalidAmount.
			position_state_account(&pool_state_key, &user_a, 0, 0, 0, 0, pos_lamports),
		),
		(
			user_b_stake_ata,
			Account::new(1, 165, &spl_token_program_id()),
		),
		token_program_account(),
		keyed_account_for_system_program(),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(StakingError::InvalidAmount.into())],
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

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(size_of::<PoolState>());
	let pos_lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<PositionState>());

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&claim_ix_data(),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(reward_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_reward_ata, false),
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

	let pool_lamports = mollusk.sysvars.rent.minimum_balance(size_of::<PoolState>());
	let pos_lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<PositionState>());

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&claim_ix_data(),
		vec![
			AccountMeta::new(user, true),
			AccountMeta::new_readonly(wrong_reward_mint, false),
			AccountMeta::new(pool_state_key, false),
			AccountMeta::new(position_state_key, false),
			AccountMeta::new(user_reward_ata, false),
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
		token_program_account(),
		keyed_account_for_system_program(),
	];

	mollusk.process_and_validate_instruction(
		&instruction,
		&accounts,
		&[Check::err(StakingError::InvalidPool.into())],
	);
}
