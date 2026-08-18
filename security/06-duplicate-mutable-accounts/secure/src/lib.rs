//! SECURE: Source authorization and duplicate account checks enforced.
//!
//! This program verifies that the signer owns the source balance and that the
//! source and destination accounts are distinct before processing a transfer.

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
	pub amount: u64,
}

#[instruction(discriminator = LedgerInstruction, variant = Transfer)]
pub struct TransferInstruction {
	pub amount: u64,
}

#[derive(Accounts, Debug)]
pub struct TransferAccounts<'a> {
	pub authority: &'a AccountView,
	pub source: &'a mut AccountView,
	pub dest: &'a mut AccountView,
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

fn validate_source_authority(authority: &Address, source_owner: &Address) -> ProgramResult {
	if authority == source_owner {
		return Ok(());
	}

	Err(ProgramError::InvalidAccountData)
}

impl<'a> ProcessAccountInfos<'a> for TransferAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = TransferInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		self.source.assert_writable()?.assert_type::<Balance>(&ID)?;
		self.dest.assert_writable()?.assert_type::<Balance>(&ID)?;

		// SECURE: Verify source and destination are different accounts.
		if self.source.address() == self.dest.address() {
			return Err(LedgerError::DuplicateAccounts.into());
		}

		// SECURE: Only the source balance owner may authorize the transfer.
		let source_owner = self.source.as_account::<Balance>(&ID)?.owner;
		validate_source_authority(self.authority.address(), &source_owner)?;

		let amount = args.amount.get();

		let mut source = self.source.as_account_mut::<Balance>(&ID)?;
		let source_amount = source.amount.get();

		let mut dest = self.dest.as_account_mut::<Balance>(&ID)?;
		let dest_amount = dest.amount.get();

		let (new_source, new_dest) = checked_transfer_balances(source_amount, dest_amount, amount)?;
		source.amount.set(new_source);
		dest.amount.set(new_dest);

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

	#[test]
	fn validate_source_authority_rejects_unrelated_signer() {
		let source_owner = Address::new_from_array([1; 32]);
		let unrelated_signer = Address::new_from_array([2; 32]);

		let result = validate_source_authority(&unrelated_signer, &source_owner);

		assert_eq!(result, Err(ProgramError::InvalidAccountData));
	}

	#[test]
	fn validate_source_authority_accepts_owner() {
		let source_owner = Address::new_from_array([1; 32]);

		let result = validate_source_authority(&source_owner, &source_owner);

		assert_eq!(result, Ok(()));
	}
}
