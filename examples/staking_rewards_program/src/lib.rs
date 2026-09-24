//! Staking account and rewards-bookkeeping scaffold built with pina.
//!
//! This example demonstrates the account and validation shape of a staking
//! lifecycle:
//! - initialize a rewards pool with a stake vault and a reward vault
//! - open per-user positions
//! - deposit stake tokens into the stake vault and withdraw them back out
//! - update reward bookkeeping against a position account and release
//!   accrued rewards from the reward vault
//!
//! This is not a staking product. Deposits and withdrawals do move real stake
//! tokens to and from the pool's stake vault, and claims release real reward
//! tokens from the reward vault, but the example still defines no reward
//! emissions, funding, or solvency: the index only moves when the admin calls
//! `SetRewardIndex`, and nothing refills the vaults. See the example README
//! and the book's production-readiness guide before adapting it for
//! production use.

#![allow(missing_docs)]
#![allow(clippy::inline_always)]
#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("9MBwKBjzTLtLe8PkHVhi5CfGxKo8gCYbMEg5NMt1tcvr");

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	nostd_entrypoint!(StakingInstruction::process_instruction);
}

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StakingError {
	/// The amount is zero, or it leaves a position below the pool minimum.
	InvalidAmount = 0,
	/// The pool is paused, so deposits and withdrawals are refused.
	PoolPaused = 1,
	/// The position holds less than the requested withdrawal.
	InsufficientBalance = 2,
	/// The signer is not the pool authority this instruction requires.
	Unauthorized = 3,
	/// The supplied account is not the pool this position belongs to.
	InvalidPool = 4,
	/// The supplied reward index would move rewards backwards.
	RewardIndexRegressed = 5,
	/// The position has accrued nothing to release.
	NothingToClaim = 6,
	/// The reward index would create liabilities no `u64` payout can
	/// represent, freezing affected positions at their next checkpoint.
	RewardIndexExceedsCapacity = 7,
	/// The reward index would create liabilities beyond the reward vault's
	/// balance, making equal entitlements depend on claim order.
	RewardIndexExceedsReserves = 8,
}

#[discriminator(entrypoint)]
pub enum StakingInstruction {
	InitializePool = 0,
	OpenPosition = 1,
	Deposit = 2,
	Withdraw = 3,
	Claim = 4,
	SetRewardIndex = 5,
}

#[discriminator]
pub enum StakingAccountType {
	PoolState = 1,
	PositionState = 2,
}

#[account(discriminator = StakingAccountType)]
#[pda(seeds = [SEED_POOL_PREFIX, stake_mint: Address, reward_mint: Address], bump = bump)]
pub struct PoolState {
	pub admin: Address,
	pub stake_mint: Address,
	pub reward_mint: Address,
	pub total_staked: u64,
	pub reward_index: u64,
	pub paused: bool,
	pub bump: u8,
}

#[account(discriminator = StakingAccountType)]
#[pda(seeds = [SEED_POSITION_PREFIX, pool: Address, owner: Address], bump = bump)]
pub struct PositionState {
	pub pool: Address,
	pub owner: Address,
	pub staked_amount: u64,
	pub reward_debt: u64,
	pub pending_rewards: u64,
	pub bump: u8,
}

#[instruction(discriminator = StakingInstruction::InitializePool)]
pub struct InitializePoolInstruction {
	pub bump: u8,
}

#[instruction(discriminator = StakingInstruction::OpenPosition)]
pub struct OpenPositionInstruction {
	pub bump: u8,
}

#[instruction(discriminator = StakingInstruction::Deposit)]
pub struct DepositInstruction {
	pub amount: u64,
}

#[instruction(discriminator = StakingInstruction::Withdraw)]
pub struct WithdrawInstruction {
	pub amount: u64,
}

#[instruction(discriminator = StakingInstruction::Claim)]
pub struct ClaimInstruction {}

#[instruction(discriminator = StakingInstruction::SetRewardIndex)]
pub struct SetRewardIndexInstruction {
	/// The new rewards-per-token index, scaled by [`REWARD_INDEX_SCALE`].
	pub new_index: u64,
}

/// Rewards accrued for `staked` tokens between two index checkpoints.
///
/// The product of a scaled index delta and a stake is computed in `u128` so a
/// large stake cannot overflow the multiplication, then divided down to base
/// units with a floor. Flooring favors the pool: a position can never be
/// released more than the index supports, and the discarded remainder stays
/// claimable by the next checkpoint.
fn accrued_rewards(staked: u64, index_delta: u64) -> Result<u64, ProgramError> {
	let scaled = u128::from(index_delta)
		.checked_mul(u128::from(staked))
		.ok_or(ProgramError::ArithmeticOverflow)?
		/ u128::from(REWARD_INDEX_SCALE);

	u64::try_from(scaled).map_err(|_| ProgramError::ArithmeticOverflow)
}

#[derive(Accounts, Debug)]
pub struct InitializePoolAccounts<'a> {
	pub admin: &'a mut AccountView,
	pub stake_mint: &'a AccountView,
	pub reward_mint: &'a AccountView,
	pub pool_state: &'a mut AccountView,
	pub stake_vault: &'a AccountView,
	pub reward_vault: &'a AccountView,
	pub associated_token_program: &'a AccountView,
	pub system_program: &'a AccountView,
	pub token_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct OpenPositionAccounts<'a> {
	pub user: &'a mut AccountView,
	pub pool_state: &'a AccountView,
	pub position_state: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct DepositAccounts<'a> {
	pub user: &'a mut AccountView,
	pub stake_mint: &'a AccountView,
	pub pool_state: &'a mut AccountView,
	pub position_state: &'a mut AccountView,
	pub user_stake_ata: &'a AccountView,
	pub stake_vault: &'a mut AccountView,
	pub associated_token_program: &'a AccountView,
	pub token_program: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct WithdrawAccounts<'a> {
	pub user: &'a AccountView,
	pub stake_mint: &'a AccountView,
	pub pool_state: &'a mut AccountView,
	pub position_state: &'a mut AccountView,
	pub user_stake_ata: &'a mut AccountView,
	pub stake_vault: &'a mut AccountView,
	pub token_program: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct ClaimAccounts<'a> {
	pub user: &'a mut AccountView,
	pub reward_mint: &'a AccountView,
	pub pool_state: &'a AccountView,
	pub position_state: &'a mut AccountView,
	pub user_reward_ata: &'a AccountView,
	pub reward_vault: &'a mut AccountView,
	pub associated_token_program: &'a AccountView,
	pub token_program: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct SetRewardIndexAccounts<'a> {
	pub admin: &'a AccountView,
	pub pool_state: &'a mut AccountView,
	/// The pool's reward mint, for validating the vault binding.
	pub reward_mint: &'a AccountView,
	/// The token program that owns the reward mint and vault.
	pub token_program: &'a AccountView,
	/// The pool's canonical reward vault. An index update is a promise to pay:
	/// it must not create liabilities the vault cannot honor or that a
	/// per-position accrual cannot represent.
	pub reward_vault: &'a AccountView,
}

/// Scale applied to the pool's rewards-per-token index.
///
/// An integer index cannot express a fractional reward per token, so the index
/// is fixed-point: a value of `REWARD_INDEX_SCALE` means one reward token per
/// staked token. Integer division floors the accrued amount, which is why the
/// scale is large enough to keep rounding dust negligible.
pub const REWARD_INDEX_SCALE: u64 = 1_000_000_000_000;

/// Seed prefix for pool PDAs.
const SEED_POOL_PREFIX: &[u8] = b"pool";

/// Seed prefix for position PDAs.
const SEED_POSITION_PREFIX: &[u8] = b"position";

const SPL_PROGRAM_IDS: [Address; 2] = [token::ID, token_2022::ID];

fn assert_pool_stake_mint(pool_state: &PoolStateZc, stake_mint: AccountView) -> ProgramResult {
	stake_mint
		.assert_address(&pool_state.stake_mint)
		.map(|_| ())
		.map_err(|_| ProgramError::from(StakingError::InvalidPool))
}

fn assert_pool_reward_mint(pool_state: &PoolStateZc, reward_mint: AccountView) -> ProgramResult {
	reward_mint
		.assert_address(&pool_state.reward_mint)
		.map(|_| ())
		.map_err(|_| ProgramError::from(StakingError::InvalidPool))
}

fn assert_position_access(
	pool_state: AccountView,
	user: AccountView,
	position_state: &PositionStateZc,
) -> ProgramResult {
	pool_state
		.assert_address(&position_state.pool)
		.map(|_| ())
		.map_err(|_| ProgramError::from(StakingError::InvalidPool))?;
	user.assert_address(&position_state.owner)
		.map(|_| ())
		.map_err(|_| ProgramError::from(StakingError::Unauthorized))
}

impl<'a> ProcessAccountInfos<'a> for InitializePoolAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		// Parse instruction and prepare PDA seeds
		let args = InitializePoolInstruction::try_from_bytes(data)?;
		let pool_seeds = PoolState::seeds(self.stake_mint.address(), self.reward_mint.address());

		// Validate accounts
		self.admin.assert_signer()?;
		self.stake_mint.assert_owners(&SPL_PROGRAM_IDS)?;
		self.reward_mint.assert_owners(&SPL_PROGRAM_IDS)?;
		// Every value-exit path asserts extension-free mints, so the entry
		// path must apply the same policy before creating state or vaults: a
		// configuration accepted here with no exit would lock funded stake
		// and rewards forever. Each check walks one TLV header, once per pool.
		self.stake_mint
			.as_token_mint_for_program(self.token_program.address())?
			.assert_no_extensions()?;
		self.reward_mint
			.as_token_mint_for_program(self.token_program.address())?
			.assert_no_extensions()?;
		self.associated_token_program
			.assert_address(&associated_token_account::ID)?;
		self.system_program.assert_address(&system::ID)?;
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		// The vault addresses are checked by the associated token program inside
		// each `Create` CPI below, which derives the same
		// `[wallet, token_program, mint]` seeds and rejects a mismatch with
		// `InvalidSeeds` before it creates anything. Restating the derivation here
		// would repeat a ~4,500 CU canonical bump search per vault.
		self.stake_vault.assert_empty()?.assert_writable()?;
		self.reward_vault.assert_empty()?.assert_writable()?;

		// Create the pool state account.
		//
		// The pool's seeds carry no signer, so the pool for a mint pair is a
		// global singleton and the canonical address is the only thing naming it.
		// `CreateProgramAccountWithBump` searches for the canonical bump and
		// rejects a supplied bump that does not match, so the second address a
		// noncanonical bump derives cannot be created. The unchecked sibling
		// would accept it: its emptiness check sees a fresh address, and every
		// later read validates stored fields rather than the seeds, so the shadow
		// pool would be fully functional while invisible to canonical
		// derivation. That costs a ~10k CU search per initialization, once per
		// pool, which is worth paying for singleton integrity.
		CreateProgramAccountWithBump {
			account: self.pool_state,
			payer: self.admin,
			owner: &ID,
			seeds: &pool_seeds.as_slices(),
			bump: args.bump,
		}
		.invoke_with::<PoolState>(|pool_state| {
			pool_state.admin = *self.admin.address();
			pool_state.stake_mint = *self.stake_mint.address();
			pool_state.reward_mint = *self.reward_mint.address();
			pool_state.total_staked.set(0);
			pool_state.reward_index.set(0);
			pool_state.paused.set(false);
			pool_state.bump = args.bump;

			Ok(())
		})?;

		// Create stake vault
		associated_token_account::instructions::Create {
			account: self.stake_vault,
			funding_account: self.admin,
			wallet: self.pool_state,
			mint: self.stake_mint,
			system_program: self.system_program,
			token_program: self.token_program,
		}
		.invoke()?;

		// Create reward vault
		associated_token_account::instructions::Create {
			account: self.reward_vault,
			funding_account: self.admin,
			wallet: self.pool_state,
			mint: self.reward_mint,
			system_program: self.system_program,
			token_program: self.token_program,
		}
		.invoke()?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for OpenPositionAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		// Parse instruction and prepare PDA seeds
		let args = OpenPositionInstruction::try_from_bytes(data)?;
		let pool_address = *self.pool_state.address();
		let user_address = *self.user.address();
		let position_seeds = PositionState::seeds(&pool_address, &user_address);

		// Validate accounts
		self.user.assert_signer()?;
		self.system_program.assert_address(&system::ID)?;
		self.pool_state.assert_not_empty()?;

		// Check pool is not paused
		let pool_state = self.pool_state.as_account::<PoolState>(&ID)?;
		if pool_state.paused.get() {
			return Err(StakingError::PoolPaused.into());
		}

		// Create the position account.
		//
		// One position per `(pool, owner)` is the invariant every deposit and
		// claim assumes, and the seeds are the only thing expressing it. Reward
		// accrual is flat per position, so a second position for the same pair
		// accrues the full amount a second time while `staked_amount` splits
		// between them. `assert_empty` cannot prevent that: it guards only the
		// address being created, and the address a noncanonical bump derives is
		// empty. `CreateProgramAccountWithBump` searches for the canonical bump
		// and rejects any other, so the duplicate is unreachable. The user pays
		// the ~10k CU search once, when they open their position.
		CreateProgramAccountWithBump {
			account: self.position_state,
			payer: self.user,
			owner: &ID,
			seeds: &position_seeds.as_slices(),
			bump: args.bump,
		}
		.invoke_with::<PositionState>(|position_state| {
			position_state.pool = pool_address;
			position_state.owner = user_address;
			position_state.staked_amount.set(0);
			position_state.reward_debt.set(0);
			position_state.pending_rewards.set(0);
			position_state.bump = args.bump;

			Ok(())
		})?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for DepositAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		// Parse instruction data
		let args = DepositInstruction::try_from_bytes(data)?;
		let amount = args.amount.get();

		// Validate accounts
		self.user.assert_signer()?;
		self.stake_mint.assert_owners(&SPL_PROGRAM_IDS)?;
		self.associated_token_program
			.assert_address(&associated_token_account::ID)?;
		self.system_program.assert_address(&system::ID)?;
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		self.pool_state.assert_not_empty()?;
		self.position_state.assert_not_empty()?;
		// The address check lives in the `CreateIdempotent` CPI below: the
		// associated token program derives the same seeds and rejects a mismatch
		// with `InvalidSeeds` before its idempotent branch.
		self.user_stake_ata.assert_writable()?;
		// The depositor signs the transfer, but custody only backs the pool's
		// ledger if the tokens land in the pool's own stake vault. Without the
		// derived-address check a caller could deposit into a token account they
		// still control and be credited `staked_amount` all the same.
		self.stake_vault
			.assert_not_empty()?
			.assert_writable()?
			.assert_owners(&SPL_PROGRAM_IDS)?
			.assert_associated_token_address(
				self.pool_state.address(),
				self.stake_mint.address(),
				self.token_program.address(),
			)?;

		// Validate pool and position state, then release the guards for the
		// token CPIs below. Reloading each account immutably first would
		// validate it twice.
		let pool_handle = *self.pool_state;
		let user_handle = *self.user;
		{
			let position_state = self.position_state.as_account_mut::<PositionState>(&ID)?;
			let pool_state = self.pool_state.as_account_mut::<PoolState>(&ID)?;

			if pool_state.paused.get() {
				return Err(StakingError::PoolPaused.into());
			}

			if amount == 0 {
				return Err(StakingError::InvalidAmount.into());
			}

			assert_pool_stake_mint(&pool_state, *self.stake_mint)?;
			assert_position_access(pool_handle, user_handle, &position_state)?;
		}

		// Ensure user's stake ATA exists
		associated_token_account::instructions::CreateIdempotent {
			funding_account: self.user,
			account: self.user_stake_ata,
			wallet: self.user,
			mint: self.stake_mint,
			system_program: self.system_program,
			token_program: self.token_program,
		}
		.invoke()?;

		// Take custody of the deposit before the ledger is touched: the position
		// is credited only once the tokens sit in the stake vault, so a
		// depositor without them fails the transfer instead of acquiring free
		// share weight against the reward vault. The depositor signs; no pool
		// signature is involved.
		let stake_decimals = {
			let mint = self
				.stake_mint
				.as_token_mint_for_program(self.token_program.address())?
				.assert_no_extensions()?;
			mint.decimals()
		};

		let vault_before = self
			.stake_vault
			.as_token_account_for_program(self.token_program.address())?
			.amount();

		token::instructions::TransferChecked::new(
			self.user_stake_ata,
			self.stake_mint,
			self.stake_vault,
			self.user,
			amount,
			stake_decimals,
		)
		.invoke_with_program(self.token_program.address())?;

		// Credit the observed vault delta rather than the requested amount: a
		// transfer-fee extension or a partial delivery can otherwise make the
		// ledger claim more stake than the vault actually holds.
		let vault_after = self
			.stake_vault
			.as_token_account_for_program(self.token_program.address())?
			.amount();
		let received = vault_after
			.checked_sub(vault_before)
			.ok_or(ProgramError::ArithmeticOverflow)?;

		// Credit the position now that custody is held. The rewards the
		// existing stake has earned are banked before the stake changes, so an
		// amount deposited now cannot claim rewards from before it arrived,
		// and the checkpoint advances with the index.
		let mut position_state = self.position_state.as_account_mut::<PositionState>(&ID)?;
		let mut pool_state = self.pool_state.as_account_mut::<PoolState>(&ID)?;

		let next_staked = position_state
			.staked_amount
			.get()
			.checked_add(received)
			.ok_or(ProgramError::ArithmeticOverflow)?;
		let next_total_staked = pool_state
			.total_staked
			.get()
			.checked_add(received)
			.ok_or(ProgramError::ArithmeticOverflow)?;

		let index = pool_state.reward_index.get();
		let banked = position_state
			.pending_rewards
			.get()
			.checked_add(accrued_rewards(
				position_state.staked_amount.get(),
				index
					.checked_sub(position_state.reward_debt.get())
					.ok_or(StakingError::RewardIndexRegressed)?,
			)?)
			.ok_or(ProgramError::ArithmeticOverflow)?;

		position_state.staked_amount.set(next_staked);
		position_state.pending_rewards.set(banked);
		position_state.reward_debt.set(index);
		pool_state.total_staked.set(next_total_staked);

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for WithdrawAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		// Parse instruction data
		let args = WithdrawInstruction::try_from_bytes(data)?;
		let amount = args.amount.get();

		// Validate accounts
		self.user.assert_signer()?;
		self.stake_mint.assert_owners(&SPL_PROGRAM_IDS)?;
		self.system_program.assert_address(&system::ID)?;
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		self.pool_state.assert_not_empty()?;
		self.position_state.assert_not_empty()?;
		self.user_stake_ata
			.assert_writable()?
			.assert_associated_token_address(
				self.user.address(),
				self.stake_mint.address(),
				self.token_program.address(),
			)?;
		// The principal is signed out by the pool, so the source must provably
		// be the pool's own stake vault. Without the derived-address check a
		// caller could name any token account they control as the vault and the
		// pool signature would authorise draining it.
		self.stake_vault
			.assert_not_empty()?
			.assert_writable()?
			.assert_owners(&SPL_PROGRAM_IDS)?
			.assert_associated_token_address(
				self.pool_state.address(),
				self.stake_mint.address(),
				self.token_program.address(),
			)?;

		// Validate pool and position state, then write through the same guards.
		// Reloading each account immutably first would validate it twice. Both
		// guards are released before the token CPI below.
		let pool_handle = *self.pool_state;
		let user_handle = *self.user;
		let mut position_state = self.position_state.as_account_mut::<PositionState>(&ID)?;
		let mut pool_state = self.pool_state.as_account_mut::<PoolState>(&ID)?;

		if pool_state.paused.get() {
			return Err(StakingError::PoolPaused.into());
		}

		if amount == 0 {
			return Err(StakingError::InvalidAmount.into());
		}

		assert_pool_stake_mint(&pool_state, *self.stake_mint)?;
		assert_position_access(pool_handle, user_handle, &position_state)?;

		let staked_amount = position_state.staked_amount.get();
		let total_staked = pool_state.total_staked.get();

		if amount > staked_amount {
			return Err(StakingError::InsufficientBalance.into());
		}

		// Bank the rewards the position has earned at its current stake, then
		// advance the checkpoint, so a withdrawal cannot strand earned rewards
		// and the reduced stake stops accruing from this index.
		let index = pool_state.reward_index.get();
		let banked = position_state
			.pending_rewards
			.get()
			.checked_add(accrued_rewards(
				staked_amount,
				index
					.checked_sub(position_state.reward_debt.get())
					.ok_or(StakingError::RewardIndexRegressed)?,
			)?)
			.ok_or(ProgramError::ArithmeticOverflow)?;

		position_state.pending_rewards.set(banked);
		position_state.reward_debt.set(index);
		position_state.staked_amount.set(
			staked_amount
				.checked_sub(amount)
				.ok_or(ProgramError::ArithmeticOverflow)?,
		);
		pool_state.total_staked.set(
			total_staked
				.checked_sub(amount)
				.ok_or(ProgramError::ArithmeticOverflow)?,
		);

		let pool_bump = pool_state.bump;
		let pool_stake_mint = pool_state.stake_mint;
		let pool_reward_mint = pool_state.reward_mint;
		drop(position_state);
		drop(pool_state);

		// Return the principal from the pool's stake vault. The pool is the
		// vault's authority, so the pool PDA signs.
		let pool_seeds = PoolState::seeds(&pool_stake_mint, &pool_reward_mint).with_bump(pool_bump);
		let signer = pool_seeds.to_signer();
		let signers = [signer.as_signer()];

		let stake_decimals = {
			let mint = self
				.stake_mint
				.as_token_mint_for_program(self.token_program.address())?
				.assert_no_extensions()?;
			mint.decimals()
		};

		token::instructions::TransferChecked::new(
			self.stake_vault,
			self.stake_mint,
			self.user_stake_ata,
			self.pool_state,
			amount,
			stake_decimals,
		)
		.invoke_signed_with_program(&signers, self.token_program.address())?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for ClaimAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		// Validate accounts
		self.user.assert_signer()?;
		self.reward_mint.assert_owners(&SPL_PROGRAM_IDS)?;
		self.associated_token_program
			.assert_address(&associated_token_account::ID)?;
		self.system_program.assert_address(&system::ID)?;
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		self.pool_state.assert_not_empty()?;
		self.position_state.assert_not_empty()?;
		// The address check lives in the `CreateIdempotent` CPI below: the
		// associated token program derives the same seeds and rejects a mismatch
		// with `InvalidSeeds` before its idempotent branch.
		self.user_reward_ata.assert_writable()?;
		// The payout is signed by the pool, so the source must provably be the
		// pool's own reward vault. Without the derived-address check a caller
		// could name any token account they control as the vault and the pool
		// signature would authorise draining it.
		self.reward_vault
			.assert_not_empty()?
			.assert_writable()?
			.assert_owners(&SPL_PROGRAM_IDS)?
			.assert_associated_token_address(
				self.pool_state.address(),
				self.reward_mint.address(),
				self.token_program.address(),
			)?;

		// Validate pool and position state, then update through the same position
		// guard. Reloading it immutably first would validate it twice.
		let pool_handle = *self.pool_state;
		let user_handle = *self.user;
		let pool_state = self.pool_state.as_account::<PoolState>(&ID)?;
		let mut position_state = self.position_state.as_account_mut::<PositionState>(&ID)?;

		if pool_state.paused.get() {
			return Err(StakingError::PoolPaused.into());
		}

		assert_pool_reward_mint(&pool_state, *self.reward_mint)?;
		assert_position_access(pool_handle, user_handle, &position_state)?;

		// Accrue the rewards earned since this position's last checkpoint, then
		// advance the checkpoint to the current index. Zeroing `pending_rewards`
		// and moving the debt together is what makes a second claim in the same
		// state pay zero instead of paying the same index again.
		let index = pool_state.reward_index.get();
		let delta = index
			.checked_sub(position_state.reward_debt.get())
			.ok_or(StakingError::RewardIndexRegressed)?;
		let payout = position_state
			.pending_rewards
			.get()
			.checked_add(accrued_rewards(position_state.staked_amount.get(), delta)?)
			.ok_or(ProgramError::ArithmeticOverflow)?;

		if payout == 0 {
			return Err(StakingError::NothingToClaim.into());
		}

		position_state.pending_rewards.set(0);
		position_state.reward_debt.set(index);
		let pool_bump = pool_state.bump;
		let pool_stake_mint = pool_state.stake_mint;
		let pool_reward_mint = pool_state.reward_mint;
		drop(position_state);
		drop(pool_state);

		// Ensure user's reward ATA exists
		associated_token_account::instructions::CreateIdempotent {
			funding_account: self.user,
			account: self.user_reward_ata,
			wallet: self.user,
			mint: self.reward_mint,
			system_program: self.system_program,
			token_program: self.token_program,
		}
		.invoke()?;

		// Release the accrued rewards from the pool's reward vault. The pool is
		// the vault's authority, so the pool PDA signs.
		let pool_seeds = PoolState::seeds(&pool_stake_mint, &pool_reward_mint).with_bump(pool_bump);
		let signer = pool_seeds.to_signer();
		let signers = [signer.as_signer()];

		let reward_decimals = {
			let mint = self
				.reward_mint
				.as_token_mint_for_program(self.token_program.address())?
				.assert_no_extensions()?;
			mint.decimals()
		};

		token::instructions::TransferChecked::new(
			self.reward_vault,
			self.reward_mint,
			self.user_reward_ata,
			self.pool_state,
			payout,
			reward_decimals,
		)
		.invoke_signed_with_program(&signers, self.token_program.address())?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for SetRewardIndexAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = SetRewardIndexInstruction::try_from_bytes(data)?;
		let new_index = args.new_index.get();

		self.admin.assert_signer()?;
		self.pool_state.assert_not_empty()?;

		let pool_key = *self.pool_state.address();
		let mut pool_state = self.pool_state.as_account_mut::<PoolState>(&ID)?;
		self.admin.assert_address(&pool_state.admin)?;

		let current = pool_state.reward_index.get();
		// Monotonicity is the security core of this instruction: a lower index
		// would let every position re-claim the rewards it already released,
		// draining the vault repeatedly.
		if new_index < current {
			return Err(StakingError::RewardIndexRegressed.into());
		}

		// The update is a promise to pay, so it must clear two more gates
		// before the index moves. Both bound the aggregate liability of every
		// position at the new index: `total_staked * new_index / SCALE` is the
		// maximum any position set can be owed from genesis (each position's
		// own reward debt only shrinks what it is still owed), computed in
		// `u128` where `u64 * u64` always fits.
		let total_staked = pool_state.total_staked.get();
		let scaled = u128::from(total_staked)
			.checked_mul(u128::from(new_index))
			.ok_or(ProgramError::ArithmeticOverflow)?;
		let liability = scaled
			.checked_div(u128::from(REWARD_INDEX_SCALE))
			.ok_or(ProgramError::ArithmeticOverflow)?;

		// Gate one — representability: a liability no `u64` payout can hold
		// freezes the affected positions at their next checkpoint (deposit,
		// withdrawal, and claim would all overflow), so it is refused here.
		if liability > u128::from(u64::MAX) {
			return Err(StakingError::RewardIndexExceedsCapacity.into());
		}

		// The vault must belong to the pool's own stored reward mint: a
		// mismatched mint would validate the liability against a token
		// account denominated in units the index never promised.
		assert_pool_reward_mint(&pool_state, *self.reward_mint)?;
		// Gate two — solvency: the canonical reward vault must cover the
		// aggregate liability, so equal entitlements never depend on claim
		// order and the pool cannot promise rewards it does not hold. The
		// vault is validated as the pool PDA's associated account below, so a
		// caller cannot substitute a token account they control.
		self.reward_vault
			.assert_not_empty()?
			.assert_owners(&SPL_PROGRAM_IDS)?
			.assert_associated_token_address(
				&pool_key,
				self.reward_mint.address(),
				self.token_program.address(),
			)?;
		let vault_balance = u128::from(
			self.reward_vault
				.as_token_account_for_program(self.token_program.address())?
				.amount(),
		);
		if liability > vault_balance {
			return Err(StakingError::RewardIndexExceedsReserves.into());
		}

		pool_state.reward_index.set(new_index);

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn accrued_rewards_scale_and_floor() {
		// One index unit (a full reward token per staked token) over 100 staked
		// releases 100 reward tokens.
		assert_eq!(
			accrued_rewards(100, REWARD_INDEX_SCALE).unwrap_or_default(),
			100
		);
		// Half an index unit releases half.
		assert_eq!(
			accrued_rewards(100, REWARD_INDEX_SCALE / 2).unwrap_or_default(),
			50
		);
		// No index movement releases nothing, however large the stake.
		assert_eq!(accrued_rewards(u64::MAX, 0).unwrap_or_default(), 0);
		// A fractional result floors rather than rounds up.
		assert_eq!(accrued_rewards(1, 1).unwrap_or_default(), 0);
	}

	#[test]
	fn accrued_rewards_does_not_overflow_a_large_stake() {
		// index_delta * staked would overflow a u64; the u128 intermediate
		// keeps the product exact before the scaling division.
		let reward = accrued_rewards(u64::MAX, REWARD_INDEX_SCALE).unwrap_or_default();
		assert_eq!(reward, u64::MAX);
	}

	#[test]
	fn discriminator_values() {
		assert_eq!(StakingInstruction::InitializePool as u8, 0);
		assert_eq!(StakingInstruction::OpenPosition as u8, 1);
		assert_eq!(StakingInstruction::Deposit as u8, 2);
		assert_eq!(StakingInstruction::Withdraw as u8, 3);
		assert_eq!(StakingInstruction::Claim as u8, 4);
		assert_eq!(StakingInstruction::SetRewardIndex as u8, 5);
	}

	#[test]
	fn error_codes_are_stable() {
		// These values are program ABI: clients match on them, so a reorder
		// must be a deliberate, documented change.
		assert_eq!(StakingError::InvalidAmount as u32, 0);
		assert_eq!(StakingError::PoolPaused as u32, 1);
		assert_eq!(StakingError::InsufficientBalance as u32, 2);
		assert_eq!(StakingError::Unauthorized as u32, 3);
		assert_eq!(StakingError::InvalidPool as u32, 4);
		assert_eq!(StakingError::RewardIndexRegressed as u32, 5);
		assert_eq!(StakingError::NothingToClaim as u32, 6);
	}

	#[test]
	fn set_reward_index_instruction_roundtrip() {
		let mut bytes = [0u8; SetRewardIndexInstruction::SIZE];
		SetRewardIndexInstruction::initialize(&mut bytes, |instruction| {
			instruction.new_index.set(REWARD_INDEX_SCALE);
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialize: {error:?}"));
		let decoded = SetRewardIndexInstruction::try_from_bytes(&bytes)
			.unwrap_or_else(|e| panic!("decode: {e:?}"));
		assert_eq!(decoded.new_index.get(), REWARD_INDEX_SCALE);
	}

	#[test]
	fn instruction_roundtrip() {
		let mut bytes = [0u8; DepositInstruction::SIZE];
		DepositInstruction::initialize(&mut bytes, |instruction| {
			instruction.amount.set(50);
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialize failed: {error:?}"));
		let parsed = DepositInstruction::try_from_bytes(&bytes)
			.unwrap_or_else(|e| panic!("decode failed: {e:?}"));
		assert_eq!(parsed.amount.get(), 50);
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [5u8; 32].into();
		let data = [StakingInstruction::InitializePool as u8, 0];
		let result = parse_instruction::<StakingInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}
}
