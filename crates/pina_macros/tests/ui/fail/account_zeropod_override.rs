use pina::*;

#[discriminator]
pub enum Kind {
	Overridden = 0,
}

#[account(discriminator = Kind)]
#[zeropod(crate = pina::zeropod)]
pub struct Overridden {
	pub value: u64,
}

fn main() {}
