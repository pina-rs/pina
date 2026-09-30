//! Instruction wire pins for the zero-field event instructions.
//!
//! No instruction in this program opts into migrations, so each one is its
//! bare discriminator with no version envelope. Dispatch reads only that
//! discriminator, and the generated zero-field parser stays exact: a request
//! still carrying the version byte an older ABI wrote is rejected rather than
//! read as a different layout.

// The program is a cdylib only (see Cargo.toml), so its real types come
// in through a source include rather than an rlib dependency.
#[path = "../src/lib.rs"]
mod program;
use pina::ProgramError;
use pina::parse_instruction;
use program::EventsInstruction;
use program::ID;
use program::InitializeInstruction;

/// Every declared event instruction, for exhaustive dispatch probes.
fn all_instructions() -> [EventsInstruction; 3] {
	[
		EventsInstruction::Initialize,
		EventsInstruction::TestEvent,
		EventsInstruction::TestEventCpi,
	]
}

/// A bare discriminator dispatches to its own variant.
#[test]
fn dispatch_accepts_the_bare_discriminator() {
	for instruction in all_instructions() {
		let data = [instruction as u8];
		let parsed = parse_instruction::<EventsInstruction>(&ID, &ID, &data)
			.unwrap_or_else(|error| panic!("bare discriminator must parse: {error:?}"));
		assert!(
			parsed == instruction,
			"{data:?} must dispatch to its own variant"
		);
	}
}

/// Unknown discriminants and empty data are rejected as invalid instruction
/// data.
#[test]
fn dispatch_rejects_unknown_discriminants_and_empty_data() {
	assert_eq!(
		parse_instruction::<EventsInstruction>(&ID, &ID, &[8]).err(),
		Some(ProgramError::InvalidInstructionData)
	);
	assert_eq!(
		parse_instruction::<EventsInstruction>(&ID, &ID, &[]).err(),
		Some(ProgramError::InvalidInstructionData)
	);
}

/// The generated zero-field parser is exact: the bare discriminator in, and a
/// leftover version byte, any other trailing byte, or a missing discriminator
/// out.
#[test]
fn the_zero_field_parser_remains_exact() {
	assert_eq!(InitializeInstruction::SIZE, 1);
	InitializeInstruction::try_from_bytes(&[EventsInstruction::Initialize as u8])
		.unwrap_or_else(|error| panic!("bare discriminator parses: {error:?}"));

	for trailing in [0_u8, 1, 255] {
		assert_eq!(
			InitializeInstruction::try_from_bytes(&[EventsInstruction::Initialize as u8, trailing])
				.err(),
			Some(ProgramError::InvalidInstructionData),
			"a trailing {trailing} byte must be rejected",
		);
	}
	assert_eq!(
		InitializeInstruction::try_from_bytes(&[]).err(),
		Some(ProgramError::InvalidInstructionData),
	);
}
