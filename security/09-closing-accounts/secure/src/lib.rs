//! SECURE: Proper account closing with data zeroing.
//!
//! This program zeros account data before closing, preventing revival attacks.

#![no_std]

#[cfg(all(not(any(target_os = "solana", target_arch = "bpf")), not(test)))]
extern crate std;

use pina::*;

declare_id!("8cr7r5t7GHnejNBds8QWnuQSosL8RrkiRxjbyQJ9ALjg");

#[discriminator]
pub enum RewardInstruction {
	ClaimAndClose = 0,
}

#[discriminator]
pub enum RewardAccount {
	RewardState = 1,
}

#[account(discriminator = RewardAccount)]
pub struct RewardState {
	pub authority: Address,
	pub claimed: u64,
}

#[instruction(discriminator = RewardInstruction, variant = ClaimAndClose)]
pub struct ClaimAndCloseInstruction {}

#[derive(Accounts, Debug)]
pub struct ClaimAndCloseAccounts<'a> {
	pub authority: &'a AccountView,
	pub reward: &'a mut AccountView,
	pub recipient: &'a mut AccountView,
}

impl<'a> ProcessAccountInfos<'a> for ClaimAndCloseAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = ClaimAndCloseInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		self.reward
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<RewardState>(&ID)?;

		let reward_authority = {
			let reward = self.reward.as_account::<RewardState>(&ID)?;
			reward.authority
		};

		self.authority.assert_address(&reward_authority)?;

		// SECURE: clear the raw backing bytes, then close and reclaim rent.
		self.reward.close_account_zeroed(self.recipient)
	}
}
