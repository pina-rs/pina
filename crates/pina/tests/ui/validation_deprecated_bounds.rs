//! The deprecated named bounds in validation grammar.
//!
//! Comparisons (`value >= 1`, `len == 4`) are the canonical spelling. The
//! named bounds still compile and generate the identical checks, but they
//! warn at the parameter the author wrote — `#[warn(deprecated)]` is on by
//! default and this suite promotes it to an error so the diagnostic is pinned
//! byte for byte.
#![deny(deprecated)]

use pina::*;

#[discriminator(primitive = u8)]
enum Kind {
	State = 1,
}

#[account(discriminator = Kind::State)]
struct LegacyAccount {
	#[pina(validate(min = 1, max = 10))]
	value: u64,
	#[pina(validate(exact_len = 4))]
	tag: [u8; 4],
}

#[discriminator(primitive = u8)]
enum Update {
	Memo = 2,
}

#[instruction(discriminator = Update::Memo)]
struct LegacyInstruction {
	#[pina(validate(min_len = 2, max_len = 8))]
	memo: String<16>,
}

fn main() {}
