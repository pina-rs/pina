// aux-build: pina.rs
// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

extern crate pina;

struct ForeignCursor;

impl ForeignCursor {
	fn remaining_mut(&mut self) -> Result<&mut [u8], ()> {
		Ok(&mut [])
	}
}

fn process_unchecked(cursor: &mut pina::traits::AccountsCursor) -> Result<(), ()> {
	let _ = cursor.remaining_mut()?;
	//~^ ERROR: direct mutable remaining-account access permits duplicate writable aliases
	Ok(())
}

fn process_distinct(cursor: &mut pina::traits::AccountsCursor) -> Result<(), ()> {
	let _ = cursor.remaining_mut_distinct()?;
	Ok(())
}

fn process_foreign(cursor: &mut ForeignCursor) -> Result<(), ()> {
	let _ = cursor.remaining_mut()?;
	Ok(())
}

trait PrimitiveCursor {
	fn remaining_mut(&mut self);
}

impl PrimitiveCursor for u64 {
	fn remaining_mut(&mut self) {}
}

fn process_primitive(cursor: &mut u64) {
	cursor.remaining_mut();
}

fn main() {}

// compile-fail
