use pina::*;

#[discriminator]
pub enum Kind {
	State = 1,
}

#[account(discriminator = Kind, compact)]
pub struct State {
	pub values: Vec<Vec<u64, 2>, 4>,
}

fn main() {}
