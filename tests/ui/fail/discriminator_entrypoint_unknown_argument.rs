//! An unsupported argument on the dispatch attribute must be rejected with the
//! supported spelling, so the grammar stays discoverable.

use pina::*;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator(entrypoint, disptach = true)]
pub enum Instruction {
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
