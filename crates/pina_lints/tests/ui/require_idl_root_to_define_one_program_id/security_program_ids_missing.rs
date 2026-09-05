// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

// An IDL-oriented crate without any program id fails the IDL contract even
// though it compiles other items.

pub const SEED_STATE: &[u8] = b"state";

fn entrypoint() -> Result<(), ()> {
	Ok(())
}

fn main() {}
