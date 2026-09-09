// aux-build: pina.rs
// aux-build: pina_macros.rs
// aux-build: unchecked_macro.rs
// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

extern crate pina;
extern crate pina_macros;
extern crate unchecked_macro;

use pina_macros::Accounts;
use unchecked_macro::unchecked_external_remaining_mut;

#[derive(Accounts)]
struct DerivedAccounts;

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

fn process_unchecked_ufcs(cursor: &mut pina::traits::AccountsCursor) -> Result<(), ()> {
	let _ = pina::traits::AccountsCursor::remaining_mut(cursor)?;
	//~^ ERROR: direct mutable remaining-account access permits duplicate writable aliases
	Ok(())
}

fn process_unchecked_function_item(cursor: &mut pina::traits::AccountsCursor) -> Result<(), ()> {
	let remaining_mut = pina::traits::AccountsCursor::remaining_mut;
	//~^ ERROR: direct mutable remaining-account access permits duplicate writable aliases
	let _ = remaining_mut(cursor)?;
	Ok(())
}

macro_rules! unchecked_remaining_mut {
	($cursor:expr) => {
		$cursor.remaining_mut()
	};
}

fn process_unchecked_local_macro(cursor: &mut pina::traits::AccountsCursor) -> Result<(), ()> {
	let _ = unchecked_remaining_mut!(cursor)?;
	//~^ ERROR: direct mutable remaining-account access permits duplicate writable aliases
	Ok(())
}

fn process_unchecked_external_macro(cursor: &mut pina::traits::AccountsCursor) -> Result<(), ()> {
	let _ = unchecked_external_remaining_mut!(cursor)?;
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

fn process_unrelated_closure_call() {
	let _ = (|| 1)();
}

fn main() {}

// compile-fail
