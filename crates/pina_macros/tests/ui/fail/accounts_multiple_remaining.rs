use pina::*;

#[derive(Accounts)]
pub struct DuplicateRemainingAccounts<'a> {
	pub payer: &'a AccountView,
	#[pina(remaining)]
	pub rest: &'a [AccountView],
	#[pina(remaining)]
	pub trailing: &'a [AccountView],
}

fn main() {}
