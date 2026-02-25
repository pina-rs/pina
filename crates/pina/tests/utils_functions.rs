use pina::ProgramError;
use pina::parse_instruction;

// Use the pina discriminator macro to create a proper discriminator enum.
#[pina::discriminator(crate = ::pina)]
#[derive(Debug, PartialEq)]
pub enum TestInstruction {
	Initialize = 0,
	Update = 1,
	Close = 2,
}

// Use the system program ID for tests (valid base58).
const PROGRAM_ID: pina::Address = pina::address!("11111111111111111111111111111111");

#[test]
fn parse_instruction_valid_discriminator() {
	let data = [0u8]; // Initialize = 0
	let result: TestInstruction = parse_instruction(&PROGRAM_ID, &PROGRAM_ID, &data)
		.unwrap_or_else(|e| panic!("expected valid parse: {e:?}"));
	assert_eq!(result, TestInstruction::Initialize);
}

#[test]
fn parse_instruction_all_variants() {
	for (byte, expected) in [
		(0u8, TestInstruction::Initialize),
		(1u8, TestInstruction::Update),
		(2u8, TestInstruction::Close),
	] {
		let data = [byte];
		let result: TestInstruction = parse_instruction(&PROGRAM_ID, &PROGRAM_ID, &data)
			.unwrap_or_else(|e| panic!("expected valid parse for variant {byte}: {e:?}"));
		assert_eq!(result, expected);
	}
}

#[test]
fn parse_instruction_wrong_program_id() {
	let data = [0u8];
	// Use a known-different valid address (Token program).
	let wrong_id = pina::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
	let err = parse_instruction::<TestInstruction>(&PROGRAM_ID, &wrong_id, &data).unwrap_err();
	assert_eq!(err, ProgramError::IncorrectProgramId);
}

#[test]
fn parse_instruction_empty_data() {
	let data: &[u8] = &[];
	let err = parse_instruction::<TestInstruction>(&PROGRAM_ID, &PROGRAM_ID, data).unwrap_err();
	assert_eq!(err, ProgramError::InvalidInstructionData);
}

/// Invalid discriminator byte (e.g. 99) should be remapped from
/// ProgramError::Custom (InvalidDiscriminator) to InvalidInstructionData.
#[test]
fn parse_instruction_invalid_discriminator_remapped() {
	let data = [99u8];
	let err = parse_instruction::<TestInstruction>(&PROGRAM_ID, &PROGRAM_ID, &data).unwrap_err();
	// The key behavior: Custom errors from invalid discriminators are
	// remapped to InvalidInstructionData for a generic "bad data" error.
	assert_eq!(err, ProgramError::InvalidInstructionData);
}

#[test]
fn parse_instruction_extra_data_is_ok() {
	// Extra trailing bytes after the discriminator should be ignored.
	let data = [1u8, 0xFF, 0xFF, 0xFF];
	let result: TestInstruction = parse_instruction(&PROGRAM_ID, &PROGRAM_ID, &data)
		.unwrap_or_else(|e| panic!("expected valid parse: {e:?}"));
	assert_eq!(result, TestInstruction::Update);
}

// ---- assert function tests ----

#[test]
fn assert_true_returns_ok() {
	let result = pina::assert(true, ProgramError::InvalidArgument, "should not fail");
	assert!(result.is_ok());
}

#[test]
fn assert_false_returns_err() {
	let result = pina::assert(false, ProgramError::InvalidArgument, "expected failure");
	assert_eq!(result.unwrap_err(), ProgramError::InvalidArgument);
}

#[test]
fn assert_custom_error_type() {
	let result = pina::assert(
		false,
		pina::PinaProgramError::DataTooShort,
		"data too short",
	);
	let err = result.unwrap_err();
	match err {
		ProgramError::Custom(code) => assert_eq!(code, 0xFFFF_FFFA),
		other => panic!("expected Custom error, got: {other:?}"),
	}
}
