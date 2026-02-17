//! INSECURE: No duplicate account check.
//!
//! This program transfers value between two accounts without verifying they
//! are distinct.

#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("DDzrz2WGGpNv8zxgjKRRGmzonau8ydwB9CokzDwwgrw");

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

impl<'a> ProcessAccountInfos<'a> for TransferAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = TransferInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		self.source.assert_writable()?.assert_type::<Balance>(&ID)?;
		self.dest.assert_writable()?.assert_type::<Balance>(&ID)?;

		// BUG: No check that source != dest!
		// If the same account is passed for both, the debit and credit
		// operate on the same account.
		let amount: u64 = args.amount.into();

		let source = self.source.as_account_mut::<Balance>(&ID)?;
		let source_amount: u64 = source.amount.into();
		source.amount = PodU64::from_primitive(source_amount.saturating_sub(amount));

		let dest = self.dest.as_account_mut::<Balance>(&ID)?;
		let dest_amount: u64 = dest.amount.into();
		dest.amount = PodU64::from_primitive(dest_amount.saturating_add(amount));

		Ok(())
	}
}
