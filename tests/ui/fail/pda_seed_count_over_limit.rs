use pina::*;

#[account(discriminator = TestAccount)]
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
		b"p",
	],
	bump = bump
)]
pub struct TestAccount {
	pub bump: u8,
}

fn main() {}
