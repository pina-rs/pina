//! INSECURE: Missing signer authorization check.
//!
//! This program modifies vault state without verifying that the authority
//! account actually signed the transaction.

#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("CibVRwrFQgic7kMsC4TGnrj82xCGxLjpWcK4fMkfK9y9");

#[discriminator]
pub enum VaultInstruction {
	Withdraw = 0,
}

#[discriminator]
pub enum VaultAccount {
	VaultState = 1,
}

#[account(discriminator = VaultAccount)]
pub struct VaultState {
	pub authority: Address,
	pub balance: PodU64,
}

#[instruction(discriminator = VaultInstruction, variant = Withdraw)]
pub struct WithdrawInstruction {
	pub amount: PodU64,
}

#[derive(Accounts, Debug)]
pub struct WithdrawAccounts<'a> {
	pub authority: &'a AccountView,
	pub vault: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for WithdrawAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = WithdrawInstruction::try_from_bytes(data)?;

		// BUG: No `assert_signer()` check on authority!
		// Anyone can pass any address as authority and withdraw funds.

		self.vault
			.assert_writable()?
			.assert_type::<VaultState>(&ID)?;

		let vault = self.vault.as_account_mut::<VaultState>(&ID)?;
		let current: u64 = vault.balance.into();
		let amount: u64 = args.amount.into();
		vault.balance = PodU64::from_primitive(current.saturating_sub(amount));

		Ok(())
	}
}
