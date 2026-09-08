use pina::*;

#[derive(Accounts)]
struct InvalidAccounts<'a> {
	#[pina(validate(writable))]
	state: &'a mut AccountView,
}

fn main() {}
