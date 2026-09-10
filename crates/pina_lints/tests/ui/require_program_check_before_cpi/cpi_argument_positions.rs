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
use pinocchio_token::InstructionLegacy;

static TOKEN_ID: Address = Address;

type ProgramAccount = PinaProgramAccount<Address>;

fn process_proven_signed_argument_index(
	instruction: &Instruction,
	program: &ProgramAccount,
) -> Result<(), ()> {
	program.assert_program(&TOKEN_ID)?;
	instruction.invoke_signed_with_unverified_program(&[], program.address())
}

fn process_unproven_signed_argument_index(
	instruction: &Instruction,
	program: &ProgramAccount,
) -> Result<(), ()> {
	instruction.invoke_signed_with_unverified_program(&[], program.address())
	//~^ ERROR: `.invoke_signed_with_unverified_program()` called without a preceding program address verification
}

fn process_missing_program_argument(
	legacy: &InstructionLegacy,
	program: &ProgramAccount,
) -> Result<(), ()> {
	program.assert_program(&TOKEN_ID)?;
	// The expected program position holds no argument, so no proof can apply.
	legacy.invoke_signed_with_unverified_program(&[])
	//~^ ERROR: `.invoke_signed_with_unverified_program()` called without a preceding program address verification
}

fn main() {}

// compile-fail
