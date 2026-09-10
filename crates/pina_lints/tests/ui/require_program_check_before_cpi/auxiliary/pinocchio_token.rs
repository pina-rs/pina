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

// The parameterized variant lets fixtures model program arguments whose type is
// not the plain `Address` (for example interior-mutable wrappers) while the
// plain `Instruction` above stays concrete for the other fixtures.
pub struct InstructionFor<Program> {
	_marker: std::marker::PhantomData<Program>,
}

impl<Program> InstructionFor<Program> {
	pub fn for_program() -> Self {
		Self {
			_marker: std::marker::PhantomData,
		}
	}

	pub fn invoke_with_unverified_program(&self, _program: &Program) -> Result<(), ()> {
		Ok(())
	}

	pub fn invoke_signed_with_unverified_program(
		&self,
		_signers: &[Signer],
		_program: &Program,
	) -> Result<(), ()> {
		Ok(())
	}
}

pub struct InstructionMut;

impl InstructionMut {
	pub fn invoke_with_unverified_program(&mut self, _program: &Address) -> Result<(), ()> {
		Ok(())
	}

	pub fn invoke_signed_with_unverified_program(
		&mut self,
		_signers: &[Signer],
		_program: &Address,
	) -> Result<(), ()> {
		Ok(())
	}
}

pub struct InstructionLegacy;

impl InstructionLegacy {
	pub fn invoke_signed_with_unverified_program(&self, _signers: &[Signer]) -> Result<(), ()> {
		Ok(())
	}
}
