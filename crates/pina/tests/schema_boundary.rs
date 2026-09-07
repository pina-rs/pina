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

#[account(crate = ::pina, discriminator = SchemaKind, variant = SafeState)]
struct FixedCollectionsState {
	pub title: String<4>,
	pub values: Vec<u16, 2>,
	pub note: Option<String<4>>,
	pub labels: Vec<String<3>, 2>,
	pub nested: Option<Option<bool>>,
	pub flags: Option<Vec<bool, 2>>,
}

const ENABLED_OFFSET: usize = 1 + 8 + 4;
const MAYBE_COUNT_OFFSET: usize = ENABLED_OFFSET + 1 + 32 + 16;
const MAYBE_ENABLED_OFFSET: usize = MAYBE_COUNT_OFFSET + 1 + 8;
const TITLE_OFFSET: usize = 1;
const VALUES_OFFSET: usize = TITLE_OFFSET + 1 + 4;
const NOTE_OFFSET: usize = VALUES_OFFSET + 2 + 2 * 2;
const LABELS_OFFSET: usize = NOTE_OFFSET + 1 + 1 + 4;
const NESTED_OFFSET: usize = LABELS_OFFSET + 2 + 2 * (1 + 3);
const FLAGS_OFFSET: usize = NESTED_OFFSET + 1 + 1 + 1;

#[test]
fn supported_schema_roundtrips_every_audited_field() {
	let owner = Address::new_from_array([7u8; 32]);
	let digest = [11u8; 16];
	let mut bytes = [0u8; SafeState::SIZE];

	{
		SafeState::initialize(&mut bytes, |state| {
			state.unsigned.set(u64::MAX);
			state.signed.set(-123);
			state.enabled.set(true);
			state.owner = owner;
			state.digest = digest;
			state.maybe_count.set(Some(PodU64::from(42)));
			state.maybe_enabled.set(Some(PodBool::from(false)));
			state.pod_value.set(99);
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
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
	SafeState::initialize(&mut bytes, |_| Ok(()))
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	bytes[ENABLED_OFFSET] = 2;

	assert!(SafeState::try_from_bytes(&bytes).is_err());
}

#[test]
fn validation_rejects_noncanonical_option_tag() {
	let mut bytes = [0u8; SafeState::SIZE];
	SafeState::initialize(&mut bytes, |_| Ok(()))
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	bytes[MAYBE_COUNT_OFFSET] = 2;

	assert!(SafeState::try_from_bytes(&bytes).is_err());
}

#[test]
fn validation_recurses_into_present_option_values() {
	let mut bytes = [0u8; SafeState::SIZE];
	SafeState::initialize(&mut bytes, |_| Ok(()))
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	bytes[MAYBE_ENABLED_OFFSET] = 1;
	bytes[MAYBE_ENABLED_OFFSET + 1] = 2;

	assert!(SafeState::try_from_bytes(&bytes).is_err());
}

#[test]
fn every_supported_mutation_leaves_the_full_backing_slice_readable() {
	let mut bytes = [0u8; SafeState::SIZE];

	{
		SafeState::initialize(&mut bytes, |state| {
			state.unsigned.set(1);
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
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

#[test]
fn failed_initialization_zeros_the_complete_destination() {
	let mut bytes = [0xff; SafeState::SIZE];
	let result = SafeState::initialize(&mut bytes, |state| {
		state.unsigned.set(42);
		Err(PinaPodError::InvalidLength)
	});

	assert!(matches!(result, Err(ProgramError::InvalidAccountData)));
	assert_eq!(bytes, [0; SafeState::SIZE]);
}

#[test]
fn fixed_collections_preserve_the_declared_wire_layout() {
	let mut bytes = [0u8; FixedCollectionsState::SIZE];

	{
		FixedCollectionsState::initialize(&mut bytes, |state| {
			state.title.try_set("pina")?;
			state.values.try_set([7u16, 11])?;
			state.note.set(Some(PodString::try_from("pod")?));
			state
				.labels
				.try_set([PodString::try_from("one")?, PodString::try_from("two")?])?;
			state.nested.set(Some(PodOption::some(PodBool::from(true))));

			let mut flags = PodVec::<bool, 2>::default();
			flags.try_set([true, false])?;
			state.flags.set(Some(flags));
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	}

	assert_eq!(bytes[0], SchemaKind::SafeState as u8);
	assert_eq!(
		&bytes[TITLE_OFFSET..TITLE_OFFSET + 5],
		&[4, b'p', b'i', b'n', b'a']
	);
	assert_eq!(
		&bytes[VALUES_OFFSET..VALUES_OFFSET + 6],
		&[2, 0, 7, 0, 11, 0]
	);
	assert_eq!(
		&bytes[NOTE_OFFSET..NOTE_OFFSET + 6],
		&[1, 3, b'p', b'o', b'd', 0]
	);
	assert_eq!(
		&bytes[LABELS_OFFSET..LABELS_OFFSET + 10],
		&[2, 0, 3, b'o', b'n', b'e', 3, b't', b'w', b'o']
	);
	assert_eq!(&bytes[NESTED_OFFSET..NESTED_OFFSET + 3], &[1, 1, 1]);
	assert_eq!(&bytes[FLAGS_OFFSET..FLAGS_OFFSET + 5], &[1, 2, 0, 1, 0]);

	let state = FixedCollectionsState::try_from_bytes(&bytes)
		.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
	assert_eq!(state.title.as_str(), "pina");
	assert_eq!(state.values[1].get(), 11);
	assert_eq!(state.note.get_ref().map(PodString::as_str), Some("pod"));
	assert_eq!(state.labels[1].as_str(), "two");
	assert_eq!(
		state
			.nested
			.get_ref()
			.and_then(PodOption::get_ref)
			.map(PodBool::get),
		Some(true)
	);
}

#[test]
fn fixed_collection_mutations_enforce_capacity_and_clear_inactive_bytes() {
	let mut bytes = [0u8; FixedCollectionsState::SIZE];
	let state = FixedCollectionsState::initialize(&mut bytes, |_| Ok(()))
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));

	assert!(state.values.try_set([1u16, 2, 3]).is_err());
	state.title.try_set("pina").unwrap();
	state.title.clear();
	state.values.try_set([7u16, 11]).unwrap();
	state.values.clear();

	assert_eq!(&bytes[TITLE_OFFSET..TITLE_OFFSET + 5], &[0; 5]);
	assert_eq!(&bytes[VALUES_OFFSET..VALUES_OFFSET + 6], &[0; 6]);
}

#[test]
fn fixed_collection_validation_rejects_invalid_nested_values() {
	let mut invalid_vector_string = [0u8; FixedCollectionsState::SIZE];
	FixedCollectionsState::initialize(&mut invalid_vector_string, |_| Ok(()))
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	invalid_vector_string[LABELS_OFFSET..LABELS_OFFSET + 2].copy_from_slice(&1u16.to_le_bytes());
	invalid_vector_string[LABELS_OFFSET + 2] = 1;
	invalid_vector_string[LABELS_OFFSET + 3] = 0xff;
	assert!(FixedCollectionsState::try_from_bytes(&invalid_vector_string).is_err());

	let mut invalid_option_string = [0u8; FixedCollectionsState::SIZE];
	FixedCollectionsState::initialize(&mut invalid_option_string, |_| Ok(()))
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	invalid_option_string[NOTE_OFFSET] = 1;
	invalid_option_string[NOTE_OFFSET + 1] = 1;
	invalid_option_string[NOTE_OFFSET + 2] = 0xff;
	assert!(FixedCollectionsState::try_from_bytes(&invalid_option_string).is_err());

	let mut invalid_option_vector = [0u8; FixedCollectionsState::SIZE];
	FixedCollectionsState::initialize(&mut invalid_option_vector, |_| Ok(()))
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	invalid_option_vector[FLAGS_OFFSET] = 1;
	invalid_option_vector[FLAGS_OFFSET + 1..FLAGS_OFFSET + 3].copy_from_slice(&1u16.to_le_bytes());
	invalid_option_vector[FLAGS_OFFSET + 3] = 2;
	assert!(FixedCollectionsState::try_from_bytes(&invalid_option_vector).is_err());
}

#[test]
fn fixed_collection_validation_rejects_noncanonical_nested_option_tags() {
	let mut outer = [0u8; FixedCollectionsState::SIZE];
	FixedCollectionsState::initialize(&mut outer, |_| Ok(()))
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	outer[NESTED_OFFSET] = 2;
	assert!(FixedCollectionsState::try_from_bytes(&outer).is_err());

	let mut inner = [0u8; FixedCollectionsState::SIZE];
	FixedCollectionsState::initialize(&mut inner, |_| Ok(()))
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	inner[NESTED_OFFSET] = 1;
	inner[NESTED_OFFSET + 1] = 2;
	assert!(FixedCollectionsState::try_from_bytes(&inner).is_err());
}
