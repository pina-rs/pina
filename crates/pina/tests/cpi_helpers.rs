#![allow(unsafe_code)]

use pina::Address;
use pina::CpiContext;
use pina::CpiHandle;
use pina::ProgramError;
use pina::ToCpiAccounts;
use pina::combine_seeds_with_bump;
#[cfg(feature = "account-resize")]
use pina::realloc_account;
#[cfg(feature = "account-resize")]
use pina::realloc_account_zero;
use pinocchio::AccountView;
use pinocchio::account::MAX_PERMITTED_DATA_INCREASE;
use pinocchio::account::NOT_BORROWED;
use pinocchio::account::RuntimeAccount;
use pinocchio::address::MAX_SEEDS;

#[test]
fn combine_seeds_with_bump_basic() {
	let seed_a: &[u8] = b"escrow";
	let seed_b: &[u8] = &[1, 2, 3];
	let bump = [42u8; 1];

	let result = combine_seeds_with_bump(&[seed_a, seed_b], &bump)
		.unwrap_or_else(|e| panic!("failed: {e:?}"));

	assert_eq!(&*result[0], b"escrow");
	assert_eq!(&*result[1], &[1, 2, 3]);
	assert_eq!(&*result[2], &[42]);
	for slot in &result[3..] {
		assert!(slot.is_empty());
	}
}

#[test]
fn combine_seeds_with_bump_single_seed() {
	let seed: &[u8] = b"hello";
	let bump = [0u8; 1];

	let result =
		combine_seeds_with_bump(&[seed], &bump).unwrap_or_else(|e| panic!("failed: {e:?}"));

	assert_eq!(&*result[0], b"hello");
	assert_eq!(&*result[1], &[0]);
	for slot in &result[2..] {
		assert!(slot.is_empty());
	}
}

#[test]
fn combine_seeds_with_bump_empty_seeds() {
	let bump = [255u8; 1];

	let result = combine_seeds_with_bump(&[], &bump).unwrap_or_else(|e| panic!("failed: {e:?}"));

	assert_eq!(&*result[0], &[255]);
	for slot in &result[1..] {
		assert!(slot.is_empty());
	}
}

#[test]
fn combine_seeds_with_bump_at_max_minus_one() {
	let seeds: Vec<&[u8]> = (0..MAX_SEEDS - 1).map(|_| &[1u8][..]).collect();
	let bump = [7u8; 1];

	let result = combine_seeds_with_bump(&seeds, &bump).unwrap_or_else(|e| panic!("failed: {e:?}"));

	for (i, slot) in result.iter().enumerate().take(MAX_SEEDS - 1) {
		assert_eq!(&**slot, &[1u8], "slot {i} should be the original seed");
	}
	assert_eq!(&*result[MAX_SEEDS - 1], &[7]);
}

#[test]
fn combine_seeds_with_bump_too_many_seeds_fails() {
	let seeds: Vec<&[u8]> = (0..MAX_SEEDS).map(|_| &[1u8][..]).collect();
	let bump = [7u8; 1];

	let result = combine_seeds_with_bump(&seeds, &bump);
	assert!(result.is_err());
}

#[cfg(feature = "account-resize")]
#[test]
fn max_permitted_data_increase_is_10_kib() {
	assert_eq!(MAX_PERMITTED_DATA_INCREASE, 10_240);
}

#[cfg(feature = "account-resize")]
#[test]
fn realloc_functions_are_exported() {
	let _grow: fn(
		&mut pinocchio::AccountView,
		usize,
		&mut pinocchio::AccountView,
		&pinocchio::Address,
	) -> pinocchio::ProgramResult = realloc_account;
	let _grow_zero: fn(
		&mut pinocchio::AccountView,
		usize,
		&mut pinocchio::AccountView,
		&pinocchio::Address,
	) -> pinocchio::ProgramResult = realloc_account_zero;
}

#[repr(C)]
struct TestAccount<const N: usize> {
	header: RuntimeAccount,
	data: [u8; N],
}

impl<const N: usize> TestAccount<N> {
	fn new(address: Address, is_signer: bool, is_writable: bool) -> Self {
		Self {
			header: RuntimeAccount {
				borrow_state: NOT_BORROWED,
				is_signer: u8::from(is_signer),
				is_writable: u8::from(is_writable),
				executable: 0,
				padding: [0; 4],
				address,
				owner: Address::new_from_array([9u8; 32]),
				lamports: 1,
				data_len: N as u64,
			},
			data: [0u8; N],
		}
	}

	fn view(&mut self) -> AccountView {
		unsafe { AccountView::new_unchecked(core::ptr::addr_of_mut!(self.header)) }
	}
}

#[derive(Clone, Copy)]
struct ExampleAccounts<'a> {
	first: CpiHandle<'a>,
	second: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a, 2> for ExampleAccounts<'a> {
	fn to_cpi_handles(&self) -> [CpiHandle<'a>; 2] {
		[self.first, self.second]
	}
}

#[test]
fn cpi_handle_preserves_writable_and_signer_flags() {
	let mut writable = TestAccount::<8>::new(Address::new_from_array([1u8; 32]), true, true);
	let mut readonly = TestAccount::<8>::new(Address::new_from_array([2u8; 32]), false, false);
	let writable_view = writable.view();
	let readonly_view = readonly.view();

	let writable_handle =
		CpiHandle::writable(&writable_view).unwrap_or_else(|e| panic!("writable handle: {e:?}"));
	let readonly_handle = CpiHandle::readonly(&readonly_view);

	assert!(writable_handle.is_writable());
	assert!(writable_handle.is_signer());
	assert!(!readonly_handle.is_writable());
	assert!(!readonly_handle.is_signer());
	assert_eq!(writable_handle.address(), writable_view.address());
	assert_eq!(readonly_handle.address(), readonly_view.address());
}

#[test]
fn cpi_handle_rejects_readonly_writable_requests() {
	let mut readonly = TestAccount::<8>::new(Address::new_from_array([3u8; 32]), false, false);
	let readonly_view = readonly.view();

	let result = CpiHandle::writable(&readonly_view);
	assert!(matches!(result, Err(ProgramError::InvalidAccountData)));
}

#[test]
fn cpi_context_accepts_typed_account_structs() {
	let mut first = TestAccount::<8>::new(Address::new_from_array([4u8; 32]), true, true);
	let mut second = TestAccount::<8>::new(Address::new_from_array([5u8; 32]), false, false);
	let first_view = first.view();
	let second_view = second.view();
	let accounts = ExampleAccounts {
		first: CpiHandle::writable(&first_view).unwrap_or_else(|e| panic!("first handle: {e:?}")),
		second: CpiHandle::readonly(&second_view),
	};
	let program = Address::new_from_array([6u8; 32]);
	let context = CpiContext::new(&program, accounts);
	let ordered = context.accounts.to_cpi_handles();

	assert_eq!(ordered[0].address(), first_view.address());
	assert!(ordered[0].is_writable());
	assert_eq!(ordered[1].address(), second_view.address());
	assert!(!ordered[1].is_writable());
}
