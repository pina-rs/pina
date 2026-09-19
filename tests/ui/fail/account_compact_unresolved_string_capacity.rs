use pina::*;

#[discriminator]
pub enum Kind {
	Notes = 0,
}

#[account(discriminator = Kind, compact)]
pub struct Notes {
	pub bump: u8,
	pub title: String<MISSING_CAPACITY>,
}

fn main() {}
