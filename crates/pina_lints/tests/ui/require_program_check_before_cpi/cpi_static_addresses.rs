// normalize-stderr-test: "\n$" -> ""
// aux-build: pinocchio_token.rs
// aux-build: pina.rs

#![allow(
	dead_code,
	unused_assignments,
	unused_variables,
	unused_mut,
	unreachable_code,
	unused_parens
)]

extern crate pina;
extern crate pinocchio_token;

use pina::AccountInfoValidation;
use pina::ProgramAccount as PinaProgramAccount;
use pinocchio_token::Address;
use pinocchio_token::Instruction;

static STATIC_TOKEN_ID: Address = Address;
const CONST_TOKEN_ID: Address = Address;
static mut MUTABLE_TOKEN_ID: Address = Address;

type ProgramAccount = PinaProgramAccount<Address>;

fn process_const_expected_id(
	instruction: &Instruction,
	program: &ProgramAccount,
) -> Result<(), ()> {
	program.assert_program(&CONST_TOKEN_ID)?;
	instruction.invoke_with_unverified_program(program.address())
}

fn process_immutable_static_expected_id(
	instruction: &Instruction,
	program: &ProgramAccount,
) -> Result<(), ()> {
	program.assert_program(&STATIC_TOKEN_ID)?;
	instruction.invoke_with_unverified_program(program.address())
}

fn process_parenthesized_const_expected_id(
	instruction: &Instruction,
	program: &ProgramAccount,
) -> Result<(), ()> {
	program.assert_program((&CONST_TOKEN_ID))?;
	instruction.invoke_with_unverified_program(program.address())
}

fn process_mutable_static_expected_id(
	instruction: &Instruction,
	program: &ProgramAccount,
) -> Result<(), ()> {
	// A mutable static is not an immutable compile-time target.
	let pointer = &raw const MUTABLE_TOKEN_ID;
	program.assert_program(unsafe { &*pointer })?;
	instruction.invoke_with_unverified_program(program.address())
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
}

fn process_dereferenced_const_alias(
	instruction: &Instruction,
	program: &ProgramAccount,
) -> Result<(), ()> {
	let alias = &&CONST_TOKEN_ID;
	program.assert_program(*alias)?;
	instruction.invoke_with_unverified_program(program.address())
}

fn process_runtime_expected_id(
	instruction: &Instruction,
	program: &ProgramAccount,
	runtime: &Address,
) -> Result<(), ()> {
	program.assert_program(runtime)?;
	instruction.invoke_with_unverified_program(program.address())
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
}

fn process_dereferenced_runtime_alias(
	instruction: &Instruction,
	program: &ProgramAccount,
	runtime: &Address,
) -> Result<(), ()> {
	let alias = &&runtime;
	// A dereferenced alias of a runtime value is not a compile-time target.
	program.assert_program(*alias)?;
	instruction.invoke_with_unverified_program(program.address())
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
}

fn main() {}

// compile-fail
