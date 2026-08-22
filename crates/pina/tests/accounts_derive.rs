#![allow(unsafe_code)]

use core::alloc::Layout;
use core::ptr::copy_nonoverlapping;
use std::alloc::alloc;
use std::alloc::dealloc;
use std::mem::MaybeUninit;
use std::vec;

use pina::entrypoint::NON_DUP_MARKER;
use pina::entrypoint::deserialize;
use pina::*;
use pinocchio::account::MAX_PERMITTED_DATA_INCREASE;

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
struct TestAccounts<'a> {
	pub one: &'a AccountView,
	pub two: &'a AccountView,
}

#[derive(Accounts)]
#[pina(crate = pina)]
struct TestAccountsRemaining<'a> {
	pub one: &'a AccountView,
	#[pina(remaining)]
	pub remaining: &'a [AccountView],
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
struct TestAccountsMut<'a> {
	pub one: &'a mut AccountView,
	pub two: &'a mut AccountView,
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
struct NestedAccounts<'a> {
	pub two: &'a AccountView,
	pub three: &'a mut AccountView,
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
struct ParentAccounts<'a> {
	pub one: &'a AccountView,
	pub nested: NestedAccounts<'a>,
}

#[derive(Accounts)]
#[pina(crate = pina)]
struct TestAccountsRemainingMut<'a> {
	pub one: &'a mut AccountView,
	#[pina(remaining)]
	pub remaining: &'a mut [AccountView],
}

#[derive(Accounts)]
#[pina(crate = pina)]
struct TestAccountsRemainingMutDistinct<'a> {
	pub one: &'a AccountView,
	#[pina(remaining, distinct)]
	pub remaining: &'a mut [AccountView],
}

#[test]
fn test_accounts_derive_exact() {
	let ix_data = [3u8; 100];

	// Input with 2 accounts.
	let mut input = create_input(2, &ix_data);
	let mut accounts = [UNINIT; 2];

	let count = unsafe { deserialize(input.as_mut_ptr(), &mut accounts) }.1;
	let accounts: &mut [AccountView] =
		unsafe { core::slice::from_raw_parts_mut(accounts.as_mut_ptr().cast(), count) };
	let one_ptr = core::ptr::addr_of!(accounts[0]);
	let two_ptr = core::ptr::addr_of!(accounts[1]);

	let test_accounts = TestAccounts::try_from_account_infos(&MOCK_PROGRAM_ID, accounts).unwrap();
	assert_eq!(test_accounts.one as *const AccountView, one_ptr);
	assert_eq!(test_accounts.two as *const AccountView, two_ptr);
}

#[test]
fn test_accounts_derive_exact_not_enough() {
	let ix_data = [3u8; 100];

	// Input with 1 account
	let mut input = create_input(1, &ix_data);
	let mut accounts = [UNINIT; 1];

	let count = unsafe { deserialize(input.as_mut_ptr(), &mut accounts) }.1;
	let not_enough_accounts: &mut [AccountView] =
		unsafe { core::slice::from_raw_parts_mut(accounts.as_mut_ptr().cast(), count) };

	let result = TestAccounts::try_from_account_infos(&MOCK_PROGRAM_ID, not_enough_accounts);
	assert!(matches!(result, Err(ProgramError::NotEnoughAccountKeys)));
}

#[test]
fn test_accounts_derive_exact_excess() {
	let ix_data = [3u8; 100];

	// Input with 4 accounts
	let mut input = create_input(4, &ix_data);
	let mut accounts = [UNINIT; 4];

	let count = unsafe { deserialize(input.as_mut_ptr(), &mut accounts) }.1;
	let too_many_accounts: &mut [AccountView] =
		unsafe { core::slice::from_raw_parts_mut(accounts.as_mut_ptr().cast(), count) };

	let result = TestAccounts::try_from_account_infos(&MOCK_PROGRAM_ID, too_many_accounts);
	assert!(result.is_err_and(|error| error.eq(&PinaProgramError::TooManyAccountKeys.into())));
}

#[test]
fn test_accounts_derive_remaining_excess() {
	// Input with 20 accounts.
	let ix_data = [3u8; 100];
	let mut input = create_input(20, &ix_data);
	let mut accounts = [UNINIT; 20];

	let count = unsafe { deserialize(input.as_mut_ptr(), &mut accounts) }.1;
	let accounts: &mut [AccountView] =
		unsafe { core::slice::from_raw_parts_mut(accounts.as_mut_ptr().cast(), count) };
	let one_ptr = core::ptr::addr_of!(accounts[0]);

	let test_accounts =
		TestAccountsRemaining::try_from_account_infos(&MOCK_PROGRAM_ID, accounts).unwrap();
	assert_eq!(test_accounts.one as *const AccountView, one_ptr);
	assert_eq!(test_accounts.remaining.len(), 19);
}

#[test]
fn test_accounts_derive_immutable_remaining_accepts_readonly_accounts() {
	let ix_data = [3u8; 100];
	let mut input = create_input_with_writability(3, &ix_data, |index| index == 0);
	let mut accounts = [UNINIT; 3];

	let count = unsafe { deserialize(input.as_mut_ptr(), &mut accounts) }.1;
	let accounts: &mut [AccountView] =
		unsafe { core::slice::from_raw_parts_mut(accounts.as_mut_ptr().cast(), count) };

	let test_accounts =
		TestAccountsRemaining::try_from_account_infos(&MOCK_PROGRAM_ID, accounts).unwrap();
	assert_eq!(test_accounts.remaining.len(), 2);
	assert!(
		test_accounts
			.remaining
			.iter()
			.all(|account| !account.is_writable())
	);
}

#[test]
fn test_accounts_derive_remaining_exact() {
	// Input with 1 accounts.
	let ix_data = [3u8; 100];
	let mut input = create_input(1, &ix_data);
	let mut accounts = [UNINIT; 1];

	let count = unsafe { deserialize(input.as_mut_ptr(), &mut accounts) }.1;
	let accounts: &mut [AccountView] =
		unsafe { core::slice::from_raw_parts_mut(accounts.as_mut_ptr().cast(), count) };
	let one_ptr = core::ptr::addr_of!(accounts[0]);

	let test_accounts =
		TestAccountsRemaining::try_from_account_infos(&MOCK_PROGRAM_ID, accounts).unwrap();
	assert_eq!(test_accounts.one as *const AccountView, one_ptr);
	assert_eq!(test_accounts.remaining.len(), 0);
}

#[test]
fn test_accounts_derive_exact_mutable() {
	let ix_data = [3u8; 100];
	let mut input = create_input(2, &ix_data);
	let mut accounts = [UNINIT; 2];

	let count = unsafe { deserialize(input.as_mut_ptr(), &mut accounts) }.1;
	let accounts: &mut [AccountView] =
		unsafe { core::slice::from_raw_parts_mut(accounts.as_mut_ptr().cast(), count) };
	let one_ptr = core::ptr::addr_of_mut!(accounts[0]);
	let two_ptr = core::ptr::addr_of_mut!(accounts[1]);

	let test_accounts =
		TestAccountsMut::try_from_account_infos(&MOCK_PROGRAM_ID, accounts).unwrap();
	assert_eq!(test_accounts.one as *mut AccountView, one_ptr);
	assert_eq!(test_accounts.two as *mut AccountView, two_ptr);
}

#[test]
fn test_accounts_derive_remaining_mutable() {
	let ix_data = [3u8; 100];
	let mut input = create_input(4, &ix_data);
	let mut accounts = [UNINIT; 4];

	let count = unsafe { deserialize(input.as_mut_ptr(), &mut accounts) }.1;
	let accounts: &mut [AccountView] =
		unsafe { core::slice::from_raw_parts_mut(accounts.as_mut_ptr().cast(), count) };
	let one_ptr = core::ptr::addr_of_mut!(accounts[0]);

	let test_accounts =
		TestAccountsRemainingMut::try_from_account_infos(&MOCK_PROGRAM_ID, accounts).unwrap();
	assert_eq!(test_accounts.one as *mut AccountView, one_ptr);
	assert_eq!(test_accounts.remaining.len(), 3);
}

#[test]
fn test_accounts_derive_mutable_remaining_rejects_readonly_accounts() {
	let ix_data = [3u8; 100];
	let mut input = create_input_with_writability(3, &ix_data, |index| index != 2);
	let mut accounts = [UNINIT; 3];

	let count = unsafe { deserialize(input.as_mut_ptr(), &mut accounts) }.1;
	let accounts: &mut [AccountView] =
		unsafe { core::slice::from_raw_parts_mut(accounts.as_mut_ptr().cast(), count) };

	let result = TestAccountsRemainingMut::try_from_account_infos(&MOCK_PROGRAM_ID, accounts);
	assert!(matches!(result, Err(ProgramError::InvalidAccountData)));
}

#[test]
fn test_accounts_derive_distinct_mutable_remaining_accepts_unique_accounts() {
	let ix_data = [3u8; 100];
	let mut input = create_input(4, &ix_data);
	let mut accounts = [UNINIT; 4];

	let count = unsafe { deserialize(input.as_mut_ptr(), &mut accounts) }.1;
	let accounts: &mut [AccountView] =
		unsafe { core::slice::from_raw_parts_mut(accounts.as_mut_ptr().cast(), count) };

	let parsed =
		TestAccountsRemainingMutDistinct::try_from_account_infos(&MOCK_PROGRAM_ID, accounts)
			.unwrap();
	assert_ne!(parsed.one.address(), parsed.remaining[0].address());
	assert_eq!(parsed.remaining.len(), 3);
}

#[test]
fn test_accounts_derive_distinct_mutable_remaining_rejects_readonly_accounts() {
	let ix_data = [3u8; 100];
	let mut input = create_input_with_writability(3, &ix_data, |index| index != 2);
	let mut accounts = [UNINIT; 3];

	let count = unsafe { deserialize(input.as_mut_ptr(), &mut accounts) }.1;
	let accounts: &mut [AccountView] =
		unsafe { core::slice::from_raw_parts_mut(accounts.as_mut_ptr().cast(), count) };

	let result =
		TestAccountsRemainingMutDistinct::try_from_account_infos(&MOCK_PROGRAM_ID, accounts);
	assert!(matches!(result, Err(ProgramError::InvalidAccountData)));
}

#[test]
fn test_accounts_derive_nested_loader_order() {
	let ix_data = [3u8; 100];
	let mut input = create_input(3, &ix_data);
	let mut accounts = [UNINIT; 3];

	let count = unsafe { deserialize(input.as_mut_ptr(), &mut accounts) }.1;
	let accounts: &mut [AccountView] =
		unsafe { core::slice::from_raw_parts_mut(accounts.as_mut_ptr().cast(), count) };
	let one_ptr = core::ptr::addr_of!(accounts[0]);
	let two_ptr = core::ptr::addr_of!(accounts[1]);
	let three_ptr = core::ptr::addr_of_mut!(accounts[2]);

	let test_accounts = ParentAccounts::try_from_account_infos(&MOCK_PROGRAM_ID, accounts).unwrap();
	assert_eq!(test_accounts.one as *const AccountView, one_ptr);
	assert_eq!(test_accounts.nested.two as *const AccountView, two_ptr);
	assert_eq!(test_accounts.nested.three as *mut AccountView, three_ptr);
}

/// The mock program ID used for testing.
const MOCK_PROGRAM_ID: Address = Address::new_from_array([5u8; 32]);
/// `assert_eq(core::mem::align_of::<u128>(), 8)` is true for BPF but not
/// for some host machines.
const BPF_ALIGN_OF_U128: usize = 8;
/// An uninitialized account view.
const UNINIT: MaybeUninit<AccountView> = MaybeUninit::<AccountView>::uninit();
/// The "static" size of an account in the input buffer.
///
/// This is the size of the account header plus the maximum permitted data
/// increase.
const STATIC_ACCOUNT_DATA: usize = 88 + MAX_PERMITTED_DATA_INCREASE;

/// Struct representing a memory region with a specific alignment.
struct AlignedMemory {
	ptr: *mut u8,
	layout: Layout,
}

impl AlignedMemory {
	pub fn new(len: usize) -> Self {
		let layout = Layout::from_size_align(len, BPF_ALIGN_OF_U128).unwrap();
		// SAFETY: `align` is set to `BPF_ALIGN_OF_U128`.
		unsafe {
			let ptr = alloc(layout);
			if ptr.is_null() {
				std::alloc::handle_alloc_error(layout);
			}
			AlignedMemory { ptr, layout }
		}
	}

	/// Write data to the memory region at the specified offset.
	pub fn write(&mut self, data: &[u8], offset: usize) {
		let end = offset
			.checked_add(data.len())
			.expect("input offset overflow");
		assert!(end <= self.layout.size(), "write exceeds input allocation");

		// SAFETY: the bounds check above keeps the write within the allocation,
		// and source and destination cannot overlap.
		unsafe {
			copy_nonoverlapping(data.as_ptr(), self.ptr.add(offset), data.len());
		}
	}

	/// Return a mutable pointer to the memory region.
	pub fn as_mut_ptr(&mut self) -> *mut u8 {
		self.ptr
	}
}

impl Drop for AlignedMemory {
	fn drop(&mut self) {
		unsafe {
			dealloc(self.ptr, self.layout);
		}
	}
}

/// Creates an input buffer with a specified number of accounts and instruction
/// data.
///
/// This function mimics the input buffer created by the SVM loader.  Each
/// account created has zeroed data, apart from the `data_len` field, which is
/// set to the index of the account.
fn create_input(accounts: usize, instruction_data: &[u8]) -> AlignedMemory {
	create_input_with_writability(accounts, instruction_data, |_| true)
}

/// Creates an input buffer with per-account writable flags.
fn create_input_with_writability(
	accounts: usize,
	instruction_data: &[u8],
	is_writable: impl Fn(usize) -> bool,
) -> AlignedMemory {
	create_input_with_layout(
		accounts,
		instruction_data,
		is_writable,
		|_| false,
		|i| {
			let mut bytes = [0u8; 32];
			// Preserve the historical unique-first-byte addressing used by the
			// duplicate-mutable-account tests.
			bytes[0] = (i + 1) as u8;
			Address::new_from_array(bytes)
		},
	)
}

/// Creates an input buffer with per-account writable, signer, and address
/// configuration.
///
/// This function mimics the input buffer created by the SVM loader. Each
/// account has zeroed data apart from the `data_len` field, which is set to
/// the index of the account.
fn create_input_with_layout(
	accounts: usize,
	instruction_data: &[u8],
	is_writable: impl Fn(usize) -> bool,
	is_signer: impl Fn(usize) -> bool,
	key: impl Fn(usize) -> Address,
) -> AlignedMemory {
	let mut input = AlignedMemory::new(1_000_000_000);
	// Number of accounts.
	input.write(&(accounts as u64).to_le_bytes(), 0);
	let mut offset = size_of::<u64>();

	for i in 0..accounts {
		// Account data.
		let mut account = [0u8; STATIC_ACCOUNT_DATA + size_of::<u64>()];
		account[0] = NON_DUP_MARKER;
		account[2] = u8::from(is_writable(i));
		account[3] = u8::from(is_signer(i));
		// Give each account its configured address so alias checks and
		// optional-slot sentinels behave deterministically.
		account[8..40].copy_from_slice(key(i).as_ref());
		// Set the accounts data length. The actual account data is zeroed.
		account[80..88].copy_from_slice(&i.to_le_bytes());
		input.write(&account, offset);
		offset += account.len();
		// Padding for the account data to align to `BPF_ALIGN_OF_U128`.
		let padding_for_data = (i + (BPF_ALIGN_OF_U128 - 1)) & !(BPF_ALIGN_OF_U128 - 1);
		input.write(&vec![0u8; padding_for_data], offset);
		offset += padding_for_data;
	}

	// Instruction data length.
	input.write(&instruction_data.len().to_le_bytes(), offset);
	offset += size_of::<u64>();
	// Instruction data.
	input.write(instruction_data, offset);
	offset += instruction_data.len();
	// Program ID (mock).
	input.write(MOCK_PROGRAM_ID.as_ref(), offset);

	input
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
struct TestAccountsOptional<'a> {
	pub one: &'a AccountView,
	/// An optional immutable account slot.
	pub optional: Option<&'a AccountView>,
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
struct TestAccountsOptionalMut<'a> {
	pub one: &'a mut AccountView,
	/// An optional writable account slot.
	pub optional: Option<&'a mut AccountView>,
}

#[derive(Accounts)]
#[pina(crate = pina)]
#[allow(dead_code)]
struct TestAccountsOptionalLeadingMut<'a> {
	pub optional: Option<&'a mut AccountView>,
	pub one: &'a mut AccountView,
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
struct TestAccountsOptionalThenImmutable<'a> {
	pub optional: Option<&'a mut AccountView>,
	pub one: &'a AccountView,
}

/// Builds an address whose first byte is `byte`, keeping test keys unique.
fn key_from_byte(byte: u8) -> Address {
	let mut bytes = [0u8; 32];
	bytes[0] = byte;
	Address::new_from_array(bytes)
}

/// Runs the entrypoint deserializer over an input buffer and returns the
/// account slice it points at.
///
/// # Safety
///
/// `input` must be a valid SVM-style entrypoint buffer and `capacity` must be
/// at least the account count encoded in the buffer.
unsafe fn slice_input<'a, const CAP: usize>(
	input: &'a mut AlignedMemory,
	accounts: &'a mut [MaybeUninit<AccountView>; CAP],
) -> &'a mut [AccountView] {
	let count = unsafe { deserialize(input.as_mut_ptr(), accounts) }.1;
	unsafe { core::slice::from_raw_parts_mut(accounts.as_mut_ptr().cast(), count) }
}

fn unique_keys(index: usize) -> Address {
	key_from_byte((index + 1) as u8)
}

#[test]
fn test_accounts_derive_optional_immutable_present() {
	let ix_data = [3u8; 100];
	let mut input = create_input_with_layout(2, &ix_data, |_| false, |_| false, unique_keys);
	let mut accounts = [UNINIT; 2];
	// SAFETY: the buffer encodes exactly two accounts.
	let accounts = unsafe { slice_input(&mut input, &mut accounts) };

	let test_accounts =
		TestAccountsOptional::try_from_account_infos(&MOCK_PROGRAM_ID, accounts).unwrap();
	assert_eq!(test_accounts.one.address(), &key_from_byte(1));
	assert_eq!(
		test_accounts
			.optional
			.expect("optional slot must be present")
			.address(),
		&key_from_byte(2)
	);
}

#[test]
fn test_accounts_derive_optional_immutable_absent() {
	let ix_data = [3u8; 100];
	let mut input = create_input_with_layout(
		2,
		&ix_data,
		|_| false,
		|_| false,
		|i| {
			if i == 1 {
				MOCK_PROGRAM_ID
			} else {
				unique_keys(i)
			}
		},
	);
	let mut accounts = [UNINIT; 2];
	// SAFETY: the buffer encodes exactly two accounts.
	let accounts = unsafe { slice_input(&mut input, &mut accounts) };

	let test_accounts =
		TestAccountsOptional::try_from_account_infos(&MOCK_PROGRAM_ID, accounts).unwrap();
	assert_eq!(test_accounts.one.address(), &key_from_byte(1));
	assert!(test_accounts.optional.is_none());
}

#[test]
fn test_accounts_derive_optional_immutable_not_enough_keys() {
	let ix_data = [3u8; 100];
	let mut input = create_input_with_layout(1, &ix_data, |_| false, |_| false, unique_keys);
	let mut accounts = [UNINIT; 1];
	// SAFETY: the buffer encodes exactly one account.
	let accounts = unsafe { slice_input(&mut input, &mut accounts) };

	let result = TestAccountsOptional::try_from_account_infos(&MOCK_PROGRAM_ID, accounts);
	assert!(matches!(result, Err(ProgramError::NotEnoughAccountKeys)));
}

#[test]
fn test_accounts_derive_try_from_tuple() {
	let ix_data = [3u8; 100];
	let mut input = create_input_with_layout(2, &ix_data, |_| false, |_| false, unique_keys);
	let mut accounts = [UNINIT; 2];
	// SAFETY: the buffer encodes exactly two accounts.
	let accounts = unsafe { slice_input(&mut input, &mut accounts) };

	let test_accounts = TestAccountsOptional::try_from((&MOCK_PROGRAM_ID, accounts)).unwrap();
	assert!(test_accounts.optional.is_some());
}

#[test]
fn test_accounts_derive_optional_mutable_present() {
	let ix_data = [3u8; 100];
	let mut input = create_input_with_layout(2, &ix_data, |_| true, |_| false, unique_keys);
	let mut accounts = [UNINIT; 2];
	// SAFETY: the buffer encodes exactly two accounts.
	let accounts = unsafe { slice_input(&mut input, &mut accounts) };
	let optional_ptr = core::ptr::addr_of_mut!(accounts[1]);

	let test_accounts =
		TestAccountsOptionalMut::try_from_account_infos(&MOCK_PROGRAM_ID, accounts).unwrap();
	assert_eq!(test_accounts.one.address(), &key_from_byte(1));
	assert_eq!(
		test_accounts
			.optional
			.expect("optional slot must be present") as *mut AccountView,
		optional_ptr
	);
}

#[test]
fn test_accounts_derive_optional_mutable_absent() {
	let ix_data = [3u8; 100];
	let mut input = create_input_with_layout(
		2,
		&ix_data,
		|i| i == 0,
		|_| false,
		|i| {
			if i == 1 {
				MOCK_PROGRAM_ID
			} else {
				unique_keys(i)
			}
		},
	);
	let mut accounts = [UNINIT; 2];
	// SAFETY: the buffer encodes exactly two accounts.
	let accounts = unsafe { slice_input(&mut input, &mut accounts) };

	let test_accounts =
		TestAccountsOptionalMut::try_from_account_infos(&MOCK_PROGRAM_ID, accounts).unwrap();
	assert_eq!(test_accounts.one.address(), &key_from_byte(1));
	assert!(test_accounts.optional.is_none());
}

/// A writable filler in an absent slot is still parsed as `None`. The runtime
/// rejects writable executable accounts before the program runs, so treating
/// address equality as authoritative keeps parsing deterministic.
#[test]
fn test_accounts_derive_optional_writable_filler_still_absent() {
	let ix_data = [3u8; 100];
	let mut input = create_input_with_layout(
		2,
		&ix_data,
		|_| true,
		|_| false,
		|i| {
			if i == 1 {
				MOCK_PROGRAM_ID
			} else {
				unique_keys(i)
			}
		},
	);
	let mut accounts = [UNINIT; 2];
	// SAFETY: the buffer encodes exactly two accounts.
	let accounts = unsafe { slice_input(&mut input, &mut accounts) };

	let test_accounts =
		TestAccountsOptionalMut::try_from_account_infos(&MOCK_PROGRAM_ID, accounts).unwrap();
	assert!(test_accounts.optional.is_none());
}

/// A present value for an optional mutable slot still requires writability.
#[test]
fn test_accounts_derive_optional_mutable_rejects_readonly_value() {
	let ix_data = [3u8; 100];
	let mut input = create_input_with_layout(2, &ix_data, |i| i == 0, |_| false, unique_keys);
	let mut accounts = [UNINIT; 2];
	// SAFETY: the buffer encodes exactly two accounts.
	let accounts = unsafe { slice_input(&mut input, &mut accounts) };

	let result = TestAccountsOptionalMut::try_from_account_infos(&MOCK_PROGRAM_ID, accounts);
	assert!(matches!(result, Err(ProgramError::InvalidAccountData)));
}

/// A present optional mutable account that aliases a later required writable
/// account is rejected by the duplicate-writable guard.
#[test]
fn test_accounts_derive_optional_mutable_rejects_duplicate_alias() {
	let ix_data = [3u8; 100];
	let shared_key = key_from_byte(9);
	let mut input = create_input_with_layout(2, &ix_data, |_| true, |_| false, |_| shared_key);
	let mut accounts = [UNINIT; 2];
	// SAFETY: the buffer encodes exactly two accounts.
	let accounts = unsafe { slice_input(&mut input, &mut accounts) };

	let result = TestAccountsOptionalLeadingMut::try_from_account_infos(&MOCK_PROGRAM_ID, accounts);
	assert!(result.is_err_and(|error| error.eq(&PinaProgramError::DuplicateMutableAccount.into())));
}

/// A duplicate alias is accepted when the second occurrence stays readonly,
/// matching the existing behaviour of required immutable fields.
#[test]
fn test_accounts_derive_optional_mutable_allows_readonly_duplicate() {
	let ix_data = [3u8; 100];
	let shared_key = key_from_byte(9);
	let mut input = create_input_with_layout(2, &ix_data, |i| i == 0, |_| false, |_| shared_key);
	let mut accounts = [UNINIT; 2];
	// SAFETY: the buffer encodes exactly two accounts.
	let accounts = unsafe { slice_input(&mut input, &mut accounts) };

	// The first slot is an optional mutable field; the second is readonly so
	// no writable alias exists and parsing succeeds.
	let result =
		TestAccountsOptionalThenImmutable::try_from_account_infos(&MOCK_PROGRAM_ID, accounts);
	assert!(result.is_ok());
	let parsed = result.unwrap();
	assert!(parsed.optional.is_some());
	assert_eq!(parsed.one.address(), &shared_key);
}
