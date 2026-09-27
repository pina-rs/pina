//! Comparison rules are the canonical spelling and raise no deprecation.
//!
//! This fixture is the counterpart to `validation_deprecated_bounds.rs`: the
//! whole vocabulary is silent under `#![deny(deprecated)]`, including chained
//! bounds and the `!=` check the named bounds could not express.

#![deny(deprecated)]

use pina::*;

#[discriminator(primitive = u8)]
enum Kind {
	State = 1,
}

#[account(discriminator = Kind::State)]
struct CanonicalAccount {
	#[pina(validate(value >= 1 && value <= u64::MAX))]
	value: u64,
	#[pina(validate(len == 4))]
	tag: [u8; 4],
	#[pina(validate(len >= 2 && len <= 16))]
	label: String<16>,
	#[pina(validate(value != 0))]
	counter: u32,
}

#[discriminator(primitive = u8)]
enum Update {
	Memo = 2,
}

#[instruction(discriminator = Update::Memo)]
struct CanonicalInstruction {
	#[pina(validate(len >= 2 && len <= 64))]
	memo: String<64>,
}

fn main() {}
