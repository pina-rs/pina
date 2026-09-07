use pina::*;

// PinaPod v0.2 migration: move this fixture to `pass` only after fixed strings initialize
// their complete storage representation.
#[discriminator]
pub enum Kind {
	Profile = 0,
}

#[account(discriminator = Kind)]
pub struct Profile {
	pub name: String<32>,
}

fn main() {}
