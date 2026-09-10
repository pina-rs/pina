// normalize-stderr-test: "\n$" -> ""
// aux-build: pinocchio_token.rs
// aux-build: pina.rs

#![feature(coroutines, coroutine_trait, stmt_expr_attributes, explicit_tail_calls)]
#![allow(
	dead_code,
	unused_assignments,
	unused_variables,
	unused_mut,
	unreachable_code,
	unused_parens,
	incomplete_features
)]

extern crate pina;
extern crate pinocchio_token;

use pina::ProgramAccount as PinaProgramAccount;
use pinocchio_token::Address;
use pinocchio_token::Instruction;

static TOKEN_ID: Address = Address;

type ProgramAccount = PinaProgramAccount<Address>;

fn process_generator_invocation(instruction: &Instruction, program: &ProgramAccount) {
	let generator = #[coroutine]
	|| {
		yield instruction.invoke_with_unverified_program(program.address());
	};
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
	let _ = generator;
}

fn process_tail_call_invocation(
	instruction: &Instruction,
	program: &ProgramAccount,
	rounds: u32,
) -> u32 {
	if rounds == 0 {
		0
	} else {
		let _ = instruction.invoke_with_unverified_program(program.address());
		//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
		become process_tail_call_invocation(instruction, program, rounds - 1)
	}
}

fn main() {}

// compile-fail
