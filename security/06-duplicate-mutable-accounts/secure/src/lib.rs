//! SECURE: Duplicate account check enforced.
//!
//! This program verifies that source and destination accounts are distinct
//! before processing a transfer.

#![no_std]

#[cfg(all(not(any(target_os = "solana", target_arch = "bpf")), not(test)))]
extern crate std;

use pina::*;

declare_id!("BQrm6HUK9J6GRn6Pk7Gz7bu7RegbdseodfbBdHf8topX");

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerError {
	DuplicateAccounts = 0,
}

#[discriminator]
pub enum LedgerInstruction {
	Transfer = 0,
}

#[discriminator]
pub enum LedgerAccount {
	Balance = 1,
}

#[account(discriminator = LedgerAccount)]
pub struct Balance {
	pub owner: Address,
	pub amount: PodU64,
}

#[instruction(discriminator = LedgerInstruction, variant = Transfer)]
pub struct TransferInstruction {
	pub amount: PodU64,
}

#[derive(Accounts, Debug)]
pub struct TransferAccounts<'a> {
	pub authority: &'a AccountView,
	pub source: &'a AccountView,
	pub dest: &'a AccountView,
}

fn checked_transfer_balances(
	source_amount: u64,
	dest_amount: u64,
	amount: u64,
) -> Result<(u64, u64), ProgramError> {
	let new_source = source_amount
		.checked_sub(amount)
		.ok_or(ProgramError::InsufficientFunds)?;
	let new_dest = dest_amount
		.checked_add(amount)
		.ok_or(ProgramError::ArithmeticOverflow)?;

	Ok((new_source, new_dest))
}

impl<'a> ProcessAccountInfos<'a> for TransferAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = TransferInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		self.source.assert_writable()?.assert_type::<Balance>(&ID)?;
		self.dest.assert_writable()?.assert_type::<Balance>(&ID)?;

		// SECURE: Verify source and destination are different accounts.
		if self.source.address() == self.dest.address() {
			return Err(LedgerError::DuplicateAccounts.into());
		}

		let amount: u64 = args.amount.into();

		let source = self.source.as_account_mut::<Balance>(&ID)?;
		let source_amount: u64 = source.amount.into();

		let dest = self.dest.as_account_mut::<Balance>(&ID)?;
		let dest_amount: u64 = dest.amount.into();
		let (new_source, new_dest) = checked_transfer_balances(source_amount, dest_amount, amount)?;
		source.amount = PodU64::from_primitive(new_source);
		dest.amount = PodU64::from_primitive(new_dest);

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn checked_transfer_balances_rejects_insufficient_funds() {
		let result = checked_transfer_balances(3, 10, 4);
		assert_eq!(result, Err(ProgramError::InsufficientFunds));
	}

	#[test]
	fn checked_transfer_balances_rejects_destination_overflow() {
		let result = checked_transfer_balances(10, u64::MAX, 1);
		assert_eq!(result, Err(ProgramError::ArithmeticOverflow));
	}

	#[test]
	fn checked_transfer_balances_transfers_exact_amount() {
		let result = checked_transfer_balances(10, 4, 3);
		assert_eq!(result, Ok((7, 7)));
	}
}
