use pina::*;

#[discriminator]
pub enum Kind {
	Words = 0,
}

#[account(discriminator = Kind)]
pub struct Words {
	pub values: [u16; 4],
}

fn main() {}
