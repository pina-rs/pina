use pina::*;

#[discriminator(entrypoint)]
pub enum CounterInstruction {
	Initialize = 0,
	Increment = 1,
}

#[discriminator(entrypoint, capacity_test = false)]
pub enum OverrideInstruction {
	#[dispatch(accounts = IncrementAccounts)]
	Routed = 0,
	Untouched = 1,
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct InitializeAccounts<'a> {
	pub authority: &'a AccountView,
	pub counter: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct IncrementAccounts<'a> {
	pub authority: &'a AccountView,
	pub counter: &'a mut AccountView,
}
