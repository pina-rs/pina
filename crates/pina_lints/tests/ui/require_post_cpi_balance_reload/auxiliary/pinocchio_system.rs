#![allow(dead_code)]

//! A lamport transfer builder shaped like a token transfer: three accounts and
//! an amount. It lives in the system program crate, so it moves lamports.

pub mod instructions {
	pub struct Transfer;

	impl Transfer {
		pub fn new<T>(_: &T, _: &T, _: &T, _: u64) -> Self {
			Self
		}

		pub fn invoke(&self) -> Result<(), ()> {
			Ok(())
		}
	}
}
