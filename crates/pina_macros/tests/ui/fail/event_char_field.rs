use pina::*;

#[discriminator]
pub enum Kind {
	Character = 0,
}

#[event(discriminator = Kind)]
pub struct Character {
	pub value: char,
}

fn main() {}
