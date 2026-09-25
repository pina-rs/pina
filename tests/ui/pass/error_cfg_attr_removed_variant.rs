// The fixture crate declares no features, so naming `extra` would otherwise
// warn that the value is unexpected.
#![allow(unexpected_cfgs)]
#![deny(warnings)]

use pina::*;

#[error]
pub enum MyError {
	Valid = 0,
	/// `cfg_attr` can produce the `cfg` that removes a variant, so the
	/// reserved-range check must not name it when `extra` is disabled.
	#[cfg_attr(not(feature = "extra"), cfg(feature = "extra"))]
	Conditional = 0xFFFF_0000,
	/// Nested `cfg_attr` expands one level at a time.
	#[cfg_attr(all(), cfg_attr(not(feature = "extra"), cfg(feature = "extra")))]
	Nested = 0xFFFF_0001,
	/// Only the `cfg` a `cfg_attr` produces matters; the rest stays on the
	/// variant.
	#[cfg_attr(all(), doc = "Still documented.", cfg(feature = "extra"))]
	Mixed = 0xFFFF_0002,
}

fn main() {
	assert_eq!(ProgramError::from(MyError::Valid), ProgramError::Custom(0));
}
