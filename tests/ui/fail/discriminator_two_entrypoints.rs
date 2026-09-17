//! A program has one entrypoint, so a second `entrypoint` discriminator in the
//! same module must fail to compile.

use pina::*;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator(entrypoint)]
pub enum FirstInstruction {
	Run = 0,
}

#[discriminator(entrypoint)]
pub enum SecondInstruction {
	Run = 0,
}

#[derive(Accounts)]
pub struct RunAccounts<'a> {
	pub authority: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for RunAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		Ok(())
	}
}

fn main() {}
