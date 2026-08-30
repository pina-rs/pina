#![allow(dead_code)]

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
}
