use pina::*;

#[derive(Clone, Copy)]
pub struct Address([u8; 32]);

unsafe impl ZcField for Address {
	type Pod = [u8; 32];
}

#[discriminator]
pub enum Kind {
	Shadowed = 0,
}

#[account(discriminator = Kind, compact)]
pub struct Shadowed {
	pub values: Vec<Address, 4>,
}

fn main() {}
