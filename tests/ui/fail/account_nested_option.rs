use pina::*;

// PinaPod v0.2 migration: move this fixture to `pass` only after nested active values are
// validated recursively.
#[discriminator]
pub enum Kind {
	Nested = 0,
}

#[account(discriminator = Kind)]
pub struct Nested {
	pub value: Option<Option<u64>>,
}

fn main() {}
