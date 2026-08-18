use pina::*;

#[discriminator]
pub enum Kind {
	OptionalOwner = 0,
}

#[account(discriminator = Kind)]
pub struct OptionalOwner {
	pub owner: Option<Address>,
}

fn main() {}
