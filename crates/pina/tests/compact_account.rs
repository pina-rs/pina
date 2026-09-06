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
	{
		let mut state = ThreeTailState::initialize(&mut data)
			.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));
		state.set_bytes(&byte_values[0]).unwrap();
		state.set_words(&word_values[0]).unwrap();
		state.set_triples(&triple_values[0]).unwrap();
		state.commit().unwrap();
	}

	{
		let mut state = ThreeTailState::try_from_bytes_mut(&mut data).unwrap();
		state.set_words(&word_values[1]).unwrap();
		state.commit().unwrap();
	}

	let committed_size = {
		let mut state = ThreeTailState::try_from_bytes_mut(&mut data).unwrap();
		state.set_triples(&triple_values[2]).unwrap();
		state.commit().unwrap()
	};
	let state = ThreeTailState::try_from_bytes(&data[..committed_size]).unwrap();
	assert_eq!(state.bytes(), &byte_values[0]);
	assert_eq!(state.words(), &word_values[1]);
	assert_eq!(state.triples(), &triple_values[2]);
}

#[test]
fn compact_account_roundtrips_active_tail_without_fixed_capacity_padding() {
	assert_eq!(DynamicState::HEADER_SIZE, 38);
	assert_eq!(DynamicState::MAX_SIZE, 76);
	assert_eq!(DynamicState::TAIL_ALIGNMENT, 2);

	let mut data = [0u8; DynamicState::MAX_SIZE];
	let values = [PodU64::from(11), PodU64::from(22)];
	let codes = [PodU16::from(3), PodU16::from(5)];
	let encoded_size = {
		let mut state = DynamicState::initialize(&mut data)
			.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));
		state.bump = 3;
		state.authority = Address::new_from_array([5; 32]);
		state
			.set_values(&values)
			.unwrap_or_else(|error| panic!("set compact values: {error:?}"));
		state
			.set_codes(&codes)
			.unwrap_or_else(|error| panic!("set compact codes: {error:?}"));
		state
			.commit()
			.unwrap_or_else(|error| panic!("commit compact state: {error:?}"))
	};

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
}

#[test]
fn compact_account_moves_later_tails_when_an_earlier_tail_changes_size() {
	let mut data = [0u8; DynamicState::MAX_SIZE];
	let initial_values = [PodU64::from(11)];
	let grown_values = [PodU64::from(11), PodU64::from(22), PodU64::from(33)];
	let codes = [PodU16::from(13)];

	let initial_size = {
		let mut state = DynamicState::initialize(&mut data)
			.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));
		state
			.set_values(&initial_values)
			.unwrap_or_else(|error| panic!("set initial values: {error:?}"));
		state
			.set_codes(&codes)
			.unwrap_or_else(|error| panic!("set codes: {error:?}"));
		state
			.commit()
			.unwrap_or_else(|error| panic!("commit initial state: {error:?}"))
	};
	assert_eq!(initial_size, DynamicState::HEADER_SIZE + 8 + 2);

	let grown_size = {
		let mut state = DynamicState::try_from_bytes_mut(&mut data)
			.unwrap_or_else(|error| panic!("load compact state: {error:?}"));
		state
			.set_values(&grown_values)
			.unwrap_or_else(|error| panic!("grow values: {error:?}"));
		state
			.commit()
			.unwrap_or_else(|error| panic!("commit grown state: {error:?}"))
	};

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
	let encoded_size = {
		let mut state = DynamicState::initialize(&mut data)
			.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));
		state
			.set_values(&values)
			.unwrap_or_else(|error| panic!("set compact values: {error:?}"));
		state
			.set_codes(&codes)
			.unwrap_or_else(|error| panic!("set compact codes: {error:?}"));
		state
			.commit()
			.unwrap_or_else(|error| panic!("commit compact state: {error:?}"))
	};

	let state = DynamicState::try_from_bytes(&data[..encoded_size])
		.unwrap_or_else(|error| panic!("read compact state: {error:?}"));
	assert_eq!(state.codes(), &codes);
}

#[test]
fn compact_account_validation_rejects_every_invalid_boundary() {
	let mut too_small = [0u8; DynamicState::HEADER_SIZE - 1];
	let mut too_large = [0u8; DynamicState::MAX_SIZE + 1];
	assert!(DynamicState::initialize(&mut too_small).is_err());
	assert!(DynamicState::initialize(&mut too_large).is_err());

	let mut split_alignment = [0u8; DynamicState::HEADER_SIZE + 1];
	assert!(DynamicState::initialize(&mut split_alignment).is_err());
	let mut spare_capacity = [0u8; DynamicState::HEADER_SIZE + 2];
	assert!(DynamicState::initialize(&mut spare_capacity).is_ok());

	let mut data = [0u8; DynamicState::HEADER_SIZE];
	DynamicState::initialize(&mut data)
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
	let mut state = DynamicState::initialize(&mut data)
		.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));

	assert!(state.assert(|header| header.bump == 0).is_ok());
	assert!(state.assert_mut(|header| header.bump == 1).is_err());
}

#[test]
fn account_view_loaders_scope_compact_borrows() {
	let mut stored = TestAccount::<{ DynamicState::MAX_SIZE }>::new();
	let mut account = stored.view();
	{
		let mut data = account
			.try_borrow_mut()
			.unwrap_or_else(|error| panic!("borrow compact data: {error:?}"));
		DynamicState::initialize(&mut data)
			.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));
	}

	account
		.with_compact_account_mut::<DynamicState, _>(&OWNER, |state| {
			state.bump = 8;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("mutate compact account: {error:?}"));
	let bump = account
		.with_compact_account::<DynamicState, _>(&OWNER, |state| Ok(state.bump))
		.unwrap_or_else(|error| panic!("read compact account: {error:?}"));

	assert_eq!(bump, 8);
	assert!(account.assert_compact_type::<DynamicState>(&OWNER).is_ok());
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
