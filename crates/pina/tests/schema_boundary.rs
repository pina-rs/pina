//! Runtime coverage for Pina's closed macro-generated storage grammar.

use pina::*;

#[discriminator(crate = ::pina, primitive = u8, final)]
enum SchemaKind {
	SafeState = 9,
}

#[account(crate = ::pina, discriminator = SchemaKind)]
struct SafeState {
	pub unsigned: u64,
	pub signed: i32,
	pub enabled: bool,
	pub owner: Address,
	pub digest: [u8; 16],
	pub maybe_count: Option<u64>,
	pub maybe_enabled: Option<bool>,
	pub pod_value: PodU32,
}

const ENABLED_OFFSET: usize = 1 + 8 + 4;
const MAYBE_COUNT_OFFSET: usize = ENABLED_OFFSET + 1 + 32 + 16;
const MAYBE_ENABLED_OFFSET: usize = MAYBE_COUNT_OFFSET + 1 + 8;

#[test]
fn supported_schema_roundtrips_every_audited_field() {
	let owner = Address::new_from_array([7u8; 32]);
	let digest = [11u8; 16];
	let mut bytes = [0u8; SafeState::SIZE];

	{
		let state = SafeState::initialize(&mut bytes)
			.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
		state.unsigned.set(u64::MAX);
		state.signed.set(-123);
		state.enabled.set(true);
		state.owner = owner;
		state.digest = digest;
		state.maybe_count.set(Some(PodU64::from(42)));
		state.maybe_enabled.set(Some(PodBool::from(false)));
		state.pod_value.set(99);
	}

	let state = SafeState::try_from_bytes(&bytes)
		.unwrap_or_else(|error| panic!("validation failed: {error:?}"));

	assert_eq!(state.unsigned.get(), u64::MAX);
	assert_eq!(state.signed.get(), -123);
	assert!(state.enabled.get());
	assert_eq!(state.owner, owner);
	assert_eq!(state.digest, digest);
	assert_eq!(state.maybe_count.get().map(|value| value.get()), Some(42));
	assert_eq!(
		state.maybe_enabled.get().map(|value| value.get()),
		Some(false)
	);
	assert_eq!(state.pod_value.get(), 99);
}

#[test]
fn validation_rejects_noncanonical_boolean() {
	let mut bytes = [0u8; SafeState::SIZE];
	SafeState::initialize(&mut bytes)
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	bytes[ENABLED_OFFSET] = 2;

	assert!(SafeState::try_from_bytes(&bytes).is_err());
}

#[test]
fn validation_rejects_noncanonical_option_tag() {
	let mut bytes = [0u8; SafeState::SIZE];
	SafeState::initialize(&mut bytes)
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	bytes[MAYBE_COUNT_OFFSET] = 2;

	assert!(SafeState::try_from_bytes(&bytes).is_err());
}

#[test]
fn validation_recurses_into_present_option_values() {
	let mut bytes = [0u8; SafeState::SIZE];
	SafeState::initialize(&mut bytes)
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	bytes[MAYBE_ENABLED_OFFSET] = 1;
	bytes[MAYBE_ENABLED_OFFSET + 1] = 2;

	assert!(SafeState::try_from_bytes(&bytes).is_err());
}

#[test]
fn every_supported_mutation_leaves_the_full_backing_slice_readable() {
	let mut bytes = [0u8; SafeState::SIZE];

	{
		let state = SafeState::initialize(&mut bytes)
			.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
		state.unsigned.set(1);
	}
	assert_eq!(bytes.iter().copied().fold(0u8, u8::wrapping_add), 10);

	{
		let state = SafeState::try_from_bytes_mut(&mut bytes)
			.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
		state.enabled.set(true);
		state.maybe_count.set(Some(PodU64::from(2)));
		state.maybe_enabled.set(Some(PodBool::from(true)));
	}
	let checksum = bytes.iter().copied().fold(0u8, u8::wrapping_add);
	assert_eq!(checksum, 16);

	{
		let state = SafeState::try_from_bytes_mut(&mut bytes)
			.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
		state.maybe_count.clear();
		state.maybe_enabled.clear();
	}
	assert_eq!(bytes.iter().copied().fold(0u8, u8::wrapping_add), 11);
}
