//! Staking and rewards distribution scaffold built with pina.
//!
//! This example keeps a realistic staking lifecycle in place:
//! - initialize a rewards pool
//! - open per-user positions
//! - deposit and withdraw stake
//! - claim rewards against a position account
//!
//! The first scaffold focuses on deterministic account structure and IDL
//! extraction. Token transfer logic can be layered in after the accounting
//! contract is stable.

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
	use pina::*;

	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		accounts: &[AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: StakingInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			StakingInstruction::InitializePool => {
				InitializePoolAccounts::try_from(accounts)?.process(data)
			}
			StakingInstruction::OpenPosition => {
				OpenPositionAccounts::try_from(accounts)?.process(data)
			}
			StakingInstruction::Deposit => DepositAccounts::try_from(accounts)?.process(data),
			StakingInstruction::Withdraw => WithdrawAccounts::try_from(accounts)?.process(data),
			StakingInstruction::Claim => ClaimAccounts::try_from(accounts)?.process(data),
		}
	}
}

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StakingError {
	InvalidAmount = 0,
	PoolPaused = 1,
	InsufficientBalance = 2,
}

#[discriminator]
pub enum StakingInstruction {
	InitializePool = 0,
	OpenPosition = 1,
	Deposit = 2,
	Withdraw = 3,
	Claim = 4,
}

#[discriminator]
pub enum StakingAccountType {
	PoolState = 1,
	PositionState = 2,
}

#[account(discriminator = StakingAccountType)]
pub struct PoolState {
	pub admin: Address,
	pub stake_mint: Address,
	pub reward_mint: Address,
	pub total_staked: PodU64,
	pub reward_index: PodU64,
	pub paused: PodBool,
	pub bump: u8,
}

#[account(discriminator = StakingAccountType)]
pub struct PositionState {
	pub pool: Address,
	pub owner: Address,
	pub staked_amount: PodU64,
	pub reward_debt: PodU64,
	pub pending_rewards: PodU64,
	pub bump: u8,
}

#[instruction(discriminator = StakingInstruction, variant = InitializePool)]
pub struct InitializePoolInstruction {
	pub bump: u8,
}

#[instruction(discriminator = StakingInstruction, variant = OpenPosition)]
pub struct OpenPositionInstruction {
	pub bump: u8,
}

#[instruction(discriminator = StakingInstruction, variant = Deposit)]
pub struct DepositInstruction {
	pub amount: PodU64,
}

#[instruction(discriminator = StakingInstruction, variant = Withdraw)]
pub struct WithdrawInstruction {
	pub amount: PodU64,
}

#[instruction(discriminator = StakingInstruction, variant = Claim)]
pub struct ClaimInstruction {}

#[derive(Accounts, Debug)]
pub struct InitializePoolAccounts<'a> {
	pub admin: &'a AccountView,
	pub stake_mint: &'a AccountView,
	pub reward_mint: &'a AccountView,
	pub pool_state: &'a AccountView,
	pub stake_vault: &'a AccountView,
	pub reward_vault: &'a AccountView,
	pub system_program: &'a AccountView,
	pub token_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct OpenPositionAccounts<'a> {
	pub user: &'a AccountView,
	pub pool_state: &'a AccountView,
	pub position_state: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct DepositAccounts<'a> {
	pub user: &'a AccountView,
	pub stake_mint: &'a AccountView,
	pub pool_state: &'a AccountView,
	pub position_state: &'a AccountView,
	pub user_stake_ata: &'a AccountView,
	pub token_program: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct WithdrawAccounts<'a> {
	pub user: &'a AccountView,
	pub stake_mint: &'a AccountView,
	pub pool_state: &'a AccountView,
	pub position_state: &'a AccountView,
	pub user_stake_ata: &'a AccountView,
	pub token_program: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct ClaimAccounts<'a> {
	pub user: &'a AccountView,
	pub reward_mint: &'a AccountView,
	pub pool_state: &'a AccountView,
	pub position_state: &'a AccountView,
	pub user_reward_ata: &'a AccountView,
	pub token_program: &'a AccountView,
	pub system_program: &'a AccountView,
}

const POOL_SEED_PREFIX: &[u8] = b"pool";
const POSITION_SEED_PREFIX: &[u8] = b"position";
const SPL_PROGRAM_IDS: [Address; 2] = [token::ID, token_2022::ID];

#[macro_export]
macro_rules! pool_seeds {
	($stake_mint:expr, $reward_mint:expr) => {
		&[POOL_SEED_PREFIX, $stake_mint, $reward_mint]
	};
	($stake_mint:expr, $reward_mint:expr, $bump:expr) => {
		&[POOL_SEED_PREFIX, $stake_mint, $reward_mint, &[$bump]]
	};
}

#[macro_export]
macro_rules! position_seeds {
	($pool:expr, $owner:expr) => {
		&[POSITION_SEED_PREFIX, $pool, $owner]
	};
	($pool:expr, $owner:expr, $bump:expr) => {
		&[POSITION_SEED_PREFIX, $pool, $owner, &[$bump]]
	};
}

impl<'a> ProcessAccountInfos<'a> for InitializePoolAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		// Parse instruction and prepare PDA seeds
		let args = InitializePoolInstruction::try_from_bytes(data)?;
		let pool_seeds = pool_seeds!(
			self.stake_mint.address().as_ref(),
			self.reward_mint.address().as_ref()
		);
		let pool_seeds_with_bump = pool_seeds!(
			self.stake_mint.address().as_ref(),
			self.reward_mint.address().as_ref(),
			args.bump
		);

		// Validate accounts
		self.admin.assert_signer()?;
		self.stake_mint.assert_owners(&SPL_PROGRAM_IDS)?;
		self.reward_mint.assert_owners(&SPL_PROGRAM_IDS)?;
		self.system_program.assert_address(&system::ID)?;
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		self.pool_state
			.assert_empty()?
			.assert_writable()?
			.assert_seeds_with_bump(pool_seeds_with_bump, &ID)?;
		self.stake_vault
			.assert_empty()?
			.assert_writable()?
			.assert_associated_token_address(
				self.pool_state.address(),
				self.stake_mint.address(),
				self.token_program.address(),
			)?;
		self.reward_vault
			.assert_empty()?
			.assert_writable()?
			.assert_associated_token_address(
				self.pool_state.address(),
				self.reward_mint.address(),
				self.token_program.address(),
			)?;

		// Create the pool state account
		create_program_account_with_bump::<PoolState>(
			self.pool_state,
			self.admin,
			&ID,
			pool_seeds,
			args.bump,
		)?;

		// Initialize pool state
		let pool_state = self.pool_state.as_account_mut::<PoolState>(&ID)?;
		*pool_state = PoolState::builder()
			.admin(*self.admin.address())
			.stake_mint(*self.stake_mint.address())
			.reward_mint(*self.reward_mint.address())
			.total_staked(PodU64::from_primitive(0))
			.reward_index(PodU64::from_primitive(0))
			.paused(PodBool::from_bool(false))
			.bump(args.bump)
			.build();

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
	fn process(&self, data: &[u8]) -> ProgramResult {
		// Parse instruction and prepare PDA seeds
		let args = OpenPositionInstruction::try_from_bytes(data)?;
		let position_seeds = position_seeds!(
			self.pool_state.address().as_ref(),
			self.user.address().as_ref()
		);
		let position_seeds_with_bump = position_seeds!(
			self.pool_state.address().as_ref(),
			self.user.address().as_ref(),
			args.bump
		);

		// Validate accounts
		self.user.assert_signer()?;
		self.system_program.assert_address(&system::ID)?;
		self.pool_state
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<PoolState>(&ID)?;
		self.position_state
			.assert_empty()?
			.assert_writable()?
			.assert_seeds_with_bump(position_seeds_with_bump, &ID)?;

		// Check pool is not paused
		let pool_state = self.pool_state.as_account::<PoolState>(&ID)?;
		if bool::from(pool_state.paused) {
			return Err(StakingError::PoolPaused.into());
		}

		// Create the position account
		create_program_account_with_bump::<PositionState>(
			self.position_state,
			self.user,
			&ID,
			position_seeds,
			args.bump,
		)?;

		// Initialize position state
		let position_state = self.position_state.as_account_mut::<PositionState>(&ID)?;
		*position_state = PositionState::builder()
			.pool(*self.pool_state.address())
			.owner(*self.user.address())
			.staked_amount(PodU64::from_primitive(0))
			.reward_debt(PodU64::from_primitive(0))
			.pending_rewards(PodU64::from_primitive(0))
			.bump(args.bump)
			.build();

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for DepositAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		// Parse instruction data
		let args = DepositInstruction::try_from_bytes(data)?;
		let amount: u64 = args.amount.into();

		// Validate accounts
		self.user.assert_signer()?;
		self.stake_mint.assert_owners(&SPL_PROGRAM_IDS)?;
		self.system_program.assert_address(&system::ID)?;
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		self.pool_state
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<PoolState>(&ID)?;
		self.position_state
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<PositionState>(&ID)?;
		self.user_stake_ata
			.assert_writable()?
			.assert_associated_token_address(
				self.user.address(),
				self.stake_mint.address(),
				self.token_program.address(),
			)?;

		// Validate pool and position state
		let pool_state = self.pool_state.as_account::<PoolState>(&ID)?;
		let position_state = self.position_state.as_account::<PositionState>(&ID)?;

		if bool::from(pool_state.paused) {
			return Err(StakingError::PoolPaused.into());
		}
		if position_state.pool != *self.pool_state.address() {
			return Err(StakingError::InvalidAmount.into());
		}
		if position_state.owner != *self.user.address() {
			return Err(StakingError::InvalidAmount.into());
		}

		// Calculate updated amounts
		let staked_amount = u64::from(position_state.staked_amount);
		let next_staked = staked_amount
			.checked_add(amount)
			.ok_or(ProgramError::ArithmeticOverflow)?;
		let total_staked = u64::from(pool_state.total_staked)
			.checked_add(amount)
			.ok_or(ProgramError::ArithmeticOverflow)?;

		// Update position state
		let position_state = self.position_state.as_account_mut::<PositionState>(&ID)?;
		position_state.staked_amount = PodU64::from_primitive(next_staked);
		position_state.reward_debt = PodU64::from_primitive(
			u64::from(position_state.reward_debt)
				.checked_add(amount)
				.ok_or(ProgramError::ArithmeticOverflow)?,
		);

		// Update pool state
		let pool_state = self.pool_state.as_account_mut::<PoolState>(&ID)?;
		pool_state.total_staked = PodU64::from_primitive(total_staked);

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

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for WithdrawAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		// Parse instruction data
		let args = WithdrawInstruction::try_from_bytes(data)?;
		let amount: u64 = args.amount.into();

		// Validate accounts
		self.user.assert_signer()?;
		self.stake_mint.assert_owners(&SPL_PROGRAM_IDS)?;
		self.system_program.assert_address(&system::ID)?;
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		self.pool_state
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<PoolState>(&ID)?;
		self.position_state
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<PositionState>(&ID)?;
		self.user_stake_ata
			.assert_writable()?
			.assert_associated_token_address(
				self.user.address(),
				self.stake_mint.address(),
				self.token_program.address(),
			)?;

		// Validate pool and position state
		let pool_state = self.pool_state.as_account::<PoolState>(&ID)?;
		let position_state = self.position_state.as_account::<PositionState>(&ID)?;

		if bool::from(pool_state.paused) {
			return Err(StakingError::PoolPaused.into());
		}

		let staked_amount = u64::from(position_state.staked_amount);
		if amount > staked_amount {
			return Err(StakingError::InsufficientBalance.into());
		}

		// Update position state
		let position_state = self.position_state.as_account_mut::<PositionState>(&ID)?;
		position_state.staked_amount = PodU64::from_primitive(staked_amount - amount);

		// Update pool state
		let pool_state = self.pool_state.as_account_mut::<PoolState>(&ID)?;
		pool_state.total_staked =
			PodU64::from_primitive(u64::from(pool_state.total_staked) - amount);

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for ClaimAccounts<'a> {
	fn process(&self, _data: &[u8]) -> ProgramResult {
		// Validate accounts
		self.user.assert_signer()?;
		self.reward_mint.assert_owners(&SPL_PROGRAM_IDS)?;
		self.system_program.assert_address(&system::ID)?;
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		self.pool_state
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<PoolState>(&ID)?;
		self.position_state
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<PositionState>(&ID)?;
		self.user_reward_ata
			.assert_writable()?
			.assert_associated_token_address(
				self.user.address(),
				self.reward_mint.address(),
				self.token_program.address(),
			)?;

		// Validate pool and position state
		let pool_state = self.pool_state.as_account::<PoolState>(&ID)?;
		let position_state = self.position_state.as_account::<PositionState>(&ID)?;

		if bool::from(pool_state.paused) {
			return Err(StakingError::PoolPaused.into());
		}
		if position_state.owner != *self.user.address() {
			return Err(StakingError::InvalidAmount.into());
		}

		// Calculate and update pending rewards
		let next_pending = u64::from(position_state.pending_rewards)
			.checked_add(u64::from(pool_state.reward_index))
			.ok_or(ProgramError::ArithmeticOverflow)?;

		let position_state = self.position_state.as_account_mut::<PositionState>(&ID)?;
		position_state.pending_rewards = PodU64::from_primitive(next_pending);

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

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn discriminator_values() {
		assert_eq!(StakingInstruction::InitializePool as u8, 0);
		assert_eq!(StakingInstruction::OpenPosition as u8, 1);
		assert_eq!(StakingInstruction::Deposit as u8, 2);
		assert_eq!(StakingInstruction::Withdraw as u8, 3);
		assert_eq!(StakingInstruction::Claim as u8, 4);
	}

	#[test]
	fn instruction_roundtrip() {
		let ix = DepositInstruction::builder()
			.amount(PodU64::from_primitive(50))
			.build();
		let bytes = ix.to_bytes();
		let parsed = DepositInstruction::try_from_bytes(bytes)
			.unwrap_or_else(|e| panic!("decode failed: {e:?}"));
		assert_eq!(u64::from(parsed.amount), 50);
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [5u8; 32].into();
		let data = [StakingInstruction::InitializePool as u8];
		let result = parse_instruction::<StakingInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}
}
