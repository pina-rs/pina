//! SECURE: Sysvar address and owner verified.
//!
//! This program verifies the sysvar account's address and owner before
//! trusting its data.

#![no_std]

#[cfg(all(not(any(target_os = "solana", target_arch = "bpf")), not(test)))]
extern crate std;

use pina::*;

declare_id!("DZqNPRrdgKaHde5xZZDYnAAPWXGxF42uPubGEqJL4b6Z");

const RENT_SYSVAR_ID: Address = address!("SysvarRent111111111111111111111111111111111");

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

		// SECURE: Verify the sysvar's address and owner.
		// assert_sysvar checks both the owner (Sysvar program) and
		// the exact address of the sysvar account.
		self.rent_sysvar.assert_sysvar(&RENT_SYSVAR_ID)?;

		log!("Rent check passed (sysvar verified)");

		Ok(())
	}
}
