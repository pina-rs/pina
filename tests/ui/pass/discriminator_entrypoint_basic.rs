//! The generated dispatch must compile and derive `MAX_INSTRUCTION_ACCOUNTS`
//! from every route's declared bound, including a per-variant override and a
//! trailing `remaining` slice.

use pina::*;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator(entrypoint)]
pub enum Instruction {
	Initialize = 0,
	#[dispatch(accounts = CustomAccounts)]
	Overridden = 1,
	Sweep = 2,
}

#[derive(Accounts)]
pub struct InitializeAccounts<'a> {
	pub authority: &'a AccountView,
	pub state: &'a mut AccountView,
}

#[derive(Accounts)]
pub struct CustomAccounts<'a> {
	pub authority: &'a AccountView,
}

#[derive(Accounts)]
pub struct SweepAccounts<'a> {
	pub authority: &'a AccountView,
	#[pina(remaining)]
	pub rest: &'a [AccountView],
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for CustomAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for SweepAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		Ok(())
	}
}

fn main() {
	// The largest route is `Initialize` with two positional accounts; `Sweep`
	// declares one positional account plus one `remaining` slot.
	assert_eq!(Instruction::MAX_INSTRUCTION_ACCOUNTS, 2);
	assert_eq!(
		<InitializeAccounts<'static> as ParseAccounts<'static>>::ACCOUNT_BOUND,
		2,
	);
	assert_eq!(
		<CustomAccounts<'static> as ParseAccounts<'static>>::ACCOUNT_BOUND,
		1,
	);
	assert_eq!(
		<SweepAccounts<'static> as ParseAccounts<'static>>::ACCOUNT_BOUND,
		2,
	);
}
