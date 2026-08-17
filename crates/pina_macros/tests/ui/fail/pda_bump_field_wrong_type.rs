use pina::*;

#[account(discriminator = TestAccount)]
#[pda(seeds = [b"test", authority: Address], bump = bump)]
pub struct TestAccount {
	pub authority: Address,
	pub bump: PodU64,
}

fn main() {}
