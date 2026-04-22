//! Property-based tests for core Pina utilities.
//!
//! Covers PDA derivation round-trips and instruction parsing safety.

#![allow(unreachable_code)]

use pina::IntoDiscriminator;
use pina::create_program_address;
use pina::parse_instruction;
use pina::try_find_program_address;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// PDA round-trip: find -> create with bump must reproduce the same address
// ---------------------------------------------------------------------------

proptest! {
	#[test]
	fn pda_find_create_roundtrip(seeds in prop::collection::vec(any::<u8>(), 1..128)) {
		let seeds_ref: Vec<&[u8]> = seeds.chunks(32).map(|c| c.as_ref()).collect();
		if let Some((pda, bump)) = try_find_program_address(&seeds_ref, &pina::system::ID) {
			let bump_seed = [bump];
			let mut seeds_with_bump = seeds_ref.clone();
			seeds_with_bump.push(&bump_seed);
			let recreated = create_program_address(&seeds_with_bump, &pina::system::ID).unwrap();
			prop_assert_eq!(pda, recreated);
		}
	}
}

// ---------------------------------------------------------------------------
// parse_instruction: arbitrary instruction data never panics
// ---------------------------------------------------------------------------

proptest! {
	#[test]
	fn parse_instruction_u8_never_panics(
		program_id in any::<[u8; 32]>(),
		api_id in any::<[u8; 32]>(),
		ref data in prop::collection::vec(any::<u8>(), 0..256),
	) {
		let program_id = pina::Address::new_from_array(program_id);
		let api_id = pina::Address::new_from_array(api_id);
		let _ = parse_instruction::<u8>(&api_id, &program_id, data);
	}

	#[test]
	fn parse_instruction_u32_never_panics(
		program_id in any::<[u8; 32]>(),
		api_id in any::<[u8; 32]>(),
		ref data in prop::collection::vec(any::<u8>(), 0..256),
	) {
		let program_id = pina::Address::new_from_array(program_id);
		let api_id = pina::Address::new_from_array(api_id);
		let _ = parse_instruction::<u32>(&api_id, &program_id, data);
	}

	#[test]
	fn parse_instruction_u64_never_panics(
		program_id in any::<[u8; 32]>(),
		api_id in any::<[u8; 32]>(),
		ref data in prop::collection::vec(any::<u8>(), 0..256),
	) {
		let program_id = pina::Address::new_from_array(program_id);
		let api_id = pina::Address::new_from_array(api_id);
		let _ = parse_instruction::<u64>(&api_id, &program_id, data);
	}
}

// ---------------------------------------------------------------------------
// parse_instruction: exact matches succeed with correct data
// ---------------------------------------------------------------------------

proptest! {
	#[test]
	fn parse_instruction_u8_roundtrip(val: u8) {
		let id = pina::system::ID;
		let mut data = [0u8; 1];
		val.write_discriminator(&mut data);
		let parsed: u8 = parse_instruction(&id, &id, &data).unwrap();
		prop_assert_eq!(parsed, val);
	}

	#[test]
	fn parse_instruction_u32_roundtrip(val: u32) {
		let id = pina::system::ID;
		let mut data = [0u8; 4];
		val.write_discriminator(&mut data);
		let parsed: u32 = parse_instruction(&id, &id, &data).unwrap();
		prop_assert_eq!(parsed, val);
	}

	#[test]
	fn parse_instruction_mismatched_program_id_returns_error(
		val: u32,
		program_id in any::<[u8; 32]>(),
		api_id in any::<[u8; 32]>(),
	) {
		prop_assume!(program_id != api_id);
		let program_id = pina::Address::new_from_array(program_id);
		let api_id = pina::Address::new_from_array(api_id);
		let mut data = [0u8; 4];
		val.write_discriminator(&mut data);
		let result = parse_instruction::<u32>(&api_id, &program_id, &data);
		prop_assert_eq!(result, Err(pina::ProgramError::IncorrectProgramId));
	}

	#[test]
	fn parse_instruction_short_data_returns_error_u32(
		val: u32,
		len in 0usize..4usize,
	) {
		let id = pina::system::ID;
		let mut data = [0u8; 4];
		val.write_discriminator(&mut data);
		let result = parse_instruction::<u32>(&id, &id, &data[..len]);
		prop_assert_eq!(result, Err(pina::ProgramError::InvalidInstructionData));
	}
}

// ---------------------------------------------------------------------------
// PDA canonical bump is always <= 255
// ---------------------------------------------------------------------------

proptest! {
	#[test]
	fn pda_bump_is_u8(seeds in prop::collection::vec(any::<u8>(), 1..128)) {
		let seeds_ref: Vec<&[u8]> = seeds.chunks(32).map(|c| c.as_ref()).collect();
		if let Some((_, bump)) = try_find_program_address(&seeds_ref, &pina::system::ID) {
			// Bump is a u8 by construction; this assertion documents the
			// invariant that the canonical bump always fits in a byte.
			let _: u8 = bump;
		}
	}
}
