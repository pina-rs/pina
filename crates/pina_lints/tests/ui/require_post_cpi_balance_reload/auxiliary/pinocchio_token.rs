#![allow(dead_code)]

use core::marker::PhantomData;

/// Mirrors `pinocchio_token::TokenProgram`, the legacy program marker.
pub struct TokenProgram;

pub mod instructions {
	pub struct TransferChecked;

	impl TransferChecked {
		pub fn new<T>(_: &T, _: &T, _: &T, _: &T, _: u64, _: u8) -> Self {
			Self
		}

		pub fn invoke(&self) -> Result<(), ()> {
			Ok(())
		}
	}

	/// Mirrors the real crate's program-generic builder aliases.
	pub type LegacyTransfer = super::generic::Transfer<super::TokenProgram>;
}

pub mod generic {
	use super::PhantomData;

	pub struct Transfer<Program>(PhantomData<Program>);

	impl<Program> Transfer<Program> {
		pub fn new<T>(_: &T, _: &T, _: &T, _: u64) -> Self {
			Self(PhantomData)
		}

		pub fn invoke(&self) -> Result<(), ()> {
			Ok(())
		}

		pub fn invoke_with_program<T>(&self, _: &T) -> Result<(), ()> {
			Ok(())
		}
	}
}
