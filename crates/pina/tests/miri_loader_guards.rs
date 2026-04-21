#![cfg(miri)]
#![allow(unsafe_code)]

use core::mem::MaybeUninit;
use core::mem::size_of;
use core::ptr::copy_nonoverlapping;
use std::alloc::Layout;
use std::alloc::alloc;
use std::alloc::dealloc;
use std::vec;
use std::vec::Vec;

use bytemuck::Pod;
use bytemuck::Zeroable;
use pina::*;
use pinocchio::account::MAX_PERMITTED_DATA_INCREASE;
use pinocchio::entrypoint;

const TEST_PROGRAM_ID: Address = address!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");
const BPF_ALIGN_OF_U128: usize = 8;
const UNINIT: MaybeUninit<AccountView> = MaybeUninit::<AccountView>::uninit();
const STATIC_ACCOUNT_DATA: usize = 88 + MAX_PERMITTED_DATA_INCREASE;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct TestState {
	discriminator: [u8; 1],
	value: PodU64,
}

impl HasDiscriminator for TestState {
	type Type = u8;

	const VALUE: u8 = 7;
}

struct AccountBuilder {
	address: Address,
	owner: Address,
	lamports: u64,
	data: Vec<u8>,
	is_writable: bool,
}

impl AccountBuilder {
	fn new() -> Self {
		Self {
			address: Address::default(),
			owner: Address::default(),
			lamports: 0,
			data: Vec::new(),
			is_writable: false,
		}
	}

	fn address(mut self, address: Address) -> Self {
		self.address = address;
		self
	}

	fn owner(mut self, owner: Address) -> Self {
		self.owner = owner;
		self
	}

	fn lamports(mut self, lamports: u64) -> Self {
		self.lamports = lamports;
		self
	}

	fn data(mut self, data: &[u8]) -> Self {
		self.data = data.to_vec();
		self
	}

	fn is_writable(mut self, is_writable: bool) -> Self {
		self.is_writable = is_writable;
		self
	}
}

struct AlignedMemory {
	ptr: *mut u8,
	layout: Layout,
}

impl AlignedMemory {
	fn new(len: usize) -> Self {
		let layout = Layout::from_size_align(len, BPF_ALIGN_OF_U128)
			.unwrap_or_else(|e| panic!("invalid layout: {e:?}"));
		unsafe {
			let ptr = alloc(layout);
			if ptr.is_null() {
				std::alloc::handle_alloc_error(layout);
			}
			AlignedMemory { ptr, layout }
		}
	}

	unsafe fn write(&mut self, data: &[u8], offset: usize) {
		unsafe {
			copy_nonoverlapping(data.as_ptr(), self.ptr.add(offset), data.len());
		}
	}

	fn as_mut_ptr(&mut self) -> *mut u8 {
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

fn compute_input_size(accounts: &[AccountBuilder], instruction_data: &[u8]) -> usize {
	let mut size = size_of::<u64>();

	for builder in accounts {
		let data_len = builder.data.len();
		let account_buf_size = STATIC_ACCOUNT_DATA + size_of::<u64>();
		size += account_buf_size;
		let padding = (data_len + (BPF_ALIGN_OF_U128 - 1)) & !(BPF_ALIGN_OF_U128 - 1);
		size += padding;
	}

	size += size_of::<u64>();
	size += instruction_data.len();
	size += ADDRESS_BYTES;

	size
}

unsafe fn create_test_input(accounts: &[AccountBuilder], instruction_data: &[u8]) -> AlignedMemory {
	let total_size = compute_input_size(accounts, instruction_data);
	let mut input = AlignedMemory::new(total_size);

	unsafe {
		input.write(&(accounts.len() as u64).to_le_bytes(), 0);
	}
	let mut offset = size_of::<u64>();

	for builder in accounts {
		let data_len = builder.data.len();
		let account_buf_size = STATIC_ACCOUNT_DATA + size_of::<u64>();
		let mut account_buf = vec![0u8; account_buf_size];

		account_buf[0] = entrypoint::NON_DUP_MARKER;
		account_buf[2] = u8::from(builder.is_writable);
		account_buf[8..40].copy_from_slice(builder.address.as_ref());
		account_buf[40..72].copy_from_slice(builder.owner.as_ref());
		account_buf[72..80].copy_from_slice(&builder.lamports.to_le_bytes());
		account_buf[80..88].copy_from_slice(&(data_len as u64).to_le_bytes());
		if !builder.data.is_empty() {
			account_buf[88..88 + data_len].copy_from_slice(&builder.data);
		}

		unsafe {
			input.write(&account_buf, offset);
		}
		offset += account_buf_size;

		let padding = (data_len + (BPF_ALIGN_OF_U128 - 1)) & !(BPF_ALIGN_OF_U128 - 1);
		if padding > 0 {
			unsafe {
				input.write(&vec![0u8; padding], offset);
			}
			offset += padding;
		}
	}

	unsafe {
		input.write(&instruction_data.len().to_le_bytes(), offset);
	}
	offset += size_of::<u64>();

	unsafe {
		input.write(instruction_data, offset);
	}
	offset += instruction_data.len();

	unsafe {
		input.write(TEST_PROGRAM_ID.as_ref(), offset);
	}

	input
}

unsafe fn deserialize_test_input<const MAX_ACCOUNTS: usize>(
	input: &mut AlignedMemory,
	accounts: &mut [MaybeUninit<AccountView>; MAX_ACCOUNTS],
) -> (&'static [AccountView], &'static [u8]) {
	let (_program_id, count, ix_data) =
		unsafe { entrypoint::deserialize::<MAX_ACCOUNTS>(input.as_mut_ptr(), accounts) };
	let accounts: &[AccountView] =
		unsafe { core::slice::from_raw_parts(accounts.as_ptr().cast(), count) };
	(accounts, ix_data)
}

fn build_test_state_bytes(value: u64) -> Vec<u8> {
	let state = TestState {
		discriminator: [TestState::VALUE],
		value: PodU64::from_primitive(value),
	};

	bytemuck::bytes_of(&state).to_vec()
}

#[cfg(feature = "token")]
fn write_address_bytes(data: &mut [u8], offset: usize, address: &Address) {
	data[offset..offset + ADDRESS_BYTES].copy_from_slice(address.as_ref());
}

#[cfg(feature = "token")]
fn build_token_mint_bytes(decimals: u8, supply: u64) -> Vec<u8> {
	let mut data = vec![0u8; token::state::Mint::LEN];
	data[0] = 1;
	write_address_bytes(&mut data, 4, &system::ID);
	data[36..44].copy_from_slice(&supply.to_le_bytes());
	data[44] = decimals;
	data[45] = 1;
	data[46] = 1;
	write_address_bytes(&mut data, 50, &TEST_PROGRAM_ID);
	data
}

#[cfg(feature = "token")]
fn build_token_account_bytes(mint: &Address, owner: &Address, amount: u64) -> Vec<u8> {
	let mut data = vec![0u8; token::state::TokenAccount::LEN];
	write_address_bytes(&mut data, 0, mint);
	write_address_bytes(&mut data, 32, owner);
	data[64..72].copy_from_slice(&amount.to_le_bytes());
	data[104] = 1;
	data
}

#[test]
fn as_account_rejects_overlapping_mutable_borrows_under_miri() {
	let account_key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let state_bytes = build_test_state_bytes(77);

	let accounts = [AccountBuilder::new()
		.address(account_key)
		.owner(TEST_PROGRAM_ID)
		.lamports(1_000_000)
		.data(&state_bytes)
		.is_writable(true)];

	let mut input = unsafe { create_test_input(&accounts, &[]) };
	let mut accts = [UNINIT; 4];
	let (account_views, _) = unsafe { deserialize_test_input::<4>(&mut input, &mut accts) };

	let state = account_views[0]
		.as_account::<TestState>(&TEST_PROGRAM_ID)
		.unwrap_or_else(|e| panic!("typed load failed: {e:?}"));
	assert_eq!(u64::from(state.value), 77);

	assert!(matches!(
		account_views[0].try_borrow_mut(),
		Err(ProgramError::AccountBorrowFailed)
	));

	drop(state);

	assert!(account_views[0].try_borrow_mut().is_ok());
}

#[test]
fn as_account_mut_rejects_shared_and_mutable_reborrows_under_miri() {
	let account_key: Address = address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki");
	let state_bytes = build_test_state_bytes(11);

	let accounts = [AccountBuilder::new()
		.address(account_key)
		.owner(TEST_PROGRAM_ID)
		.lamports(1_000_000)
		.data(&state_bytes)
		.is_writable(true)];

	let mut input = unsafe { create_test_input(&accounts, &[]) };
	let mut accts = [UNINIT; 4];
	let (account_views, _) = unsafe { deserialize_test_input::<4>(&mut input, &mut accts) };

	let mut state = account_views[0]
		.as_account_mut::<TestState>(&TEST_PROGRAM_ID)
		.unwrap_or_else(|e| panic!("typed mutable load failed: {e:?}"));
	state.value = PodU64::from_primitive(99);

	assert!(matches!(
		account_views[0].try_borrow(),
		Err(ProgramError::AccountBorrowFailed)
	));
	assert!(matches!(
		account_views[0].try_borrow_mut(),
		Err(ProgramError::AccountBorrowFailed)
	));

	drop(state);

	let state = account_views[0]
		.as_account::<TestState>(&TEST_PROGRAM_ID)
		.unwrap_or_else(|e| panic!("typed reload failed: {e:?}"));
	assert_eq!(u64::from(state.value), 99);
}

#[cfg(feature = "token")]
#[test]
fn as_token_mint_rejects_overlapping_mutable_borrows_under_miri() {
	let mint_key: Address = address!("8qbHbw2BbbTHBW1sK7d7Yx4Z4DccnE9vrFica8FWHQrP");
	let mint_data = build_token_mint_bytes(6, 1_000);

	let accounts = [AccountBuilder::new()
		.address(mint_key)
		.owner(token::ID)
		.lamports(1_000_000)
		.data(&mint_data)
		.is_writable(true)];

	let mut input = unsafe { create_test_input(&accounts, &[]) };
	let mut accts = [UNINIT; 4];
	let (account_views, _) = unsafe { deserialize_test_input::<4>(&mut input, &mut accts) };

	let mint = account_views[0]
		.as_token_mint()
		.unwrap_or_else(|e| panic!("mint load failed: {e:?}"));
	assert_eq!(mint.decimals(), 6);
	assert_eq!(mint.supply(), 1_000);

	assert!(matches!(
		account_views[0].try_borrow_mut(),
		Err(ProgramError::AccountBorrowFailed)
	));

	drop(mint);

	assert!(account_views[0].try_borrow_mut().is_ok());
}

#[cfg(feature = "token")]
#[test]
fn as_token_account_checked_with_owners_supports_token_2022_under_miri() {
	let account_key: Address = address!("4vJ9JU1bJJE96FWSJKv9J5xBqHkM7SspGq2pZ7uS5k4x");
	let mint: Address = address!("CktRuQ2mttxyPjdvVSxGJySLjeRGna43E77gzHu6HotE");
	let owner: Address = address!("4Nd1mL5g7dUvNbKQjnYQgQki71RJKVQ1BM8DT6vKrrf5");
	let token_account_data = build_token_account_bytes(&mint, &owner, 55);

	let accounts = [AccountBuilder::new()
		.address(account_key)
		.owner(token_2022::ID)
		.lamports(1_000_000)
		.data(&token_account_data)
		.is_writable(true)];

	let mut input = unsafe { create_test_input(&accounts, &[]) };
	let mut accts = [UNINIT; 4];
	let (account_views, _) = unsafe { deserialize_test_input::<4>(&mut input, &mut accts) };

	let token_account = account_views[0]
		.as_token_account_checked_with_owners(&[token::ID, token_2022::ID])
		.unwrap_or_else(|e| panic!("multi-owner token account load failed: {e:?}"));
	assert_eq!(token_account.amount(), 55);
	assert_eq!(token_account.mint(), &mint);
	assert_eq!(token_account.owner(), &owner);

	assert!(matches!(
		account_views[0].try_borrow_mut(),
		Err(ProgramError::AccountBorrowFailed)
	));

	drop(token_account);

	assert!(account_views[0].try_borrow_mut().is_ok());
}

#[cfg(feature = "token")]
#[test]
fn as_associated_token_account_checked_supports_token_2022_under_miri() {
	let wallet: Address = address!("6QWeT6FpJrm8AF1btu6WH2k2Xhq6t5vbheKVfQavmeoZ");
	let mint: Address = address!("4hT5gDpr9HMmXzttW2Kz7LxyzKDn5XxhxL7sRKqGZo4x");
	let (ata_address, _bump) = try_get_associated_token_address(&wallet, &mint, &token_2022::ID)
		.unwrap_or_else(|| panic!("failed to derive ata"));
	let token_account_data = build_token_account_bytes(&mint, &wallet, 88);

	let accounts = [AccountBuilder::new()
		.address(ata_address)
		.owner(token_2022::ID)
		.lamports(1_000_000)
		.data(&token_account_data)
		.is_writable(true)];

	let mut input = unsafe { create_test_input(&accounts, &[]) };
	let mut accts = [UNINIT; 4];
	let (account_views, _) = unsafe { deserialize_test_input::<4>(&mut input, &mut accts) };

	let token_account = account_views[0]
		.as_associated_token_account_checked(&wallet, &mint, &token_2022::ID)
		.unwrap_or_else(|e| panic!("associated token account load failed: {e:?}"));
	assert_eq!(token_account.amount(), 88);
	assert_eq!(token_account.owner(), &wallet);

	assert!(matches!(
		account_views[0].try_borrow_mut(),
		Err(ProgramError::AccountBorrowFailed)
	));

	drop(token_account);

	assert!(account_views[0].try_borrow_mut().is_ok());
}
