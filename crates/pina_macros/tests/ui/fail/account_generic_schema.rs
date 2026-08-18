use pina::*;

#[discriminator]
pub enum Kind {
	Generic = 0,
}

#[account(discriminator = Kind)]
pub struct Generic<T> {
	pub value: T,
}

fn main() {}
