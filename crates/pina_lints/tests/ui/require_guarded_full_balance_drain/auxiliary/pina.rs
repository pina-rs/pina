//! Stub of `pina::assert` for fixture-only use.
//!
//! The lint resolves the helper through the crate name, so the auxiliary is
//! named `pina` to stand in for the real dependency.

/// Returns `err` when `v` is false, like the real helper.
pub fn assert(v: bool, err: (), _msg: &str) -> Result<(), ()> {
	if v { Ok(()) } else { Err(err) }
}
