use pina::*;

#[derive(Accounts)]
pub struct DistinctWithoutRemaining<'a> {
	#[pina(distinct)]
	pub payer: &'a mut AccountView,
}

fn main() {}
