use pina::*;

#[account(discriminator = TestAccount)]
#[pda(seeds = [b"test", authority: String], bump = bump)]
pub struct TestAccount {
	pub authority: String,
	pub bump: u8,
}

fn main() {}
