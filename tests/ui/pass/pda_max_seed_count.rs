use pina::*;

#[discriminator]
pub enum TestAccountType {
	TestAccount = 0,
}

/// A PDA at the maximum seed count: 15 declared seeds plus the bump seed.
#[account(discriminator = TestAccountType)]
#[pda(
	seeds = [
		b"a",
		b"b",
		b"c",
		b"d",
		b"e",
		b"f",
		b"g",
		b"h",
		b"i",
		b"j",
		b"k",
		b"l",
		b"m",
		b"n",
		b"o",
	],
	bump = bump
)]
pub struct TestAccount {
	pub bump: u8,
}

fn main() {
	let seeds = TestAccount::seeds();
	// 15 seeds without the bump, 16 with it: the runtime accepts the latter.
	assert_eq!(seeds.as_slices().len(), 15);
	assert_eq!(seeds.with_bump(1).as_slices().len(), 16);
}
