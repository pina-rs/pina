#![cfg(feature = "compact")]
#![allow(unsafe_code)]

use core::mem::size_of;

use pina::*;
use pinocchio::account::NOT_BORROWED;
use pinocchio::account::RuntimeAccount;

const OWNER: Address = Address::new_from_array([9; 32]);

#[discriminator(crate = ::pina)]
enum CompactKind {
	DynamicState = 7,
	ThreeTailState = 8,
	PrefixState = 9,
	StringState = 10,
	StringPrefixState = 11,
}

#[account(crate = ::pina, discriminator = CompactKind, compact)]
struct DynamicState {
	pub bump: u8,
	pub authority: Address,
	pub values: Vec<u64, 4>,
	pub codes: Vec<u16, 3>,
}

#[account(
	crate = ::pina,
	discriminator = CompactKind,
	variant = ThreeTailState,
	compact
)]
struct ThreeTailState {
	pub marker: u8,
	pub bytes: Vec<u8, 2>,
	pub words: Vec<u16, 2>,
	pub triples: Vec<[u8; 3], 2>,
}

#[account(crate = ::pina, discriminator = CompactKind, compact)]
struct PrefixState {
	pub one: PodVec<u8, 2, 1>,
	pub two: PodVec<PodU16, 2, 2>,
	pub four: PodVec<PodU32, 2, 4>,
	pub eight: PodVec<PodU64, 2, 8>,
}

#[account(crate = ::pina, discriminator = CompactKind, compact)]
struct StringState {
	/// Semantic optional values use `PodOption<PodU64>` in the compact header.
	pub featured: Option<u64>,
	/// Strings are encoded as active UTF-8 tail bytes, without inactive capacity.
	pub title: PodString<12>,
	pub values: Vec<u8, 2>,
}

#[account(crate = ::pina, discriminator = CompactKind, compact)]
struct StringPrefixState {
	pub one: String<2>,
	pub two: PodString<2, 2>,
	pub four: PodString<2, 4>,
	pub eight: PodString<2, 8>,
}

#[test]
fn compact_account_combines_a_pod_option_header_with_a_pod_string_tail() {
	assert_eq!(size_of::<PodOption<PodU64>>(), 9);
	assert_eq!(StringState::HEADER_SIZE, 13);
	assert_eq!(StringState::TITLE_CAPACITY, 12);
	assert_eq!(StringState::VALUES_CAPACITY, 2);
	assert_eq!(StringState::projected_bytes(5, 2), Ok(20));

	let values = [3u8, 5];
	let mut data = [0u8; StringState::MAX_SIZE];
	let encoded_size = StringState::initialize(
		&mut data,
		&StringStatePatch::new()
			.featured(Some(42_u64))
			.title("piña")
			.replace_values(&values),
	)
	.unwrap_or_else(|error| panic!("initialize string state: {error:?}"));
	assert_eq!(encoded_size, 20);

	let state = StringState::try_from_bytes(&data[..encoded_size])
		.unwrap_or_else(|error| panic!("read string state: {error:?}"));
	assert_eq!(state.featured.get().map(|value| value.get()), Some(42));
	assert_eq!(state.title(), "piña");
	assert_eq!(state.values(), &values);
}

#[test]
fn compact_strings_support_every_prefix_width_and_independent_lengths() {
	assert_eq!(StringPrefixState::HEADER_SIZE, 16);
	assert_eq!(StringPrefixState::MIN_SIZE, 16);
	assert_eq!(StringPrefixState::MAX_SIZE, 24);
	assert_eq!(StringPrefixState::projected_bytes(1, 2, 0, 2), Ok(21));

	let mut data = [0u8; StringPrefixState::MAX_SIZE];
	let encoded_size = StringPrefixState::initialize(
		&mut data,
		&StringPrefixStatePatch::new()
			.one("a")
			.two("bc")
			.four("")
			.eight("de"),
	)
	.unwrap_or_else(|error| panic!("initialize string prefixes: {error:?}"));

	let state = StringPrefixState::try_from_bytes(&data[..encoded_size])
		.unwrap_or_else(|error| panic!("read string prefixes: {error:?}"));
	assert_eq!(state.one(), "a");
	assert_eq!(state.two(), "bc");
	assert_eq!(state.four(), "");
	assert_eq!(state.eight(), "de");
}

#[test]
fn compact_account_preserves_full_tails_across_two_full_replacements() {
	let byte_values = [[1u8, 2], [3, 4], [5, 6]];
	let word_values = [
		[PodU16::from(10), PodU16::from(11)],
		[PodU16::from(12), PodU16::from(13)],
		[PodU16::from(14), PodU16::from(15)],
	];
	let triple_values = [
		[[20u8, 21, 22], [23, 24, 25]],
		[[26, 27, 28], [29, 30, 31]],
		[[32, 33, 34], [35, 36, 37]],
	];
	let mut data = [0u8; ThreeTailState::MAX_SIZE];
	ThreeTailState::initialize(
		&mut data,
		&ThreeTailStatePatch::new()
			.replace_bytes(&byte_values[0])
			.replace_words(&word_values[0])
			.replace_triples(&triple_values[0]),
	)
	.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));

	ThreeTailState::update(
		&mut data,
		&ThreeTailStatePatch::new().replace_words(&word_values[1]),
	)
	.unwrap();

	let committed_size = ThreeTailState::update(
		&mut data,
		&ThreeTailStatePatch::new().replace_triples(&triple_values[2]),
	)
	.unwrap();
	let state = ThreeTailState::try_from_bytes(&data[..committed_size]).unwrap();
	assert_eq!(state.bytes(), &byte_values[0]);
	assert_eq!(state.words(), &word_values[1]);
	assert_eq!(state.triples(), &triple_values[2]);
}

#[test]
fn compact_account_roundtrips_active_tail_without_fixed_capacity_padding() {
	assert_eq!(DynamicState::HEADER_SIZE, 38);
	assert_eq!(DynamicState::MIN_SIZE, DynamicState::HEADER_SIZE);
	assert_eq!(DynamicState::MAX_SIZE, 76);
	assert_eq!(DynamicState::TAIL_ALIGNMENT, 2);
	assert_eq!(DynamicState::VALUES_CAPACITY, 4);
	assert_eq!(DynamicState::CODES_CAPACITY, 3);
	assert_eq!(
		DynamicState::projected_bytes(2, 2),
		Ok(DynamicState::HEADER_SIZE + 16 + 4)
	);

	let mut data = [0u8; DynamicState::MAX_SIZE];
	let values = [PodU64::from(11), PodU64::from(22)];
	let codes = [PodU16::from(3), PodU16::from(5)];
	let encoded_size = DynamicState::initialize(
		&mut data,
		&DynamicStatePatch::new()
			.bump(3)
			.authority(Address::new_from_array([5; 32]))
			.replace_values(&values)
			.replace_codes(&codes),
	)
	.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));

	assert_eq!(encoded_size, DynamicState::HEADER_SIZE + 16 + 4);
	assert_eq!(&data[34..36], &2u16.to_le_bytes());
	assert_eq!(&data[36..38], &2u16.to_le_bytes());
	let state = DynamicState::try_from_bytes(&data[..encoded_size])
		.unwrap_or_else(|error| panic!("read compact state: {error:?}"));
	assert_eq!(state.bump, 3);
	assert_eq!(state.authority, Address::new_from_array([5; 32]));
	assert_eq!(state.values()[0].get(), 11);
	assert_eq!(state.values()[1].get(), 22);
	assert_eq!(state.codes(), &codes);
	assert_eq!(state.encoded_len(), encoded_size);
}

#[test]
fn projected_bytes_rejects_each_tail_past_its_own_capacity() {
	assert_eq!(
		DynamicState::projected_bytes(DynamicState::VALUES_CAPACITY + 1, 0),
		Err(ProgramError::InvalidAccountData)
	);
	assert_eq!(
		DynamicState::projected_bytes(0, DynamicState::CODES_CAPACITY + 1),
		Err(ProgramError::InvalidAccountData)
	);
}

#[test]
fn compact_size_helpers_cover_every_prefix_width_and_staged_edit() {
	assert_eq!(PrefixState::MIN_SIZE, PrefixState::HEADER_SIZE);
	assert_eq!(PrefixState::HEADER_SIZE, 16);
	assert_eq!(PrefixState::ONE_CAPACITY, 2);
	assert_eq!(PrefixState::TWO_CAPACITY, 2);
	assert_eq!(PrefixState::FOUR_CAPACITY, 2);
	assert_eq!(PrefixState::EIGHT_CAPACITY, 2);

	let one = [1u8, 2];
	let two = [PodU16::from(3)];
	let four = [PodU32::from(5), PodU32::from(8)];
	let eight = [PodU64::from(13)];
	let expected = PrefixState::projected_bytes(one.len(), two.len(), four.len(), eight.len())
		.unwrap_or_else(|error| panic!("project prefix state size: {error:?}"));
	let mut data = [0u8; PrefixState::MAX_SIZE];
	let committed = PrefixState::initialize(
		&mut data,
		&PrefixStatePatch::new()
			.replace_one(&one)
			.replace_two(&two)
			.replace_four(&four)
			.replace_eight(&eight),
	)
	.unwrap_or_else(|error| panic!("initialize prefix state: {error:?}"));
	assert_eq!(committed, expected);

	let state = PrefixState::try_from_bytes(&data)
		.unwrap_or_else(|error| panic!("read prefix state: {error:?}"));
	assert_eq!(state.encoded_len(), committed);
	assert!(state.encoded_len() < data.len());

	for counts in [[3, 0, 0, 0], [0, 3, 0, 0], [0, 0, 3, 0], [0, 0, 0, 3]] {
		assert_eq!(
			PrefixState::projected_bytes(counts[0], counts[1], counts[2], counts[3]),
			Err(ProgramError::InvalidAccountData)
		);
	}

	let projected =
		PrefixState::updated_len(&data, &PrefixStatePatch::new().replace_one(&one[..1]))
			.unwrap_or_else(|error| panic!("project prefix state update: {error:?}"));
	assert_eq!(projected, committed - 1);
}

#[test]
fn compact_account_moves_later_tails_when_an_earlier_tail_changes_size() {
	let mut data = [0u8; DynamicState::MAX_SIZE];
	let initial_values = [PodU64::from(11)];
	let grown_values = [PodU64::from(11), PodU64::from(22), PodU64::from(33)];
	let codes = [PodU16::from(13)];

	let initial_size = DynamicState::initialize(
		&mut data,
		&DynamicStatePatch::new()
			.replace_values(&initial_values)
			.replace_codes(&codes),
	)
	.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));
	assert_eq!(initial_size, DynamicState::HEADER_SIZE + 8 + 2);

	let grown_size = DynamicState::update(
		&mut data,
		&DynamicStatePatch::new().replace_values(&grown_values),
	)
	.unwrap_or_else(|error| panic!("grow values: {error:?}"));

	let state = DynamicState::try_from_bytes(&data[..grown_size])
		.unwrap_or_else(|error| panic!("read grown compact state: {error:?}"));
	assert_eq!(state.values(), &grown_values);
	assert_eq!(&data[36..38], &1u16.to_le_bytes());
	let codes_offset = DynamicState::HEADER_SIZE + grown_values.len() * size_of::<PodU64>();
	assert_eq!(&data[codes_offset..codes_offset + 2], &13u16.to_le_bytes());
}

#[test]
fn compact_vec_accessors_use_their_own_independent_lengths() {
	let mut data = [0u8; DynamicState::MAX_SIZE];
	let values = [PodU64::from(11)];
	let codes = [PodU16::from(3), PodU16::from(5), PodU16::from(8)];
	let encoded_size = DynamicState::initialize(
		&mut data,
		&DynamicStatePatch::new()
			.replace_values(&values)
			.replace_codes(&codes),
	)
	.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));

	let state = DynamicState::try_from_bytes(&data[..encoded_size])
		.unwrap_or_else(|error| panic!("read compact state: {error:?}"));
	assert_eq!(state.codes(), &codes);
}

#[test]
fn compact_account_validation_rejects_every_invalid_boundary() {
	let mut too_small = [0u8; DynamicState::HEADER_SIZE - 1];
	let mut too_large = [0u8; DynamicState::MAX_SIZE + 1];
	assert!(DynamicState::initialize(&mut too_small, &DynamicStatePatch::new()).is_err());
	assert!(DynamicState::initialize(&mut too_large, &DynamicStatePatch::new()).is_err());
	DynamicState::initialize(
		&mut too_large[..DynamicState::MAX_SIZE],
		&DynamicStatePatch::new(),
	)
	.unwrap_or_else(|error| panic!("initialize maximum compact storage: {error:?}"));
	assert!(DynamicState::try_from_bytes(&too_large).is_err());
	assert!(DynamicState::updated_len(&too_large, &DynamicStatePatch::new()).is_err());

	let mut split_alignment = [0u8; DynamicState::HEADER_SIZE + 1];
	assert!(DynamicState::initialize(&mut split_alignment, &DynamicStatePatch::new()).is_err());
	let mut spare_capacity = [0u8; DynamicState::HEADER_SIZE + 2];
	assert!(DynamicState::initialize(&mut spare_capacity, &DynamicStatePatch::new()).is_ok());

	let mut data = [0u8; DynamicState::HEADER_SIZE];
	DynamicState::initialize(&mut data, &DynamicStatePatch::new())
		.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));
	data[0] = 99;
	assert!(DynamicState::try_from_bytes(&data).is_err());

	data[0] = CompactKind::DynamicState as u8;
	data[34] = 5;
	assert!(DynamicState::try_from_bytes(&data).is_err());

	data[34] = 0;
	data[36] = 4;
	assert!(DynamicState::try_from_bytes(&data).is_err());
}

#[test]
fn compact_header_uses_account_validation() {
	let mut data = [0u8; DynamicState::HEADER_SIZE];
	let state = DynamicState::initialize(&mut data, &DynamicStatePatch::new())
		.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));
	assert_eq!(state, DynamicState::HEADER_SIZE);
	let state = DynamicState::try_from_bytes(&data).unwrap();
	assert!(state.assert(|header| header.bump == 0).is_ok());
}

#[test]
fn account_view_loaders_scope_compact_borrows() {
	let mut stored = TestAccount::<{ DynamicState::MAX_SIZE }>::new();
	let mut account = stored.view();
	{
		let mut data = account
			.try_borrow_mut()
			.unwrap_or_else(|error| panic!("borrow compact data: {error:?}"));
		DynamicState::initialize(&mut data, &DynamicStatePatch::new())
			.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));
	}

	account
		.update_compact_account::<DynamicState>(&OWNER, &DynamicStatePatch::new().bump(8))
		.unwrap_or_else(|error| panic!("mutate compact account: {error:?}"));
	let bump = account
		.with_compact_account::<DynamicState, _>(&OWNER, |state| Ok(state.bump))
		.unwrap_or_else(|error| panic!("read compact account: {error:?}"));

	assert_eq!(bump, 8);
	assert!(account.assert_compact_type::<DynamicState>(&OWNER).is_ok());
}

#[test]
fn compact_updates_reject_readonly_accounts_without_changing_bytes() {
	let mut stored = TestAccount::<{ DynamicState::MAX_SIZE }>::new();
	DynamicState::initialize(&mut stored.data, &DynamicStatePatch::new())
		.unwrap_or_else(|error| panic!("initialize compact data: {error:?}"));
	stored.header.is_writable = 0;
	let before = stored.data;
	let mut account = stored.view();

	let result =
		account.update_compact_account::<DynamicState>(&OWNER, &DynamicStatePatch::new().bump(8));

	assert_eq!(result, Err(ProgramError::InvalidAccountData));
	let data = account
		.try_borrow()
		.unwrap_or_else(|error| panic!("borrow unchanged compact data: {error:?}"));
	assert_eq!(&*data, &before);
}

#[repr(C)]
struct TestAccount<const N: usize> {
	header: RuntimeAccount,
	data: [u8; N],
}

impl<const N: usize> TestAccount<N> {
	fn new() -> Self {
		Self {
			header: RuntimeAccount {
				borrow_state: NOT_BORROWED,
				is_signer: 0,
				is_writable: 1,
				executable: 0,
				padding: [0; 4],
				address: Address::new_from_array([1; 32]),
				owner: OWNER,
				lamports: 1,
				data_len: N as u64,
			},
			data: [0; N],
		}
	}

	fn view(&mut self) -> AccountView {
		unsafe { AccountView::new_unchecked(core::ptr::addr_of_mut!(self.header)) }
	}
}
