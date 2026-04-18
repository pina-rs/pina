use pina::*;

#[derive(Accounts)]
pub struct BasicAccounts<'a> {
	pub payer: &'a AccountView,
}

fn main() {}
