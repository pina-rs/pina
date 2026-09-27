//! A migration ladder must resolve against the checked-in manifest. The
//! dispatch macro walks up from the expanding crate to find the program
//! directory, so this fixture — compiled from a harness crate with no
//! `migrations/manifest.json` anywhere above it — fails with the
//! unlocatable-manifest error instead of an unsatisfied `MigratableAccount`
//! bound at the `run_optional` call.

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
