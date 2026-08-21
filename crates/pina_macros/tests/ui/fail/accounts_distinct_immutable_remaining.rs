use pina::*;

#[derive(Accounts)]
pub struct DistinctImmutableRemaining<'a> {
	#[pina(remaining, distinct)]
	pub remaining: &'a [AccountView],
}

fn main() {}
