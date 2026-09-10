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
use pinocchio_token::InstructionMut;

static TOKEN_ID: Address = Address;

type ProgramAccount = PinaProgramAccount<Address>;

fn process_mutating_receiver_invalidates(instruction: &Instruction) -> Result<(), ()> {
	let mut program = PinaProgramAccount::new(&TOKEN_ID);
	program.assert_program(&&TOKEN_ID)?;
	program.refresh();
	instruction.invoke_with_unverified_program(program.address())?;
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
	Ok(())
}

fn process_shared_receiver_preserves_proof(instruction: &Instruction) -> Result<(), ()> {
	let mut program = PinaProgramAccount::new(&TOKEN_ID);
	program.assert_program(&&TOKEN_ID)?;
	let _ = program.address();
	instruction.invoke_with_unverified_program(program.address())
}

fn process_mutating_unverified_invocation(
	transfer: &mut InstructionMut,
	attacker: &Address,
) -> Result<(), ()> {
	transfer.invoke_with_unverified_program(attacker)?;
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
	Ok(())
}

fn process_mutating_receiver_invalidated_after_proof(
	transfer: &mut InstructionMut,
	checked: &ProgramAccount,
) -> Result<(), ()> {
	let mut program = checked;
	program.assert_program(&TOKEN_ID)?;
	transfer.invoke_with_unverified_program(program.address())
}

fn main() {}

// compile-fail
