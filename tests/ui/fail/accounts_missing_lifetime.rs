use pina::*;

#[derive(Accounts)]
pub struct MissingLifetimeAccounts {
	pub payer: AccountView,
}

fn main() {}
