#![crate_type = "lib"]

pub mod traits {
	pub struct AccountsCursor;

	impl AccountsCursor {
		pub fn remaining_mut(&mut self) -> Result<&mut [u8], ()> {
			Ok(&mut [])
		}

		pub fn remaining_mut_distinct(&mut self) -> Result<&mut [u8], ()> {
			Ok(&mut [])
		}
	}
}
