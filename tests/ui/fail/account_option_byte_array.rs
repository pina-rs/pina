use pina::*;

#[discriminator]
pub enum Kind {
	OptionalBytes = 0,
}

#[account(discriminator = Kind)]
pub struct OptionalBytes {
	pub value: Option<[u8; 8]>,
}

fn main() {}
