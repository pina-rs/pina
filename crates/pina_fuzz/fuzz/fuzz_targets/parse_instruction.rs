//! Fuzz harness for `parse_instruction`.
//!
//! Feeds arbitrary byte slices to the instruction parsing path,
//! exercising program-ID validation and discriminator decoding
//! for real instruction enums from the workspace example programs.

#![allow(clippy::all)]
#![no_main]

use counter_program::CounterInstruction;
use libfuzzer_sys::fuzz_target;
use pina::Address;
use pina::ProgramError;
use pina::parse_instruction;
use role_registry_program::RegistryInstruction;

// Use fixed, distinct IDs so every input exercises both the discriminator and
// incorrect-program-ID paths, including empty input and a leading 0xff byte.
fuzz_target!(|data: &[u8]| {
	let program_id = Address::default();
	let other_id = Address::new_from_array([1u8; 32]);

	let _ = parse_instruction::<CounterInstruction>(&program_id, &program_id, data);
	let _ = parse_instruction::<RegistryInstruction>(&program_id, &program_id, data);
	assert!(matches!(
		parse_instruction::<CounterInstruction>(&program_id, &other_id, data),
		Err(ProgramError::IncorrectProgramId)
	));
	assert!(matches!(
		parse_instruction::<RegistryInstruction>(&program_id, &other_id, data),
		Err(ProgramError::IncorrectProgramId)
	));
});
