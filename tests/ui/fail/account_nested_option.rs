use pina::*;

#[discriminator]
pub enum Kind {
	Nested = 0,
}

#[account(discriminator = Kind)]
pub struct Nested {
	pub value: Option<Option<u64>>,
}

fn main() {}
