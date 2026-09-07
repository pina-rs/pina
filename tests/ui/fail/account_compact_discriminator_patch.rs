use pina::*;

#[discriminator]
pub enum Kind {
	State = 1,
}

#[account(discriminator = Kind, compact)]
pub struct State {
	pub values: Vec<u64, 4>,
}

fn main() {
	let _ = StatePatch::new().discriminator(2);
}
