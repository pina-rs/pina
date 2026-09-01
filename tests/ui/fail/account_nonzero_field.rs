use core::num::NonZeroU64;

use pina::*;

#[discriminator]
pub enum Kind {
	NonZero = 0,
}

#[account(discriminator = Kind)]
pub struct NonZero {
	pub value: NonZeroU64,
}

fn main() {}
