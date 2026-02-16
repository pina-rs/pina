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
	pub claimed: PodU64,
}

#[instruction(discriminator = RewardInstruction, variant = ClaimAndClose)]
pub struct ClaimAndCloseInstruction {}

#[derive(Accounts, Debug)]
pub struct ClaimAndCloseAccounts<'a> {
	pub authority: &'a AccountView,
	pub reward: &'a AccountView,
	pub recipient: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for ClaimAndCloseAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = ClaimAndCloseInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		self.reward
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<RewardState>(&ID)?;

		let reward = self.reward.as_account::<RewardState>(&ID)?;
		self.authority.assert_address(&reward.authority)?;

		// SECURE: Zero the account data first, then close properly.
		// zeroed() clears all bytes, preventing stale data reuse.
		// close_with_recipient() transfers lamports, resizes to 0, and closes.
		{
			self.reward.as_account_mut::<RewardState>(&ID)?.zeroed();
		}
		self.reward.close_with_recipient(self.recipient)
	}
}
