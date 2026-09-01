use pina::*;

#[derive(Accounts)]
pub struct RemainingNotLast<'a> {
	pub payer: &'a AccountView,
	#[pina(remaining)]
	pub rest: &'a [AccountView],
	pub system_program: &'a AccountView,
}

fn main() {}
