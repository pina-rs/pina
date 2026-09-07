use pina::*;

// PinaPod v0.2 migration: move this fixture to `pass` only after fixed vectors initialize
// their complete storage representation.
#[discriminator]
pub enum Kind {
	Push = 0,
}

#[instruction(discriminator = Kind)]
pub struct Push {
	pub values: Vec<u64, 8>,
}

fn main() {}
