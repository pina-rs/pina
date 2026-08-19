use pina::*;

#[derive(Clone, Copy)]
pub struct Address([u8; 32]);

impl ZcField for Address {
	type Pod = [u8; 32];

	const POD_SIZE: usize = 32;
}

#[discriminator]
pub enum Kind {
	Shadowed = 0,
}

#[account(discriminator = Kind)]
pub struct Shadowed {
	pub owner: Address,
}

fn main() {}
