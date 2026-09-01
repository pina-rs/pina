use pina::*;

#[discriminator]
pub enum Kind {
	ManualDerive = 0,
}

#[account(discriminator = Kind)]
#[derive(ZeroPod)]
pub struct ManualDerive {
	pub value: u64,
}

fn main() {}
