//! INSECURE: Missing sysvar address check.
//!
//! This program reads a sysvar account without verifying its address or owner.

#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("Eqf79Pwyhdm5F9eqE4JQByApm2rAXbNwx3mVvqzHLjsP");

#[discriminator]
pub enum RentInstruction {
	CheckRent = 0,
}

#[instruction(discriminator = RentInstruction, variant = CheckRent)]
pub struct CheckRentInstruction {}

#[derive(Accounts, Debug)]
pub struct CheckRentAccounts<'a> {
	pub user: &'a AccountView,
	pub rent_sysvar: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for CheckRentAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = CheckRentInstruction::try_from_bytes(data)?;

		self.user.assert_signer()?;

		// BUG: No verification that rent_sysvar is actually the rent sysvar!
		// An attacker can pass a fake account with manipulated data.
		// The program trusts whatever data is in this account.
		let _data = self.rent_sysvar.try_borrow()?;

		log!("Rent check passed (sysvar unverified)");

		Ok(())
	}
}
