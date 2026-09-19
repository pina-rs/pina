use pina::*;

#[discriminator]
pub enum Kind {
	Roster = 0,
}

// An associated constant cannot be evaluated during macro expansion.
struct Bounds;

impl Bounds {
	pub const MAX_MEMBERS: usize = 24;
}

#[account(discriminator = Kind, compact)]
pub struct Roster {
	pub bump: u8,
	pub members: Vec<Address, Bounds::MAX_MEMBERS>,
}

fn main() {}
