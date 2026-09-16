use pina::*;

#[discriminator]
pub enum Kind {
	Words = 0,
}

const WIDTH: usize = 4;

#[account(discriminator = Kind)]
pub struct Words {
	pub values: [u64; WIDTH],
}

fn main() {}
