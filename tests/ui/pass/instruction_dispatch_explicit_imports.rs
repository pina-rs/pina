//! A program with explicit imports — no `use pina::*` glob — must compile with
//! the generated dispatcher: the emitted arms resolve `ProcessAccountInfos`
//! through the configured crate path instead of the caller's scope.

extern crate pina;

use pina::AccountView;
use pina::Accounts;
use pina::ParseAccounts;
use pina::ProgramResult;
use pina::declare_id;
use pina::discriminator;
use pina::instruction_dispatch;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[instruction_dispatch]
#[discriminator]
pub enum CounterInstruction {
	Initialize = 0,
	Increment = 1,
}

fn main() {
	assert_eq!(MAX_INSTRUCTION_ACCOUNTS, 3);
	assert_eq!(
		<InitializeAccounts<'static> as ParseAccounts<'static>>::ACCOUNT_BOUND,
		3,
	);
}

#[derive(Accounts)]
pub struct InitializeAccounts<'a> {
	pub authority: &'a AccountView,
	pub counter: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts)]
pub struct IncrementAccounts<'a> {
	pub authority: &'a AccountView,
	pub counter: &'a mut AccountView,
}

impl<'a> ::pina::ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, _data: &[u8]) -> ::pina::ProgramResult {
		Ok(())
	}
}

impl<'a> ::pina::ProcessAccountInfos<'a> for IncrementAccounts<'a> {
	fn process(self, _data: &[u8]) -> ::pina::ProgramResult {
		Ok(())
	}
}
