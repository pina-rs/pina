//! Anchor `errors` parity example ported to pina.
//!
//! This adaptation keeps the key behavior: deterministic custom error codes
//! and explicit guard helpers that return those errors.

#![allow(missing_docs)]
#![allow(clippy::inline_always)]
#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MyError {
	/// A plain custom error.
	Hello = 6000,
	// Deliberately undocumented: this variant exists to pin the Anchor-parity
	// behavior of an error whose IDL message is empty. Documenting it would
	// delete the only fixture covering that case.
	HelloNoMsg = 6123,
	/// A distinct code with its own message.
	HelloNext = 6124,
	/// A custom error carrying caller-supplied context.
	HelloCustom = 6125,
	/// Two values were expected to differ and did not.
	ValueMismatch = 6126,
	/// Two values were expected to be equal and were not.
	ValueMatch = 6127,
	/// The compared value was not less than the bound.
	ValueLess = 6128,
	/// The compared value was not less than or equal to the bound.
	ValueLessOrEqual = 6129,
}

#[discriminator]
pub enum ErrorsInstruction {
	Hello = 0,
	HelloNoMsg = 1,
	HelloNext = 2,
	RequireEq = 3,
	RequireNeq = 4,
	RequireGt = 5,
	RequireGte = 6,
}

#[instruction(discriminator = ErrorsInstruction::Hello)]
pub struct HelloInstruction {}

#[instruction(discriminator = ErrorsInstruction::HelloNoMsg)]
pub struct HelloNoMsgInstruction {}

#[instruction(discriminator = ErrorsInstruction::HelloNext)]
pub struct HelloNextInstruction {}

#[instruction(discriminator = ErrorsInstruction::RequireEq)]
pub struct RequireEqInstruction {}

#[instruction(discriminator = ErrorsInstruction::RequireNeq)]
pub struct RequireNeqInstruction {}

#[instruction(discriminator = ErrorsInstruction::RequireGt)]
pub struct RequireGtInstruction {}

#[instruction(discriminator = ErrorsInstruction::RequireGte)]
pub struct RequireGteInstruction {}

#[allow(dead_code)]
fn require_eq(left: u64, right: u64, error: MyError) -> ProgramResult {
	if left != right {
		return Err(error.into());
	}

	Ok(())
}

#[allow(dead_code)]
fn require_neq(left: u64, right: u64, error: MyError) -> ProgramResult {
	if left == right {
		return Err(error.into());
	}

	Ok(())
}

#[allow(dead_code)]
fn require_gt(left: u64, right: u64, error: MyError) -> ProgramResult {
	if left <= right {
		return Err(error.into());
	}

	Ok(())
}

#[allow(dead_code)]
fn require_gte(left: u64, right: u64, error: MyError) -> ProgramResult {
	if left < right {
		return Err(error.into());
	}

	Ok(())
}

#[allow(dead_code)]
fn process_instruction_variant(instruction: ErrorsInstruction) -> ProgramResult {
	match instruction {
		ErrorsInstruction::Hello => Err(MyError::Hello.into()),
		ErrorsInstruction::HelloNoMsg => Err(MyError::HelloNoMsg.into()),
		ErrorsInstruction::HelloNext => Err(MyError::HelloNext.into()),
		ErrorsInstruction::RequireEq => require_eq(5_241, 124_124_124, MyError::ValueMismatch),
		ErrorsInstruction::RequireNeq => require_neq(500, 500, MyError::ValueMatch),
		ErrorsInstruction::RequireGt => require_gt(5, 10, MyError::ValueLessOrEqual),
		ErrorsInstruction::RequireGte => require_gte(5, 10, MyError::ValueLess),
	}
}

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		_accounts: &mut [AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: ErrorsInstruction = parse_instruction(program_id, &ID, data)?;
		process_instruction_variant(instruction)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn assert_custom_code(error: MyError, expected: u32) {
		let as_program_error: ProgramError = error.into();
		assert!(matches!(as_program_error, ProgramError::Custom(code) if code == expected));
	}

	#[test]
	fn error_codes_match_anchor_expectations() {
		assert_custom_code(MyError::Hello, 6000);
		assert_custom_code(MyError::HelloNoMsg, 6123);
		assert_custom_code(MyError::HelloNext, 6124);
		assert_custom_code(MyError::HelloCustom, 6125);
		assert_custom_code(MyError::ValueMismatch, 6126);
		assert_custom_code(MyError::ValueMatch, 6127);
		assert_custom_code(MyError::ValueLess, 6128);
		assert_custom_code(MyError::ValueLessOrEqual, 6129);
	}

	#[test]
	fn hello_variants_return_expected_errors() {
		assert!(matches!(
			process_instruction_variant(ErrorsInstruction::Hello),
			Err(ProgramError::Custom(code)) if code == MyError::Hello as u32
		));
		assert!(matches!(
			process_instruction_variant(ErrorsInstruction::HelloNoMsg),
			Err(ProgramError::Custom(code)) if code == MyError::HelloNoMsg as u32
		));
		assert!(matches!(
			process_instruction_variant(ErrorsInstruction::HelloNext),
			Err(ProgramError::Custom(code)) if code == MyError::HelloNext as u32
		));
	}

	#[test]
	fn require_helpers_return_expected_errors() {
		assert!(matches!(
			process_instruction_variant(ErrorsInstruction::RequireEq),
			Err(ProgramError::Custom(code)) if code == MyError::ValueMismatch as u32
		));
		assert!(matches!(
			process_instruction_variant(ErrorsInstruction::RequireNeq),
			Err(ProgramError::Custom(code)) if code == MyError::ValueMatch as u32
		));
		assert!(matches!(
			process_instruction_variant(ErrorsInstruction::RequireGt),
			Err(ProgramError::Custom(code)) if code == MyError::ValueLessOrEqual as u32
		));
		assert!(matches!(
			process_instruction_variant(ErrorsInstruction::RequireGte),
			Err(ProgramError::Custom(code)) if code == MyError::ValueLess as u32
		));
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [6u8; 32].into();
		let data = [ErrorsInstruction::Hello as u8];
		let result = parse_instruction::<ErrorsInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}
}
