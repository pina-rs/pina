#[derive(Clone, Copy)]
pub struct AccountView;

pub struct RefMut;

impl AccountView {
	pub fn address(&self) -> &[u8; 32] {
		&[0; 32]
	}

	pub fn is_writable(&self) -> bool {
		true
	}

	pub fn try_borrow_mut(&mut self) -> Result<RefMut, ()> {
		Ok(RefMut)
	}

	pub fn set_lamports(&mut self, _lamports: u64) {}

	pub fn close(&mut self) -> Result<(), ()> {
		Ok(())
	}

	pub fn assert_writable(self) -> Result<Self, ()> {
		Ok(self)
	}
}
