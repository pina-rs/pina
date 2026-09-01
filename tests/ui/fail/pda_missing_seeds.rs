use pina::*;

#[account(discriminator = TestAccount)]
#[pda(bump = bump)]
pub struct TestAccount {
	pub bump: u8,
}

fn main() {}
