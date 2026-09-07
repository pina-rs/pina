use pina::*;

#[discriminator]
pub enum Kind {
	Push = 0,
}

#[instruction(discriminator = Kind)]
pub struct Push {
	pub values: Vec<u64, 8>,
}

fn main() {}
