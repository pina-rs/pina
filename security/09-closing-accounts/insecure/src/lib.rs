//! INSECURE: Improper account closing.
//!
//! This program transfers lamports but doesn't zero account data or properly
//! close the account.

#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("47B21GXJ2xyBsGVoCbi8PifWu788oNMbzNmdwao3CyHU");

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

		// BUG: Only transfers lamports without zeroing data or closing.
		// The account can be revived within the same transaction by
		// sending lamports back to it. The stale data remains.
		self.reward.send(self.reward.lamports(), self.recipient)?;

		Ok(())
	}
}
