//! An event record larger than `MAX_EVENT_RECORD_BYTES` cannot be built in the
//! `emit` frame without exhausting the SBF stack, so the generated assertion
//! must reject it at compile time.

use pina::*;

#[discriminator]
pub enum Kind {
	Oversized = 0,
}

#[event(discriminator = Kind)]
pub struct Oversized {
	pub payload: [u8; 4096],
}

fn main() {}
