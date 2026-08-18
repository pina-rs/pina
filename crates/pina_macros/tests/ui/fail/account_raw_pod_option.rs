use pina::*;

#[discriminator]
pub enum Kind {
	RawOption = 0,
}

#[account(discriminator = Kind)]
pub struct RawOption {
	pub value: PodOption<PodU64>,
}

fn main() {}
