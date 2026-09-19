use pina::*;

#[discriminator]
pub enum Kind {
	Words = 0,
}

// An associated constant cannot be evaluated during macro expansion.
struct Bounds;

impl Bounds {
	pub const WIDTH: usize = 4;
}

#[account(discriminator = Kind)]
pub struct Words {
	pub values: [u64; Bounds::WIDTH],
}

fn main() {}
