#![allow(dead_code)]

pub mod account {
	pub struct AccountView;
}

pub mod sysvars {
	pub mod clock {
		use crate::account::AccountView;

		pub struct Clock {
			pub slot: u64,
		}

		impl Clock {
			pub fn from_account_view(_account: &AccountView) -> Result<Self, ()> {
				Ok(Self { slot: 0 })
			}

			pub fn from_bytes(_data: &[u8]) -> Result<Self, ()> {
				Ok(Self { slot: 0 })
			}

			pub unsafe fn from_bytes_unchecked(_data: &[u8]) -> Self {
				Self { slot: 0 }
			}

			pub fn advance(&mut self, by: u64) {
				self.slot += by;
			}
		}

		pub struct ClockWrapper;

		impl ClockWrapper {
			pub fn from_bytes(_data: &[u8]) -> Self {
				Self
			}
		}
	}

	pub mod instructions {
		use crate::account::AccountView;

		pub struct Instructions;

		impl TryFrom<&AccountView> for Instructions {
			type Error = ();

			fn try_from(_account: &AccountView) -> Result<Self, Self::Error> {
				Ok(Self)
			}
		}

		impl Instructions {
			pub unsafe fn new_unchecked(_data: &[u8]) -> Self {
				Self
			}

			pub fn load_current_index(&self) -> usize {
				0
			}
		}
	}

	pub mod rent {
		use crate::account::AccountView;

		pub struct Rent;

		impl Rent {
			pub fn from_account_view(_account: &AccountView) -> Result<Self, ()> {
				Ok(Self)
			}

			pub fn from_bytes(_data: &[u8]) -> Result<Self, ()> {
				Ok(Self)
			}

			pub unsafe fn from_bytes_unchecked(_data: &[u8]) -> Self {
				Self
			}

			pub fn minimum_balance(&self, _data_len: usize) -> u64 {
				0
			}
		}
	}

	pub mod slot_hashes {
		use crate::account::AccountView;

		pub struct SlotHashes;

		impl SlotHashes {
			pub fn new(_data: &[u8]) -> Result<Self, ()> {
				Ok(Self)
			}

			pub unsafe fn new_unchecked(_data: &[u8]) -> Self {
				Self
			}

			pub fn from_account_view(_account: &AccountView) -> Result<Self, ()> {
				Ok(Self)
			}

			pub fn len(&self) -> usize {
				0
			}
		}
	}
}
