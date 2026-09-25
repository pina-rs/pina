//! The shape of `solana_account_view`'s data-borrow API: `try_borrow_mut`
//! returns a guard that derefs to the account's byte slice and releases the
//! borrow when dropped.

use core::ops::Deref;
use core::ops::DerefMut;

#[derive(Debug)]
pub enum ProgramError {
	AccountBorrowFailed,
}

pub struct AccountView {
	data: [u8; 8],
}

pub struct RefMut<'a, T: ?Sized> {
	value: &'a mut T,
}

impl<T: ?Sized> Deref for RefMut<'_, T> {
	type Target = T;

	fn deref(&self) -> &T {
		self.value
	}
}

impl<T: ?Sized> DerefMut for RefMut<'_, T> {
	fn deref_mut(&mut self) -> &mut T {
		self.value
	}
}

impl<T: ?Sized> Drop for RefMut<'_, T> {
	fn drop(&mut self) {}
}

impl AccountView {
	pub fn try_borrow_mut(&mut self) -> Result<RefMut<'_, [u8]>, ProgramError> {
		Ok(RefMut {
			value: &mut self.data,
		})
	}

	pub fn close(&mut self) -> Result<(), ProgramError> {
		Ok(())
	}
}
