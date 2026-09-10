#![allow(dead_code)]

pub struct ProgramAccount<Address> {
	address: Address,
}

impl<Address> ProgramAccount<Address> {
	pub fn new(address: Address) -> Self {
		Self { address }
	}

	pub fn address(&self) -> &Address {
		&self.address
	}

	pub fn refresh(&mut self) {}
}

pub trait AccountInfoValidation<Address>: Sized {
	fn assert_address(self, expected: &Address) -> Result<Self, ()>;
	fn assert_addresses(self, expected: &[Address]) -> Result<Self, ()>;
	fn assert_program(self, expected: &Address) -> Result<Self, ()>;
}

impl<'a, Address> AccountInfoValidation<Address> for &'a ProgramAccount<Address> {
	fn assert_address(self, _expected: &Address) -> Result<Self, ()> {
		Ok(self)
	}

	fn assert_addresses(self, _expected: &[Address]) -> Result<Self, ()> {
		Ok(self)
	}

	fn assert_program(self, _expected: &Address) -> Result<Self, ()> {
		Ok(self)
	}
}
