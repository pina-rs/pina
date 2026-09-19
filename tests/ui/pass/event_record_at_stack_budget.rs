//! The largest event record the generated `emit` helper can build on the SBF
//! stack compiles, so the budget rejects only oversized schemas.

use pina::*;

#[discriminator]
pub enum Kind {
	AtBudget = 0,
}

/// A `u8` discriminator is one byte, so this payload puts the record exactly at
/// `MAX_EVENT_RECORD_BYTES`: 1 (discriminator) + 3583 (payload).
#[event(discriminator = Kind)]
pub struct AtBudget {
	pub payload: [u8; 3583],
}

// The boundary is inclusive: a record exactly at the budget still compiles,
// and the arithmetic agrees with the public constant.
const _: () = assert!(AtBudget::SIZE == MAX_EVENT_RECORD_BYTES);
const _: () = assert!(MAX_EVENT_RECORD_BYTES == 4096 - 512);

fn main() {}
