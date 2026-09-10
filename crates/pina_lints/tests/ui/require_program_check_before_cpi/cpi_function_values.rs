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

use pinocchio_token::Address;
use pinocchio_token::Instruction;
use pinocchio_token::Signer;

type InvokeSignedFn = fn(&Instruction, &[Signer], &Address) -> Result<(), ()>;

fn process_invoke_function_value() {
	let invoke = Instruction::invoke_with_unverified_program;
	//~^ ERROR: `invoke_with_unverified_program` cannot be used as a function value
	let _ = invoke;
}

fn process_invoke_signed_function_value() {
	let invoke = Instruction::invoke_signed_with_unverified_program;
	//~^ ERROR: `invoke_signed_with_unverified_program` cannot be used as a function value
	let _ = invoke;
}

fn process_invoke_signed_function_value_cast() {
	let invoke = Instruction::invoke_signed_with_unverified_program as InvokeSignedFn;
	//~^ ERROR: `invoke_signed_with_unverified_program` cannot be used as a function value
	let _ = invoke;
}

fn process_option_wrapped_function_value() {
	let invoke = Some(Instruction::invoke_signed_with_unverified_program);
	//~^ ERROR: `invoke_signed_with_unverified_program` cannot be used as a function value
	let _ = invoke;
}

fn process_closure_wrapped_function_value() {
	let invoke = || Instruction::invoke_signed_with_unverified_program;
	//~^ ERROR: `invoke_signed_with_unverified_program` cannot be used as a function value
	let _ = invoke;
}

fn process_tuple_wrapped_function_value() {
	let (invoke,) = (Instruction::invoke_signed_with_unverified_program,);
	//~^ ERROR: `invoke_signed_with_unverified_program` cannot be used as a function value
	let _ = invoke;
}

fn process_verified_function_value_is_allowed() {
	let invoke = Instruction::invoke_signed;
	// No lint: `invoke_signed` is not an unverified dynamic CPI method.
	let _ = invoke;
}

fn main() {}

// compile-fail
