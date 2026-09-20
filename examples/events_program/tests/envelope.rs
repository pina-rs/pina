//! Instruction-envelope wire pins for the zero-field event instructions.
//!
//! Every instruction in this program carries `[discriminator, version]` and
//! none carries a payload, so instruction dispatch is the only place the
//! migration envelope can be enforced. These pins hold sweep finding F-2
//! closed: a zero-field instruction accepts only the current version byte and
//! rejects a missing or unknown version byte at parse time, exactly like an
//! instruction with a payload.

use events_program::EventsInstruction;
use events_program::ID;
use events_program::InitializeInstruction;
use pina::IntoDiscriminator as _;
use pina::PinaProgramError;
use pina::ProgramError;
use pina::parse_instruction;

/// Every declared event instruction, for exhaustive envelope probes.
fn all_instructions() -> [EventsInstruction; 3] {
	[
		EventsInstruction::Initialize,
		EventsInstruction::TestEvent,
		EventsInstruction::TestEventCpi,
	]
}

/// The invalid-migration-version error the payload path returns for a future
/// version, as a `ProgramError`.
fn invalid_migration_version() -> ProgramError {
	ProgramError::Custom(PinaProgramError::InvalidMigrationVersion as u32)
}

/// The current version byte is the only version a zero-field instruction
/// accepts: `[disc, 0]` dispatches to its variant.
#[test]
fn dispatch_accepts_the_current_version_byte() {
	for instruction in all_instructions() {
		let data = [instruction as u8, 0];
		let parsed = parse_instruction::<EventsInstruction>(&ID, &ID, &data)
			.unwrap_or_else(|error| panic!("current envelope must parse: {error:?}"));
		assert!(
			parsed == instruction,
			"envelope {data:?} must dispatch to its own variant"
		);
	}
}

/// Any other byte in the version slot fails closed. Before the envelope gate a
/// zero-field instruction parsed with any version byte, so it could never be
/// version-gated after the fact. The dispatch surface reports one uniform
/// invalid-data failure (the remap `parse_instruction` applies to every
/// discriminator-parse error).
#[test]
fn dispatch_rejects_unknown_version_bytes() {
	for instruction in all_instructions() {
		for version in [1_u8, 7, 42, 255] {
			let data = [instruction as u8, version];
			assert_eq!(
				parse_instruction::<EventsInstruction>(&ID, &ID, &data).err(),
				Some(ProgramError::InvalidInstructionData),
				"instruction {} must reject version byte {version}",
				instruction as u8
			);
		}
	}
}

/// The typed error survives on the parse itself: an unknown or future version
/// byte is the same `InvalidMigrationVersion` error the payload path returns.
#[test]
fn the_parser_names_the_unknown_version_typed_error() {
	for instruction in all_instructions() {
		let data = [instruction as u8, 7];
		assert_eq!(
			EventsInstruction::discriminator_from_bytes(&data).err(),
			Some(invalid_migration_version()),
			"instruction {} must reject version byte 7 with its typed error",
			instruction as u8
		);
	}
}

/// The exact width includes the version byte: a bare discriminator is a
/// truncated envelope and fails as invalid instruction data.
#[test]
fn dispatch_rejects_a_missing_version_byte() {
	for instruction in all_instructions() {
		let data = [instruction as u8];
		assert_eq!(
			parse_instruction::<EventsInstruction>(&ID, &ID, &data).err(),
			Some(ProgramError::InvalidInstructionData),
			"instruction {} must reject the bare discriminator",
			instruction as u8
		);
		assert_eq!(
			EventsInstruction::discriminator_from_bytes(&data).err(),
			Some(ProgramError::InvalidInstructionData)
		);
	}
}

/// Unknown discriminants keep their existing rejection, with or without a
/// version byte.
#[test]
fn dispatch_rejects_unknown_discriminants_unchanged() {
	assert_eq!(
		parse_instruction::<EventsInstruction>(&ID, &ID, &[8]).err(),
		Some(ProgramError::InvalidInstructionData)
	);
	assert_eq!(
		parse_instruction::<EventsInstruction>(&ID, &ID, &[8, 0]).err(),
		Some(ProgramError::InvalidInstructionData)
	);
}

/// The generated zero-field parser itself stays exact: current envelope in,
/// and wrong version, missing version, and trailing bytes out.
#[test]
fn the_zero_field_parser_remains_exact() {
	let current = [EventsInstruction::Initialize as u8, 0];
	InitializeInstruction::try_from_bytes(&current)
		.unwrap_or_else(|error| panic!("current envelope parses: {error:?}"));

	assert_eq!(
		InitializeInstruction::try_from_bytes(&[EventsInstruction::Initialize as u8, 1]).err(),
		Some(invalid_migration_version()),
	);
	assert_eq!(
		InitializeInstruction::try_from_bytes(&[EventsInstruction::Initialize as u8]).err(),
		Some(ProgramError::InvalidInstructionData),
	);
	assert_eq!(
		InitializeInstruction::try_from_bytes(&[EventsInstruction::Initialize as u8, 0, 0]).err(),
		Some(ProgramError::InvalidInstructionData),
	);
}
