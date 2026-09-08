#![allow(dead_code, unused_assignments)]
// normalize-stderr-test: "\n$" -> ""
// aux-build: pinocchio_token.rs

extern crate pinocchio_token;

use pinocchio_token::Address;
use pinocchio_token::Instruction;

static TOKEN_PROGRAM_ID: Address = Address;

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

	fn assert_program(&self, _expected: &Address) -> Result<(), ()> {
		Ok(())
	}
}

struct UnrelatedInstruction;

impl UnrelatedInstruction {
	fn invoke_with_unverified_program(&self, _program: &Address) -> Result<(), ()> {
		Ok(())
	}
}

type InvokeFn = fn(&Instruction, &Address) -> Result<(), ()>;

const UNVERIFIED_INVOKE: InvokeFn = Instruction::invoke_with_unverified_program;
//~^ ERROR: `invoke_with_unverified_program` cannot be used as a function value

fn unrelated_invoke(_instruction: &Instruction, _program: &Address) -> Result<(), ()> {
	Ok(())
}

fn static_program_invocation_does_not_need_account_validation() -> Result<(), ()> {
	Instruction.invoke()?;
	Instruction.invoke_signed(&[])?;
	Instruction.invoke_with_program(&TOKEN_PROGRAM_ID)?;
	let target = &TOKEN_PROGRAM_ID;
	Instruction.invoke_with_program(target)
}

fn verified_program_methods_self_validate(token_program: &ProgramAccount) -> Result<(), ()> {
	Instruction.invoke_with_program(token_program.address())?;
	Instruction.invoke_signed_with_program(&[], token_program.address())?;
	UnrelatedInstruction.invoke_with_unverified_program(token_program.address())
}

fn missing_dynamic_program_check(token_program: &ProgramAccount) -> Result<(), ()> {
	Instruction.invoke_with_unverified_program(token_program.address())?;
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification

	Instruction.invoke_signed_with_unverified_program(&[], token_program.address())
	//~^ ERROR: `.invoke_signed_with_unverified_program()` called without a preceding program address verification
}

fn missing_dynamic_program_check_ufcs(token_program: &ProgramAccount) -> Result<(), ()> {
	Instruction::invoke_with_unverified_program(&Instruction, token_program.address())?;
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification

	Instruction::invoke_signed_with_unverified_program(&Instruction, &[], token_program.address())
	//~^ ERROR: `.invoke_signed_with_unverified_program()` called without a preceding program address verification
}

fn missing_dynamic_program_check_function_item(token_program: &ProgramAccount) -> Result<(), ()> {
	let invoke = Instruction::invoke_with_unverified_program;
	//~^ ERROR: `invoke_with_unverified_program` cannot be used as a function value
	invoke(&Instruction, token_program.address())
}

fn missing_dynamic_program_check_cast(token_program: &ProgramAccount) -> Result<(), ()> {
	let invoke = Instruction::invoke_with_unverified_program as InvokeFn;
	//~^ ERROR: `invoke_with_unverified_program` cannot be used as a function value
	invoke(&Instruction, token_program.address())?;

	(Instruction::invoke_with_unverified_program as InvokeFn)(&Instruction, token_program.address())
	//~^ ERROR: `invoke_with_unverified_program` cannot be used as a function value
}

fn unverified_function_value_escapes_are_rejected(condition: bool) -> Result<(), ()> {
	let (tuple_invoke,) = (Instruction::invoke_with_unverified_program,);
	//~^ ERROR: `invoke_with_unverified_program` cannot be used as a function value
	let Some(option_invoke) = Some(Instruction::invoke_with_unverified_program) else {
		//~^ ERROR: `invoke_with_unverified_program` cannot be used as a function value
		return Ok(());
	};
	let immediate = (|| Instruction::invoke_with_unverified_program)();
	//~^ ERROR: `invoke_with_unverified_program` cannot be used as a function value
	let captured_item = Instruction::invoke_with_unverified_program;
	//~^ ERROR: `invoke_with_unverified_program` cannot be used as a function value
	let capture = move || captured_item;

	let mut short_circuit = unrelated_invoke as InvokeFn;
	let _ = condition && {
		short_circuit = Instruction::invoke_with_unverified_program;
		//~^ ERROR: `invoke_with_unverified_program` cannot be used as a function value
		true
	};

	let _ = (
		tuple_invoke,
		option_invoke,
		immediate,
		capture,
		short_circuit,
	);
	Ok(())
}

fn function_value_policy_is_independent_of_reachability(condition: bool) -> Result<(), ()> {
	let _invoke: InvokeFn = if condition {
		Instruction::invoke_with_unverified_program
		//~^ ERROR: `invoke_with_unverified_program` cannot be used as a function value
	} else {
		return Ok(());
	};
	Ok(())
}

fn checked_dynamic_program(token_program: &ProgramAccount, expected: &Address) -> Result<(), ()> {
	token_program.assert_program(expected)?;
	Instruction.invoke_with_unverified_program(token_program.address())?;
	Instruction.invoke_signed_with_unverified_program(&[], token_program.address())?;
	Instruction::invoke_with_unverified_program(&Instruction, token_program.address())
}

fn unrelated_program_check_does_not_authorize_dynamic_target(
	token_program: &ProgramAccount,
	system_program: &ProgramAccount,
	expected: &Address,
) -> Result<(), ()> {
	system_program.assert_address(expected)?;
	Instruction.invoke_with_unverified_program(token_program.address())
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
}

fn same_terminal_field_name_does_not_alias(
	first: Programs<'_>,
	second: Programs<'_>,
	expected: &Address,
) -> Result<(), ()> {
	first.token_program.assert_address(expected)?;
	Instruction.invoke_with_unverified_program(second.token_program.address())
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
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
		Instruction.invoke_with_unverified_program(token_program.address())?;
		//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
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
	Instruction.invoke_with_unverified_program(token_program.address())
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
}

fn conditional_check_does_not_dominate(
	token_program: &ProgramAccount,
	expected: &Address,
	condition: bool,
) -> Result<(), ()> {
	if condition {
		token_program.assert_address(expected)?;
	}

	Instruction.invoke_with_unverified_program(token_program.address())
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
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

	Instruction.invoke_with_unverified_program(token_program.address())
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
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

	Instruction.invoke_with_unverified_program(token_program.address())
}

fn diverging_paths_do_not_erase_program_proofs(
	token_program: &ProgramAccount,
	expected: &Address,
	condition: bool,
) -> Result<(), ()> {
	match condition {
		true => return Ok(()),
		false => token_program.assert_program(expected)?,
	}

	Instruction.invoke_with_unverified_program(token_program.address())
}

fn branch_local_check_dominates_branch_cpi(
	token_program: &ProgramAccount,
	expected: &Address,
	condition: bool,
) -> Result<(), ()> {
	if condition {
		token_program.assert_address(expected)?;
		Instruction.invoke_signed_with_unverified_program(&[], token_program.address())?;
	}

	Ok(())
}

fn validated_address_alias_passes(
	token_program: &ProgramAccount,
	expected: &Address,
) -> Result<(), ()> {
	token_program.assert_address(expected)?;
	let token_program = token_program.address();
	Instruction.invoke_with_unverified_program(token_program)
}

fn main() {}

// compile-fail
