//! SECURE: Signer authorization check present.
//!
//! This program verifies that the authority account signed the transaction
//! before allowing any state mutation.

#![no_std]

#[cfg(all(not(any(target_os = "solana", target_arch = "bpf")), not(test)))]
extern crate std;

use pina::*;

declare_id!("Fiowp2vKZUHi9yLjtmmshG8rPeV4P4hpG9NRpshhJsW4");

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

fn checked_withdraw_balance(current: u64, amount: u64) -> Result<u64, ProgramError> {
	current
		.checked_sub(amount)
		.ok_or(ProgramError::InsufficientFunds)
}

impl<'a> ProcessAccountInfos<'a> for WithdrawAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = WithdrawInstruction::try_from_bytes(data)?;

		// SECURE: Verify the authority signed this transaction.
		self.authority.assert_signer()?;

		self.vault
			.assert_writable()?
			.assert_type::<VaultState>(&ID)?;

		let vault = self.vault.as_account::<VaultState>(&ID)?;
		// Also verify the authority matches the vault's stored authority.
		self.authority.assert_address(&vault.authority)?;

		let vault = self.vault.as_account_mut::<VaultState>(&ID)?;
		let current: u64 = vault.balance.into();
		let amount: u64 = args.amount.into();
		vault.balance = PodU64::from_primitive(checked_withdraw_balance(current, amount)?);

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn checked_withdraw_balance_rejects_insufficient_funds() {
		let result = checked_withdraw_balance(5, 6);
		assert_eq!(result, Err(ProgramError::InsufficientFunds));
	}

	#[test]
	fn checked_withdraw_balance_allows_exact_balance() {
		let result = checked_withdraw_balance(7, 7);
		assert_eq!(result, Ok(0));
	}
}
