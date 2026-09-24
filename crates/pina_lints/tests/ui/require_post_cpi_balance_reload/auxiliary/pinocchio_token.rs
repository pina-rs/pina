#![allow(dead_code)]

//! Mirrors the shape of `pinocchio_token` 0.7: builders are generic over the
//! token program, and `instructions::*` re-exports them as aliases bound to the
//! legacy `TokenProgram`.

/// The legacy SPL Token program marker.
pub struct TokenProgram;

pub mod instructions {
	pub mod transfer_checked {
		use core::marker::PhantomData;

		pub struct TransferChecked<Program>(PhantomData<Program>);

		impl<Program> TransferChecked<Program> {
			pub fn new<T>(_: &T, _: &T, _: &T, _: &T, _: u64, _: u8) -> Self {
				Self(PhantomData)
			}

			pub fn invoke(&self) -> Result<(), ()> {
				Ok(())
			}

			pub fn invoke_signed(&self, _: &[u8]) -> Result<(), ()> {
				Ok(())
			}

			pub fn invoke_with_program<T>(&self, _: &T) -> Result<(), ()> {
				Ok(())
			}
		}
	}

	pub mod transfer {
		use core::marker::PhantomData;

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

	pub type TransferChecked = transfer_checked::TransferChecked<super::TokenProgram>;
	pub type Transfer = transfer::Transfer<super::TokenProgram>;
}

pub mod state {
	/// Mirrors the crate's own account parser, which Pina re-exports.
	pub struct TokenAccount;

	impl TokenAccount {
		pub fn from_account_view<T>(_: &T) -> Result<&'static TokenAccount, ()> {
			Ok(&TokenAccount)
		}

		pub fn amount(&self) -> u64 {
			0
		}
	}
}
