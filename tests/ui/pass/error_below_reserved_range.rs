#![deny(warnings)]

use pina::*;

const BASE: u32 = 6000;

#[error]
pub enum MyError {
	Implicit,
	Anchor = BASE,
	/// `0xFFFE_FFFF` is the highest code outside the reserved range.
	Highest = 0xFFFE_FFFF,
	/// A compiled-out variant has no discriminant, so it is never checked.
	#[cfg(any())]
	CompiledOut = 0xFFFF_0000,
	/// Checking a deprecated variant must not warn at a site the author never
	/// wrote.
	#[deprecated]
	Retired = 1,
}

fn main() {
	assert_eq!(
		ProgramError::from(MyError::Highest),
		ProgramError::Custom(0xFFFE_FFFF)
	);
}
