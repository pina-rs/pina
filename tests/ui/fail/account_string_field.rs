use pina::*;

#[discriminator]
pub enum Kind {
	Profile = 0,
}

#[account(discriminator = Kind)]
pub struct Profile {
	pub name: String<32>,
}

fn main() {}
