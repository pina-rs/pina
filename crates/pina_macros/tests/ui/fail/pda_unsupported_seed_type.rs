use pina::*;

#[account(discriminator = TestAccount)]
#[pda(seeds = [b"test", authority: bool], bump = bump)]
pub struct TestAccount {
	pub authority: bool,
	pub bump: u8,
}

fn main() {}
