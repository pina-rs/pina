use pina::*;

#[account(discriminator = TestAccount)]
#[pda(seeds = [b"test", authority: Address], bump = missing)]
pub struct TestAccount {
	pub authority: Address,
}

fn main() {}
