#![allow(dead_code)]

pub struct Address;
pub struct Signer;

pub struct Instruction;

impl Instruction {
	pub fn invoke(&self) -> Result<(), ()> {
		Ok(())
	}

	pub fn invoke_signed(&self, _signers: &[Signer]) -> Result<(), ()> {
		Ok(())
	}

	pub fn invoke_with_program(&self, _program: &Address) -> Result<(), ()> {
		Ok(())
	}

	pub fn invoke_signed_with_program(
		&self,
		_signers: &[Signer],
		_program: &Address,
	) -> Result<(), ()> {
		Ok(())
	}

	pub fn invoke_with_unverified_program(&self, _program: &Address) -> Result<(), ()> {
		Ok(())
	}

	pub fn invoke_signed_with_unverified_program(
		&self,
		_signers: &[Signer],
		_program: &Address,
	) -> Result<(), ()> {
		Ok(())
	}
}
