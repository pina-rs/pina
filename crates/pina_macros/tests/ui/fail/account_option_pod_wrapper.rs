use pina::*;

#[discriminator]
pub enum Kind {
	OptionalPod = 0,
}

#[account(discriminator = Kind)]
pub struct OptionalPod {
	pub value: Option<PodU64>,
}

fn main() {}
