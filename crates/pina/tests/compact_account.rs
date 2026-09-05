#![allow(unsafe_code)]

use pina::*;
use pinocchio::account::NOT_BORROWED;
use pinocchio::account::RuntimeAccount;

const OWNER: Address = Address::new_from_array([9; 32]);

#[discriminator(crate = ::pina)]
enum CompactKind {
	DynamicState = 7,
}

#[account(crate = ::pina, discriminator = CompactKind, compact)]
struct DynamicState {
	pub bump: u8,
	pub authority: Address,
	pub values: Vec<u64, 4>,
}

#[test]
fn compact_account_roundtrips_active_tail_without_fixed_capacity_padding() {
	assert_eq!(DynamicState::HEADER_SIZE, 36);
	assert_eq!(DynamicState::MAX_SIZE, 68);
	assert_eq!(DynamicState::TAIL_ELEMENT_SIZE, 8);

	let mut data = [0u8; DynamicState::MAX_SIZE];
	let values = [PodU64::from(11), PodU64::from(22)];
	let encoded_size = {
		let mut state = DynamicState::initialize(&mut data)
			.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));
		state.bump = 3;
		state.authority = Address::new_from_array([5; 32]);
		state
			.set_values(&values)
			.unwrap_or_else(|error| panic!("set compact values: {error:?}"));
		state
			.commit()
			.unwrap_or_else(|error| panic!("commit compact state: {error:?}"))
	};

	assert_eq!(encoded_size, DynamicState::HEADER_SIZE + 16);
	let state = DynamicState::try_from_bytes(&data[..encoded_size])
		.unwrap_or_else(|error| panic!("read compact state: {error:?}"));
	assert_eq!(state.bump, 3);
	assert_eq!(state.authority, Address::new_from_array([5; 32]));
	assert_eq!(state.values()[0].get(), 11);
	assert_eq!(state.values()[1].get(), 22);
}

#[test]
fn compact_account_validation_rejects_every_invalid_boundary() {
	let mut too_small = [0u8; DynamicState::HEADER_SIZE - 1];
	let mut too_large = [0u8; DynamicState::MAX_SIZE + 1];
	let mut split_element = [0u8; DynamicState::HEADER_SIZE + 1];
	assert!(DynamicState::initialize(&mut too_small).is_err());
	assert!(DynamicState::initialize(&mut too_large).is_err());
	assert!(DynamicState::initialize(&mut split_element).is_err());

	let mut data = [0u8; DynamicState::HEADER_SIZE];
	DynamicState::initialize(&mut data)
		.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));
	data[0] = 99;
	assert!(DynamicState::try_from_bytes(&data).is_err());

	data[0] = CompactKind::DynamicState as u8;
	data[34] = 5;
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
