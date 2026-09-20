//! A migration ladder must resolve against the checked-in manifest. The
//! dispatch macro reads the same policy source as `#[account]`, so a program
//! with no manifest fails with the `pina migrations create` remedy instead of an
//! unsatisfied `MigratableAccount` bound at the `run_optional` call.

use pina::*;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator(entrypoint, migrations(State), migrations_max_lamports = 20_000)]
pub enum Instruction {
	Update = 0,
}

#[account(discriminator = AccountType::State)]
pub struct State {
	pub value: u64,
}

#[discriminator]
pub enum AccountType {
	State = 1,
}

#[derive(Accounts)]
pub struct UpdateAccounts<'a> {
	pub state: &'a mut AccountView,
}

impl<'a> ProcessAccountInfos<'a> for UpdateAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		Ok(())
	}
}

fn main() {}
