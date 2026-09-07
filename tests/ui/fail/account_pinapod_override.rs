use pina::*;

#[discriminator]
pub enum Kind {
	Overridden = 0,
}

#[account(discriminator = Kind)]
#[pinapod(crate = pina::pinapod)]
pub struct Overridden {
	pub value: u64,
}

fn main() {}
