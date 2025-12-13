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
use pinocchio::account_info::MAX_PERMITTED_DATA_INCREASE;

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
struct TestAccounts<'a> {
	pub one: &'a AccountInfo,
	pub two: &'a AccountInfo,
}

#[derive(Accounts)]
#[pina(crate = pina)]
struct TestAccountsRemaining<'a> {
	pub one: &'a AccountInfo,
	#[pina(remaining)]
	pub remaining: &'a [AccountInfo],
}

#[test]
fn test_accounts_derive_exact() {
	let ix_data = [3u8; 100];

	// Input with 2 accounts.

	let mut input = unsafe { create_input(2, &ix_data) };
	let mut accounts = [UNINIT; 2];

	let count = unsafe { deserialize(input.as_mut_ptr(), &mut accounts) }.1;
	let accounts: &[AccountInfo] =
		unsafe { core::slice::from_raw_parts(accounts.as_ptr().cast(), count) };

	let test_accounts = TestAccounts::try_from_account_infos(accounts).unwrap();
	assert_eq!(test_accounts.one, &accounts[0]);
	assert_eq!(test_accounts.two, &accounts[1]);
}

#[test]
fn test_accounts_derive_exact_not_enough() {
	let ix_data = [3u8; 100];

	// Input with 1 account

	let mut input = unsafe { create_input(1, &ix_data) };
	let mut accounts = [UNINIT; 1];

	let count = unsafe { deserialize(input.as_mut_ptr(), &mut accounts) }.1;
	let not_enough_accounts: &[AccountInfo] =
		unsafe { core::slice::from_raw_parts(accounts.as_ptr().cast(), count) };

	let result = TestAccounts::try_from_account_infos(not_enough_accounts);
	assert!(matches!(result, Err(ProgramError::NotEnoughAccountKeys)));
}

#[test]
fn test_accounts_derive_exact_excess() {
	let ix_data = [3u8; 100];

	// Input with 4 accounts

	let mut input = unsafe { create_input(4, &ix_data) };
	let mut accounts = [UNINIT; 4];

	let count = unsafe { deserialize(input.as_mut_ptr(), &mut accounts) }.1;
	let too_many_accounts: &[AccountInfo] =
		unsafe { core::slice::from_raw_parts(accounts.as_ptr().cast(), count) };

	let result = TestAccounts::try_from_account_infos(too_many_accounts);
	// let expected_err: ProgramError = PinaProgramError::TooManyAccountKeys.into();
	// assert!(matches!(result, Err(expected_err)));
	assert!(result.is_err_and(|error| error.eq(&PinaProgramError::TooManyAccountKeys.into())));
}

#[test]
fn test_accounts_derive_remaining_excess() {
	// Input with 20 accounts.
	let ix_data = [3u8; 100];
	let mut input = unsafe { create_input(20, &ix_data) };
	let mut accounts = [UNINIT; 20];

	let count = unsafe { deserialize(input.as_mut_ptr(), &mut accounts) }.1;
	let accounts: &[AccountInfo] =
		unsafe { core::slice::from_raw_parts(accounts.as_ptr().cast(), count) };

	let test_accounts = TestAccountsRemaining::try_from_account_infos(accounts).unwrap();
	assert_eq!(test_accounts.one, &accounts[0]);
	assert_eq!(test_accounts.remaining.len(), 19);
}

#[test]
fn test_accounts_derive_remaining_exact() {
	// Input with 1 accounts.
	let ix_data = [3u8; 100];
	let mut input = unsafe { create_input(1, &ix_data) };
	let mut accounts = [UNINIT; 1];

	let count = unsafe { deserialize(input.as_mut_ptr(), &mut accounts) }.1;
	let accounts: &[AccountInfo] =
		unsafe { core::slice::from_raw_parts(accounts.as_ptr().cast(), count) };

	let test_accounts = TestAccountsRemaining::try_from_account_infos(accounts).unwrap();
	assert_eq!(test_accounts.one, &accounts[0]);
	assert_eq!(test_accounts.remaining.len(), 0);
}

/// The mock program ID used for testing.
const MOCK_PROGRAM_ID: Pubkey = [5u8; 32];
/// `assert_eq(core::mem::align_of::<u128>(), 8)` is true for BPF but not
/// for some host machines.
const BPF_ALIGN_OF_U128: usize = 8;
/// An uninitialized account info.
const UNINIT: MaybeUninit<AccountInfo> = MaybeUninit::<AccountInfo>::uninit();
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
	///
	/// # Safety
	///
	/// The caller must ensure that the `data` length does not exceed the
	/// remaining space in the memory region starting from the `offset`.
	pub unsafe fn write(&mut self, data: &[u8], offset: usize) {
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
///
/// # Safety
///
/// The returned `AlignedMemory` should only be used within the test context.
unsafe fn create_input(accounts: usize, instruction_data: &[u8]) -> AlignedMemory {
	let mut input = AlignedMemory::new(1_000_000_000);
	// Number of accounts.
	unsafe {
		input.write(&(accounts as u64).to_le_bytes(), 0);
	}
	let mut offset = size_of::<u64>();

	for i in 0..accounts {
		// Account data.
		let mut account = [0u8; STATIC_ACCOUNT_DATA + size_of::<u64>()];
		account[0] = NON_DUP_MARKER;
		// Set the accounts data length. The actual account data is zeroed.
		account[80..88].copy_from_slice(&i.to_le_bytes());
		unsafe {
			input.write(&account, offset);
		}
		offset += account.len();
		// Padding for the account data to align to `BPF_ALIGN_OF_U128`.
		let padding_for_data = (i + (BPF_ALIGN_OF_U128 - 1)) & !(BPF_ALIGN_OF_U128 - 1);
		unsafe {
			input.write(&vec![0u8; padding_for_data], offset);
		}
		offset += padding_for_data;
	}

	// Instruction data length.
	unsafe {
		input.write(&instruction_data.len().to_le_bytes(), offset);
	}
	offset += size_of::<u64>();
	// Instruction data.
	unsafe {
		input.write(instruction_data, offset);
	}
	offset += instruction_data.len();
	// Program ID (mock).
	unsafe {
		input.write(&MOCK_PROGRAM_ID, offset);
	}

	input
}
