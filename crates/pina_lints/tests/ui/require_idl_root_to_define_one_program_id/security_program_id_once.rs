// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

// The crate name (from the file stem) contains `security`, so this compile is
// treated as an IDL-oriented example crate and the lint requires exactly one
// crate-root `declare_id!` expansion.

macro_rules! declare_id {
	($id:expr) => {
		pub const ID: &str = $id;
	};
}

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

fn entrypoint() -> Result<(), ()> {
	Ok(())
}

fn main() {}
