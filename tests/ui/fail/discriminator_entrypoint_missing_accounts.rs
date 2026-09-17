//! A variant with no matching `Accounts` struct must fail with a diagnostic
//! that names the variant, not the attribute macro.

use pina::*;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator(entrypoint)]
pub enum Instruction {
	Present = 0,
	Absent = 1,
}

#[derive(Accounts)]
pub struct PresentAccounts<'a> {
	pub authority: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for PresentAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		Ok(())
	}
}

fn main() {}
