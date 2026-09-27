//! Behavior tests for the account-realloc program's public API.
//!
//! These were inlined as a `#[cfg(test)]` module inside `src/lib.rs`. They moved
//! here because the migration-aware codegen those tests pin (envelope
//! geometry, version bytes) only expands when `migrations/manifest.json` is
//! discoverable — which holds for any crate inside this program's tree, but not
//! for a foreign harness that source-includes the program with `#[path]` (such
//! as the workspace compute-unit snapshot). Tests for the crate-private helper
//! functions stayed in `src/lib.rs`: they are the only consumers of those
//! helpers, and an integration test cannot reach them.

// The program is a cdylib only (see Cargo.toml), so its real types come
// in through a source include rather than an rlib dependency.
#[path = "../src/lib.rs"]
mod program;

// The in-crate module glob-imported the crate root; the integration test
// imports the included module and the framework instead.
use pina::*;
use program::*;

#[test]
fn parse_instruction_rejects_program_id_mismatch() {
	let wrong_program_id: Address = [5u8; 32].into();
	let data = [ReallocInstruction::Realloc as u8, 0];
	let result = parse_instruction::<ReallocInstruction>(&wrong_program_id, &ID, &data);
	assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
}

#[test]
fn realloc_instruction_roundtrip() {
	let mut bytes = [0u8; ReallocIx::SIZE];
	ReallocIx::initialize(&mut bytes, |ix| {
		ix.len.set(Sample::MIN_SIZE as u16);
		Ok(())
	})
	.unwrap_or_else(|e| panic!("encode: {e:?}"));
	let parsed = ReallocIx::try_from_bytes(&bytes).unwrap_or_else(|e| panic!("decode: {e:?}"));
	assert_eq!(usize::from(parsed.len.get()), Sample::MIN_SIZE);
}

#[test]
fn sample_pda_is_authority_bound() {
	let authority: Address = [1u8; 32].into();
	let attacker: Address = [2u8; 32].into();
	let (authority_sample, _) = Sample::find_pda(&authority, &ID);
	let (attacker_sample, _) = Sample::find_pda(&attacker, &ID);

	assert_ne!(authority_sample, attacker_sample);
}

#[test]
fn growth_limit_is_the_runtime_ten_kib_cap() {
	assert_eq!(MAX_PERMITTED_DATA_INCREASE, 10 * 1024);
}

#[test]
fn sample_compact_codec_roundtrips_active_values() {
	let target_size =
		Sample::projected_bytes(3).unwrap_or_else(|error| panic!("project size: {error:?}"));
	let mut backing = [0u8; Sample::MAX_SIZE];
	let data = &mut backing[..target_size];
	let values = [PodU64::from(3), PodU64::from(5), PodU64::from(8)];
	let encoded_size = Sample::initialize(
		&mut *data,
		&SamplePatch::new()
			.bump(7)
			.authority(Address::new_from_array([9; 32]))
			.replace_values(&values),
	)
	.unwrap_or_else(|error| panic!("initialize: {error:?}"));

	assert_eq!(encoded_size, target_size);
	let sample = Sample::try_from_bytes(&*data).unwrap_or_else(|error| panic!("decode: {error:?}"));
	assert_eq!(sample.encoded_len(), target_size);
	assert_eq!(sample.bump, 7);
	assert_eq!(sample.authority, Address::new_from_array([9; 32]));
	assert_eq!(sample.values(), values);
}
