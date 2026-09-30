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

extern crate std;

use pina::*;
use program::*;

#[test]
fn discriminator_values() {
	assert_eq!(ProfileInstruction::Initialize as u8, 0);
	assert_eq!(ProfileInstruction::UpdateProfile as u8, 1);
	assert_eq!(ProfileInstruction::AddTag as u8, 2);
	assert_eq!(ProfileInstruction::RemoveTag as u8, 3);
}

#[test]
fn discriminator_roundtrip() {
	assert!(ProfileInstruction::try_from(0u8).is_ok());
	assert!(ProfileInstruction::try_from(3u8).is_ok());
	assert!(ProfileInstruction::try_from(99u8).is_err());
}

#[test]
fn profile_state_layout() {
	// 1 (discriminator) + 1 (migration version) + 1 (bump) + 33 (name)
	// + 129 (bio) + 66 (tags) + 9 (favorite tag) + 1 (active) = 241 bytes.
	assert_eq!(ProfileState::SIZE, 241);
}

#[test]
fn profile_state_discriminator() {
	assert!(ProfileState::matches_discriminator(&[
		ProfileAccountType::ProfileState as u8
	]));
	assert!(!ProfileState::matches_discriminator(&[0u8]));
}

#[test]
fn profile_state_initialization() {
	let mut bytes = [0u8; ProfileState::SIZE];
	let state = ProfileState::initialize(&mut bytes, |state| {
		state.bump = 42;
		state.active.set(true);
		Ok(())
	})
	.unwrap();
	assert_eq!(state.bump, 42);
	assert_eq!(state.name.as_str(), "");
	assert_eq!(state.tags.len(), 0);
	assert!(state.favorite_tag.is_none());
	assert!(state.active.get());
}

#[test]
fn bounded_string_roundtrip() {
	let empty = String::<32>::default();
	let name = String::<32>::try_from("alice")
		.unwrap_or_else(|error| panic!("encoding failed: {error:?}"));

	assert_eq!(empty.as_str(), "");
	assert_eq!(name.as_str(), "alice");
	assert_eq!(size_of::<String<32>>(), 33);
}

#[test]
fn bounded_fields_preserve_wire_layout() {
	let mut bytes = [0u8; ProfileState::SIZE];
	ProfileState::initialize(&mut bytes, |state| {
		state.name.try_set("alice")?;
		state.bio.try_set("hi")?;
		state.tags.try_set([7u64, 9u64])?;
		Ok(())
	})
	.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));

	// Payload offsets shift by the 2-byte envelope (1 discriminator +
	// 1 migration version); `tests/abi_layout.rs` records the same geometry:
	// bump at 2, the `name` length prefix at 3, and `tags` at 165.
	assert_eq!(bytes[2], 0);
	assert_eq!(bytes[3], 5);
	assert_eq!(&bytes[4..9], b"alice");
	assert!(bytes[9..36].iter().all(|byte| *byte == 0));
	assert_eq!(bytes[36], 2);
	assert_eq!(&bytes[37..39], b"hi");
	assert!(bytes[39..165].iter().all(|byte| *byte == 0));
	assert_eq!(&bytes[165..167], 2u16.to_le_bytes());
	assert_eq!(&bytes[167..175], 7u64.to_le_bytes());
	assert_eq!(&bytes[175..183], 9u64.to_le_bytes());
	assert!(bytes[183..231].iter().all(|byte| *byte == 0));
}

#[test]
fn bounded_string_rejects_invalid_utf8() {
	let mut bytes = [0u8; ProfileState::SIZE];
	ProfileState::initialize(&mut bytes, |_| Ok(())).unwrap();
	bytes[2] = 1;
	bytes[3] = 0xff;

	assert!(matches!(
		ProfileState::try_from_bytes(&bytes),
		Err(ProgramError::InvalidAccountData)
	));
}

#[test]
fn bounded_string_rejects_length_over_capacity() {
	let mut bytes = [0u8; ProfileState::SIZE];
	ProfileState::initialize(&mut bytes, |_| Ok(())).unwrap();
	// `bump` occupies offset 2 (discriminator + migration version), so the
	// `name` length prefix starts at 3 — the same geometry the generated
	// `tests/abi_layout.rs` records.
	bytes[3] = 33;

	assert!(matches!(
		ProfileState::try_from_bytes(&bytes),
		Err(ProgramError::InvalidAccountData)
	));
}

#[test]
fn bounded_tags_roundtrip() {
	let mut bytes = [0u8; ProfileState::SIZE];
	let state = ProfileState::initialize(&mut bytes, |_| Ok(())).unwrap();
	state.tags.try_push(1u64).unwrap();
	state.tags.try_push(2u64).unwrap();

	assert_eq!(state.tags.len(), 2);
	assert_eq!(state.tags.get(0).map(PodU64::get), Some(1));
	assert_eq!(state.tags.get(1).map(PodU64::get), Some(2));
	assert_eq!(state.tags.remove(0).map(|tag| tag.get()), Some(1));
	assert_eq!(state.tags.len(), 1);
	assert_eq!(state.tags.get(0).map(PodU64::get), Some(2));
}

#[test]
fn bounded_tags_reject_capacity_overflow() {
	let mut bytes = [0u8; ProfileState::SIZE];
	let state = ProfileState::initialize(&mut bytes, |_| Ok(())).unwrap();
	for tag in 0..8u64 {
		state.tags.try_push(tag).unwrap();
	}

	assert_eq!(state.tags.try_push(8u64), Err(PinaPodError::Overflow));
}

#[test]
fn bounded_tags_reject_length_over_capacity() {
	let mut bytes = [0u8; ProfileState::SIZE];
	ProfileState::initialize(&mut bytes, |_| Ok(())).unwrap();
	// `tags` starts at `MIGRATION_HEADER_SIZE + 163` == 165, so its length
	// prefix occupies 165..167.
	bytes[165..167].copy_from_slice(&9u16.to_le_bytes());

	assert!(matches!(
		ProfileState::try_from_bytes(&bytes),
		Err(ProgramError::InvalidAccountData)
	));
}

#[test]
fn initialize_instruction_data_layout() {
	// 1 (discriminator) + 1 (bump) + 33 (name) + 129 (bio) = 164 bytes.
	// Instructions carry no version envelope unless they opt into migrations.
	assert_eq!(InitializeInstruction::SIZE, 164);
	assert!(InitializeInstruction::matches_discriminator(&[
		ProfileInstruction::Initialize as u8
	]));
}

#[test]
fn update_profile_instruction_data_layout() {
	// 1 (discriminator) + 33 (name) + 129 (bio) = 163 bytes.
	assert_eq!(UpdateProfileInstruction::SIZE, 163);
}

#[test]
fn add_tag_instruction_data_layout() {
	// 1 (discriminator) + 8 (tag) = 9 bytes.
	assert_eq!(AddTagInstruction::SIZE, 9);
}

#[test]
fn remove_tag_instruction_data_layout() {
	// 1 (discriminator) + 8 (index) = 9 bytes.
	assert_eq!(RemoveTagInstruction::SIZE, 9);
}

#[test]
fn initialize_instruction_try_from_bytes() {
	let mut data = [0u8; InitializeInstruction::SIZE];
	InitializeInstruction::initialize(&mut data, |initialized| {
		initialized.bump = 42;
		initialized.name.try_set("ali")?;
		Ok(())
	})
	.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	let ix =
		InitializeInstruction::try_from_bytes(&data).unwrap_or_else(|e| panic!("failed: {e:?}"));
	assert_eq!(ix.bump, 42);
	assert_eq!(ix.name.as_str(), "ali");
}

#[test]
fn initialize_instruction_reports_invalid_utf8() {
	let mut data = [0u8; InitializeInstruction::SIZE];
	InitializeInstruction::initialize(&mut data, |_| Ok(()))
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	data[2] = 1;
	data[3] = 0xff;

	assert!(matches!(
		InitializeInstruction::try_from_bytes(&data),
		Err(ProgramError::InvalidInstructionData)
	));
}

#[test]
fn semantic_mutations_preserve_valid_profile_storage() {
	let mut bytes = [0u8; ProfileState::SIZE];

	{
		ProfileState::initialize(&mut bytes, |state| {
			state.name.try_set("alice")?;
			state.bio.try_set("hello")?;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	}
	{
		let state = ProfileState::try_from_bytes(&bytes)
			.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
		assert_eq!(state.name.as_str(), "alice");
		assert_eq!(state.bio.as_str(), "hello");
	}

	{
		let state = ProfileState::try_from_bytes_mut(&mut bytes)
			.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
		state
			.tags
			.try_push(7u64)
			.unwrap_or_else(|error| panic!("tag push failed: {error:?}"));
		state.favorite_tag.set(Some(PodU64::from(7)));
		state.active.set(true);
	}
	{
		let state = ProfileState::try_from_bytes(&bytes)
			.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
		assert_eq!(state.tags.len(), 1);
		assert_eq!(state.tags.get(0).map(PodU64::get), Some(7));
		assert_eq!(state.favorite_tag.get(), Some(PodU64::from(7)));
		assert!(state.active.get());
	}

	{
		let state = ProfileState::try_from_bytes_mut(&mut bytes)
			.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
		let removed = state.tags.remove(0);
		assert_eq!(removed.map(|tag| tag.get()), Some(7));
		state.tags.clear();
		state.favorite_tag.clear();
		state.active.set(false);
	}
	{
		let state = ProfileState::try_from_bytes(&bytes)
			.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
		assert_eq!(state.tags.len(), 0);
		assert_eq!(state.favorite_tag.get(), None);
		assert!(!state.active.get());
	}
}

#[test]
fn profile_seeds() {
	let authority = Address::new_from_array([1u8; 32]);
	let seeds = ProfileState::seeds(&authority);
	let slices = seeds.as_slices();
	assert_eq!(slices.len(), 2);
	assert_eq!(slices[0], b"profile");
	assert_eq!(slices[1], authority.as_ref());
}

#[test]
fn profile_seeds_with_bump() {
	let authority = Address::new_from_array([1u8; 32]);
	let seeds = ProfileState::seeds(&authority);
	let with_bump = seeds.with_bump(42);
	let slices = with_bump.as_slices();
	assert_eq!(slices.len(), 3);
	assert_eq!(slices[0], b"profile");
	assert_eq!(slices[1], authority.as_ref());
	assert_eq!(slices[2], &[42u8]);
}

#[test]
fn program_id_is_valid() {
	assert_ne!(ID, Address::default());
}
