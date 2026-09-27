//! Comparison rules are the canonical spelling and raise no deprecation.
//!
//! This fixture is the counterpart to `validation_deprecated_bounds.rs`: the
//! whole vocabulary is silent under `#![deny(deprecated)]`, including chained
//! bounds and the `!=` check the named bounds could not express.
//!
//! A chained range is the case a token-level test cannot catch. Rust has no
//! chained comparison, so `100 < value <= u64::MAX` only compiles because the
//! macro splits it into two `&&`-joined checks; listing one here keeps that
//! split honest.

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
	#[pina(validate(100 < value <= u64::MAX))]
	open_range: u64,
	#[pina(validate(4 < len <= 64))]
	memo: String<64>,
	#[pina(validate(u64::MIN <= value < u64::MAX))]
	fully_bounded: u64,
}

#[discriminator(primitive = u8)]
enum Update {
	Memo = 2,
}

#[instruction(discriminator = Update::Memo)]
struct CanonicalInstruction {
	#[pina(validate(len >= 2 && len <= 64))]
	memo: String<64>,
	#[pina(validate(1 <= len < 8))]
	short: String<8>,
}

fn main() {}
