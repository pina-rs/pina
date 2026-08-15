//! Fuzz harness for `parse_instruction`.
//!
//! Feeds arbitrary byte slices to the instruction parsing path,
//! exercising program-ID validation and discriminator decoding
//! for real instruction enums from the workspace example programs.

#![allow(clippy::all)]

use counter_program::CounterInstruction;
use libfuzzer_sys::fuzz_target;
use pina::Address;
use pina::parse_instruction;
use role_registry_program::RegistryInstruction;

/// Fuzz `parse_instruction::<CounterInstruction>` — a `u8` discriminator
/// enum with variants `Initialize = 0` and `Increment = 1`.
///
/// `parse_instruction` first checks that `program_id == api_id`, then
/// decodes the discriminator from the data slice. We supply matching
/// IDs so the discriminator path is reached, and also exercise the
/// mismatch case by flipping a byte in the program ID.
fuzz_target!(|data: &[u8]| {
	let program_id = Address::default();
	let api_id = Address::default();

	// Matching IDs — discriminator decode branch
	let _ = parse_instruction::<CounterInstruction>(&program_id, &api_id, data);

	// Mismatching IDs — should always return IncorrectProgramId
	let other_id = {
		let mut bytes = [0u8; 32];
		if !data.is_empty() {
			bytes[0] = data[0].wrapping_add(1);
		}
		Address::new_from_array(bytes)
	};
	let _ = parse_instruction::<CounterInstruction>(&program_id, &other_id, data);
});

/// Fuzz `parse_instruction::<RegistryInstruction>` — a `u8` discriminator
/// enum with 5 variants (`Initialize = 0` through `RotateAdmin = 4`).
fuzz_target!(|data: &[u8]| {
	let program_id = Address::default();
	let api_id = Address::default();

	let _ = parse_instruction::<RegistryInstruction>(&program_id, &api_id, data);

	let other_id = {
		let mut bytes = [0u8; 32];
		if !data.is_empty() {
			bytes[0] = data[0].wrapping_add(1);
		}
		Address::new_from_array(bytes)
	};
	let _ = parse_instruction::<RegistryInstruction>(&program_id, &other_id, data);
});
