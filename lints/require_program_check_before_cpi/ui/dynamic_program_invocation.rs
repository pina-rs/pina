#![allow(dead_code)]
// normalize-stderr-test: "\n$" -> ""

struct Address;
struct Signer;

struct ProgramAccount {
	address: Address,
}

struct Programs<'a> {
	token_program: &'a ProgramAccount,
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

fn same_terminal_field_name_does_not_alias(
	first: Programs<'_>,
	second: Programs<'_>,
	expected: &Address,
) -> Result<(), ()> {
	first.token_program.assert_address(expected)?;
	Instruction.invoke_with_program(second.token_program.address())
	//~^ ERROR: `.invoke_with_program()` called without a preceding program address verification
}

fn shadowed_binding_does_not_inherit_validation(
	first: &ProgramAccount,
	second: &ProgramAccount,
	expected: &Address,
) -> Result<(), ()> {
	let token_program = first;
	token_program.assert_address(expected)?;

	{
		let token_program = second;
		Instruction.invoke_with_program(token_program.address())?;
		//~^ ERROR: `.invoke_with_program()` called without a preceding program address verification
	}

	Ok(())
}

fn reassignment_invalidates_validation(
	first: &ProgramAccount,
	second: &ProgramAccount,
	expected: &Address,
) -> Result<(), ()> {
	let mut token_program = first;
	token_program.assert_address(expected)?;
	token_program = second;
	Instruction.invoke_with_program(token_program.address())
	//~^ ERROR: `.invoke_with_program()` called without a preceding program address verification
}

fn conditional_check_does_not_dominate(
	token_program: &ProgramAccount,
	expected: &Address,
	condition: bool,
) -> Result<(), ()> {
	if condition {
		token_program.assert_address(expected)?;
	}

	Instruction.invoke_with_program(token_program.address())
	//~^ ERROR: `.invoke_with_program()` called without a preceding program address verification
}

fn match_check_does_not_dominate(
	token_program: &ProgramAccount,
	expected: &Address,
	condition: bool,
) -> Result<(), ()> {
	match condition {
		true => token_program.assert_address(expected)?,
		false => (),
	}

	Instruction.invoke_with_program(token_program.address())
	//~^ ERROR: `.invoke_with_program()` called without a preceding program address verification
}

fn checks_on_every_path_dominate(
	token_program: &ProgramAccount,
	expected: &Address,
	condition: bool,
) -> Result<(), ()> {
	if condition {
		token_program.assert_address(expected)?;
	} else {
		token_program.assert_addresses(&[])?;
	}

	match condition {
		true => token_program.assert_address(expected)?,
		false => token_program.assert_addresses(&[])?,
	}

	Instruction.invoke_with_program(token_program.address())
}

fn branch_local_check_dominates_branch_cpi(
	token_program: &ProgramAccount,
	expected: &Address,
	condition: bool,
) -> Result<(), ()> {
	if condition {
		token_program.assert_address(expected)?;
		Instruction.invoke_signed_with_program(&[], token_program.address())?;
	}

	Ok(())
}

fn validated_address_alias_passes(
	token_program: &ProgramAccount,
	expected: &Address,
) -> Result<(), ()> {
	token_program.assert_address(expected)?;
	let token_program = token_program.address();
	Instruction.invoke_with_program(token_program)
}

fn main() {}
