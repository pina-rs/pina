use pina::*;

#[derive(Accounts)]
struct InvalidAccounts<'a> {
	#[pina(validate(singner))]
	authority: &'a AccountView,
}

fn main() {}
