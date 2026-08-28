#![allow(dead_code)]
// normalize-stderr-test: "\n$" -> ""

struct Address;
struct Signer;

struct ProgramAccount {
	address: Address,
}

impl ProgramAccount {
	fn address(&self) -> &Address {
		&self.address
	}

	fn assert_address(&self, _expected: &Address) -> Result<(), ()> {
		Ok(())
	}

	fn assert_addresses(&self, _expected: &[Address]) -> Result<(), ()> {
		Ok(())
	}
}

struct Instruction;

impl Instruction {
	fn invoke_with_program(&self, _program: &Address) -> Result<(), ()> {
		Ok(())
	}

	fn invoke_signed_with_program(
		&self,
		_signers: &[Signer],
		_program: &Address,
	) -> Result<(), ()> {
		Ok(())
	}
}

fn missing_dynamic_program_check(token_program: &ProgramAccount) -> Result<(), ()> {
	Instruction.invoke_with_program(token_program.address())?;
	//~^ ERROR: `.invoke_with_program()` called without a preceding program address verification

	Instruction.invoke_signed_with_program(&[], token_program.address())
	//~^ ERROR: `.invoke_signed_with_program()` called without a preceding program address verification
}

fn checked_dynamic_program(token_program: &ProgramAccount, expected: &Address) -> Result<(), ()> {
	token_program.assert_address(expected)?;
	Instruction.invoke_with_program(token_program.address())?;
	Instruction.invoke_signed_with_program(&[], token_program.address())
}

fn unrelated_program_check_does_not_authorize_dynamic_target(
	token_program: &ProgramAccount,
	system_program: &ProgramAccount,
	expected: &Address,
) -> Result<(), ()> {
	system_program.assert_address(expected)?;
	Instruction.invoke_with_program(token_program.address())
	//~^ ERROR: `.invoke_with_program()` called without a preceding program address verification
}

fn main() {}
