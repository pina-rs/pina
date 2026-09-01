use pina::*;

#[account(discriminator = TestAccount)]
#[pda(seeds = [b"this seed literal is way too long to be a valid seed"], bump = bump)]
pub struct TestAccount {
	pub bump: u8,
}

fn main() {}
