//! Behavior tests that were inlined as `#[cfg(test)]` modules inside `src/lib.rs`.
//!
//! They moved here because the migration-aware codegen those tests pin
//! (envelope geometry, version bytes) only expands when `migrations/manifest.json`
//! is discoverable — which holds for any crate inside this program's tree, but
//! not for a foreign harness that source-includes the program with `#[path]`
//! (such as the workspace compute-unit snapshot). From `tests/` the manifest
//! discovery walk resolves the program's manifest exactly as the in-crate
//! module did, and every assertion keeps its meaning.

// The program is a cdylib only (see Cargo.toml), so its real types come
// in through a source include rather than an rlib dependency, and the
// framework names the tests glob-imported through the crate root come from
// `pina` itself.
#[path = "../src/lib.rs"]
mod program;

// The in-crate module glob-imported the crate root; the integration test
// imports the included module instead.
use pina::*;
use program::*;

#[test]
fn discriminator_values() {
	assert_eq!(CounterInstruction::Initialize as u8, 0);
	assert_eq!(CounterInstruction::Increment as u8, 1);
}

#[test]
fn discriminator_roundtrip() {
	assert!(CounterInstruction::try_from(0u8).is_ok());
	assert!(CounterInstruction::try_from(1u8).is_ok());
	assert!(CounterInstruction::try_from(99u8).is_err());
}

#[test]
fn counter_state_layout() {
	// CounterState: 1 (discriminator) + 1 (migration version) + 1 (bump) +
	// 8 (count) = 11 bytes. Migrations are on, so every account envelope
	// carries the version byte and the generated `tests/abi_layout.rs` pins
	// the same geometry.
	assert_eq!(CounterState::SIZE, 11);
}

#[test]
fn counter_state_discriminator() {
	assert!(CounterState::matches_discriminator(&[
		CounterAccountType::CounterState as u8
	]));
	assert!(!CounterState::matches_discriminator(&[0u8]));
}

#[test]
fn counter_state_initialization() {
	let mut bytes = [0u8; CounterState::SIZE];
	let state = CounterState::initialize(&mut bytes, |state| {
		state.bump = 42;
		state.count.set(100);
		Ok(())
	})
	.unwrap();
	assert_eq!(state.bump, 42);
	assert_eq!(state.count.get(), 100);
}

#[test]
fn counter_state_deserialize_roundtrip() {
	let mut bytes = [0u8; CounterState::SIZE];
	CounterState::initialize(&mut bytes, |state| {
		state.bump = 7;
		state.count.set(999);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	assert_eq!(bytes.len(), 11);

	// Deserialize back.
	let deserialized = CounterState::try_from_bytes(&bytes)
		.unwrap_or_else(|e| panic!("deserialization failed: {e:?}"));
	assert_eq!(deserialized.bump, 7);
	assert_eq!(deserialized.count.get(), 999);
}

#[test]
fn initialize_instruction_data_layout() {
	// InitializeInstruction: 1 (discriminator) + 1 (bump) = 2 bytes.
	// Instructions carry no version envelope unless they opt into migrations.
	assert_eq!(InitializeInstruction::SIZE, 2);
	assert!(InitializeInstruction::matches_discriminator(&[
		CounterInstruction::Initialize as u8
	]));
}

#[test]
fn increment_instruction_data_layout() {
	// IncrementInstruction: 1 (discriminator).
	assert_eq!(IncrementInstruction::SIZE, 1);
	assert!(IncrementInstruction::matches_discriminator(&[
		CounterInstruction::Increment as u8
	]));
}

#[test]
fn initialize_instruction_try_from_bytes() {
	// discriminator + bump.
	let data = [CounterInstruction::Initialize as u8, 42u8];
	let ix =
		InitializeInstruction::try_from_bytes(&data).unwrap_or_else(|e| panic!("failed: {e:?}"));
	assert_eq!(ix.bump, 42);
}

#[test]
fn increment_instruction_try_from_bytes() {
	let data = [CounterInstruction::Increment as u8];
	let result = IncrementInstruction::try_from_bytes(&data);
	assert!(result.is_ok());
}

#[test]
fn counter_seeds() {
	let authority = Address::new_from_array([1u8; 32]);
	let seeds = CounterState::seeds(&authority);
	let slices = seeds.as_slices();
	assert_eq!(slices.len(), 2);
	assert_eq!(slices[0], b"counter");
	assert_eq!(slices[1], authority.as_ref());
}

#[test]
fn counter_seeds_with_bump() {
	let authority = Address::new_from_array([1u8; 32]);
	let seeds = CounterState::seeds(&authority);
	let with_bump = seeds.with_bump(42);
	let slices = with_bump.as_slices();
	assert_eq!(slices.len(), 3);
	assert_eq!(slices[0], b"counter");
	assert_eq!(slices[1], authority.as_ref());
	assert_eq!(slices[2], &[42u8]);
}

#[test]
fn program_id_is_valid() {
	assert_ne!(ID, Address::default());
}
