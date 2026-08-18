//! Integration tests for the `#[pda(...)]` attribute macro.
//!
//! These tests exercise the generated seed structs, derivation helpers, and
//! the stored-bump verification method against real `AccountView` instances
//! built from SVM-format input buffers.

#![allow(unsafe_code, dead_code)]

use core::alloc::Layout;
use core::mem::MaybeUninit;
use core::mem::size_of;
use core::ptr::copy_nonoverlapping;
use std::alloc::alloc;
use std::alloc::dealloc;
use std::vec;
use std::vec::Vec;

use pina::*;
use pinocchio::account::MAX_PERMITTED_DATA_INCREASE;

// ---------------------------------------------------------------------------
// Test program definitions
// ---------------------------------------------------------------------------

const TEST_PROGRAM_ID: Address = address!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

/// Create a unique `Address` for tests (avoids the `atomic` feature).
fn unique_address(counter: u64) -> Address {
	let mut bytes = [0u8; 32];
	bytes[..8].copy_from_slice(&counter.to_le_bytes());
	Address::new_from_array(bytes)
}

/// Account discriminator for the test program.
#[discriminator(crate = ::pina)]
pub enum TestAccountType {
	TestState = 1,
	CounterState = 2,
}

/// On-chain state exercising every supported seed type.
///
/// Layout (58 bytes total):
/// | offset | size | field         |
/// |--------|------|---------------|
/// | 0      | 1    | discriminator |
/// | 1      | 32   | authority     |
/// | 33     | 8    | amount        |
/// | 41     | 1    | side          |
/// | 42     | 8    | tag           |
/// | 50     | 2    | width         |
/// | 52     | 4    | height        |
/// | 56     | 1    | bump          |
/// | 57     | 1    | padding       |
#[account(crate = ::pina, discriminator = TestAccountType)]
#[pda(
	crate = ::pina,
	seeds = [
		b"test",
		authority: Address,
		amount: u64,
		side: u8,
		tag: [u8; 8],
		width: u16,
		height: u32,
	],
	bump = bump,
)]
pub struct TestState {
	pub authority: Address,
	pub amount: u64,
	pub side: u8,
	pub tag: [u8; 8],
	pub width: u16,
	pub height: u32,
	pub bump: u8,
	pub _padding: u8,
}

/// A simple account with a single `Address` seed (mirrors `counter_program`).
#[account(crate = ::pina, discriminator = TestAccountType, variant = CounterState)]
#[pda(crate = ::pina, seeds = [b"counter", authority: Address], bump = bump)]
pub struct CounterState {
	pub authority: Address,
	pub bump: u8,
}

fn build_test_state_bytes(authority: Address, bump: u8) -> Vec<u8> {
	let mut bytes = vec![0u8; TestState::SIZE];
	let state = TestState::initialize(&mut bytes).expect("valid account storage");
	state.authority = authority;
	state.amount.set(42);
	state.side = 1;
	state.tag = [0xAB; 8];
	state.width.set(7);
	state.height.set(99);
	state.bump = bump;
	bytes
}

// ---------------------------------------------------------------------------
// Seed struct and derivation tests
// ---------------------------------------------------------------------------

#[test]
fn seeds_as_slices_matches_manual_seed_bytes() {
	let authority = unique_address(1);
	let seeds = TestState::seeds(&authority, 42, 1, [0xAB; 8], 7, 99);

	let slices = seeds.as_slices();
	assert_eq!(slices.len(), 7);
	assert_eq!(slices[0], b"test");
	assert_eq!(slices[1], authority.as_ref());
	assert_eq!(slices[2], &42u64.to_le_bytes());
	assert_eq!(slices[3], &[1u8]);
	assert_eq!(slices[4], &[0xAB; 8]);
	assert_eq!(slices[5], &7u16.to_le_bytes());
	assert_eq!(slices[6], &99u32.to_le_bytes());
}

#[test]
fn seeds_with_bump_appends_bump_seed() {
	let authority = unique_address(2);
	let seeds = TestState::seeds(&authority, 42, 1, [0xAB; 8], 7, 99).with_bump(5);

	let slices = seeds.as_slices();
	assert_eq!(slices.len(), 8);
	assert_eq!(slices[0], b"test");
	assert_eq!(slices[1], authority.as_ref());
	assert_eq!(slices[2], &42u64.to_le_bytes());
	assert_eq!(slices[3], &[1u8]);
	assert_eq!(slices[4], &[0xAB; 8]);
	assert_eq!(slices[5], &7u16.to_le_bytes());
	assert_eq!(slices[6], &99u32.to_le_bytes());
	assert_eq!(slices[7], &[5u8]);
}

#[test]
fn seeds_with_bump_keeps_seed_order() {
	let authority = unique_address(3);
	let seeds = TestState::seeds(&authority, 42, 1, [0xAB; 8], 7, 99);
	let with_bump = seeds.with_bump(5);
	let slices = with_bump.as_slices();

	assert_eq!(slices.len(), 8);
	assert_eq!(
		slices[..7],
		[
			b"test",
			authority.as_ref(),
			&42u64.to_le_bytes(),
			&[1u8],
			&[0xAB; 8],
			&7u16.to_le_bytes(),
			&99u32.to_le_bytes(),
		],
		"the bump must be appended, not inserted"
	);
	assert_eq!(slices[7], &[5u8]);
}

#[test]
fn try_find_pda_matches_manual_derivation() {
	let authority = unique_address(4);
	let manual_seeds: &[&[u8]] = &[
		b"test",
		authority.as_ref(),
		&42u64.to_le_bytes(),
		&[1u8],
		&[0xAB; 8],
		&7u16.to_le_bytes(),
		&99u32.to_le_bytes(),
	];

	let expected = try_find_program_address(manual_seeds, &TEST_PROGRAM_ID)
		.unwrap_or_else(|| panic!("expected to derive a PDA"));
	let actual = TestState::try_find_pda(&authority, 42, 1, [0xAB; 8], 7, 99, &TEST_PROGRAM_ID)
		.unwrap_or_else(|| panic!("expected to derive a PDA"));

	assert_eq!(
		actual, expected,
		"generated derivation must match manual derivation"
	);
}

#[test]
fn find_pda_matches_try_find_pda() {
	let authority = unique_address(5);
	let (expected, _) =
		TestState::try_find_pda(&authority, 42, 1, [0xAB; 8], 7, 99, &TEST_PROGRAM_ID)
			.unwrap_or_else(|| panic!("expected to derive a PDA"));

	let (actual, _bump) =
		TestState::find_pda(&authority, 42, 1, [0xAB; 8], 7, 99, &TEST_PROGRAM_ID);
	assert_eq!(actual, expected);
}

#[test]
fn try_find_pda_is_deterministic() {
	let authority = unique_address(6);
	let first = TestState::try_find_pda(&authority, 42, 1, [0xAB; 8], 7, 99, &TEST_PROGRAM_ID)
		.unwrap_or_else(|| panic!("expected to derive a PDA"));
	let second = TestState::try_find_pda(&authority, 42, 1, [0xAB; 8], 7, 99, &TEST_PROGRAM_ID)
		.unwrap_or_else(|| panic!("expected to derive a PDA"));

	assert_eq!(first, second);
}

#[test]
fn try_find_pda_differs_across_seed_values() {
	let authority = unique_address(7);
	let first = TestState::try_find_pda(&authority, 42, 1, [0xAB; 8], 7, 99, &TEST_PROGRAM_ID)
		.unwrap_or_else(|| panic!("expected to derive a PDA"));
	let second = TestState::try_find_pda(&authority, 43, 1, [0xAB; 8], 7, 99, &TEST_PROGRAM_ID)
		.unwrap_or_else(|| panic!("expected to derive a PDA"));

	assert_ne!(
		first, second,
		"different seed values must produce different PDAs"
	);
}

#[test]
fn counter_seeds_single_address_seed() {
	let authority = unique_address(8);
	let seeds = CounterState::seeds(&authority);

	let slices = seeds.as_slices();
	assert_eq!(slices.len(), 2);
	assert_eq!(slices[0], b"counter");
	assert_eq!(slices[1], authority.as_ref());

	let with_bump = seeds.with_bump(9);
	assert_eq!(with_bump.as_slices().len(), 3);
	assert_eq!(with_bump.as_slices()[2], &[9u8]);
}

#[test]
fn counter_try_find_pda_matches_manual_derivation() {
	let authority = unique_address(9);
	let expected = try_find_program_address(&[b"counter", authority.as_ref()], &TEST_PROGRAM_ID)
		.unwrap_or_else(|| panic!("expected to derive a PDA"));
	let actual = CounterState::try_find_pda(&authority, &TEST_PROGRAM_ID)
		.unwrap_or_else(|| panic!("expected to derive a PDA"));

	assert_eq!(actual, expected);
}

// ---------------------------------------------------------------------------
// Stored-bump verification tests
// ---------------------------------------------------------------------------

#[test]
fn assert_seeds_accepts_correct_account() {
	let authority = unique_address(10);
	let (pda, bump) = TestState::find_pda(&authority, 42, 1, [0xAB; 8], 7, 99, &TEST_PROGRAM_ID);
	let _ = bump;

	let state_bytes = build_test_state_bytes(authority, bump);
	let test_account = build_account_view(pda, &state_bytes);
	let account = test_account.view;
	let result = TestState::assert_seeds(
		&account,
		&authority,
		42,
		1,
		[0xAB; 8],
		7,
		99,
		&TEST_PROGRAM_ID,
	);

	assert!(
		result.is_ok(),
		"expected assert_seeds to accept the correct account: {result:?}"
	);
}

#[test]
fn assert_seeds_rejects_wrong_address() {
	let authority = unique_address(11);
	let (_pda, bump) = TestState::find_pda(&authority, 42, 1, [0xAB; 8], 7, 99, &TEST_PROGRAM_ID);
	let _ = bump;

	let state_bytes = build_test_state_bytes(authority, bump);

	// Same data, but at a different address.
	let wrong_address = unique_address(12);
	let test_account = build_account_view(wrong_address, &state_bytes);
	let account = test_account.view;
	let result = TestState::assert_seeds(
		&account,
		&authority,
		42,
		1,
		[0xAB; 8],
		7,
		99,
		&TEST_PROGRAM_ID,
	);

	assert!(
		result.is_err(),
		"expected assert_seeds to reject a wrong address"
	);
}

#[test]
fn assert_seeds_rejects_wrong_stored_bump() {
	let authority = unique_address(13);
	let (pda, bump) = TestState::find_pda(&authority, 42, 1, [0xAB; 8], 7, 99, &TEST_PROGRAM_ID);

	// Correct address, but the stored bump does not match the derived one.
	let state_bytes = build_test_state_bytes(authority, bump.wrapping_add(1));
	let test_account = build_account_view(pda, &state_bytes);
	let account = test_account.view;
	let result = TestState::assert_seeds(
		&account,
		&authority,
		42,
		1,
		[0xAB; 8],
		7,
		99,
		&TEST_PROGRAM_ID,
	);

	assert!(
		result.is_err(),
		"expected assert_seeds to reject a wrong stored bump"
	);
}

#[test]
fn assert_seeds_rejects_wrong_owner() {
	let authority = unique_address(14);
	let (pda, bump) = TestState::find_pda(&authority, 42, 1, [0xAB; 8], 7, 99, &TEST_PROGRAM_ID);
	let _ = bump;

	let state_bytes = build_test_state_bytes(authority, bump);

	// Correct address and data, but owned by a different program.
	let test_account = build_account_view_with_owner(pda, &state_bytes, unique_address(15));
	let account = test_account.view;
	let result = TestState::assert_seeds(
		&account,
		&authority,
		42,
		1,
		[0xAB; 8],
		7,
		99,
		&TEST_PROGRAM_ID,
	);

	assert!(
		result.is_err(),
		"expected assert_seeds to reject a wrong owner"
	);
}

#[test]
fn assert_seeds_rejects_wrong_seed_value() {
	let authority = unique_address(16);
	let (pda, bump) = TestState::find_pda(&authority, 42, 1, [0xAB; 8], 7, 99, &TEST_PROGRAM_ID);
	let _ = bump;

	let state_bytes = build_test_state_bytes(authority, bump);

	// The account is the PDA for `amount = 42`, so asserting with a
	// different seed value must fail even though the address matches the
	// stored bump.
	let test_account = build_account_view(pda, &state_bytes);
	let account = test_account.view;
	let result = TestState::assert_seeds(
		&account,
		&authority,
		43,
		1,
		[0xAB; 8],
		7,
		99,
		&TEST_PROGRAM_ID,
	);

	assert!(
		result.is_err(),
		"expected assert_seeds to reject a wrong seed value"
	);
}

// ---------------------------------------------------------------------------
// AccountView construction helpers (SVM input format)
// ---------------------------------------------------------------------------

const BPF_ALIGN_OF_U128: usize = 16;
const STATIC_ACCOUNT_DATA: usize = 88 + MAX_PERMITTED_DATA_INCREASE;

/// Builder for a raw account in the SVM loader input format.
struct AccountBuilder {
	address: Address,
	owner: Address,
	lamports: u64,
	data: Vec<u8>,
	is_signer: bool,
	is_writable: bool,
	executable: bool,
}

impl AccountBuilder {
	fn new(address: Address) -> Self {
		Self {
			address,
			owner: TEST_PROGRAM_ID,
			lamports: 1_000_000_000,
			data: Vec::new(),
			is_signer: false,
			is_writable: true,
			executable: false,
		}
	}

	fn owner(mut self, owner: Address) -> Self {
		self.owner = owner;
		self
	}

	fn data(mut self, data: &[u8]) -> Self {
		self.data = data.to_vec();
		self
	}
}

/// Struct representing a memory region with a specific alignment.
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

/// Creates a serialized input buffer with a single account, mimicking the
/// SVM loader input format exactly as `pinocchio::entrypoint::deserialize`
/// expects.
///
/// # Safety
///
/// The returned `AlignedMemory` must outlive any `AccountView` created from it.
unsafe fn create_test_input(account: &AccountBuilder, instruction_data: &[u8]) -> AlignedMemory {
	let data_len = account.data.len();
	let account_buf_size = STATIC_ACCOUNT_DATA + size_of::<u64>();
	let padding = (data_len + (BPF_ALIGN_OF_U128 - 1)) & !(BPF_ALIGN_OF_U128 - 1);
	let total_size = size_of::<u64>()
		+ account_buf_size
		+ padding
		+ size_of::<u64>()
		+ instruction_data.len()
		+ 32;

	let mut input = AlignedMemory::new(total_size);

	// Number of accounts.
	unsafe {
		input.write(&1u64.to_le_bytes(), 0);
	}
	let mut offset = size_of::<u64>();

	// The account buffer: RuntimeAccount header (88 bytes) + spare
	// (MAX_PERMITTED_DATA_INCREASE) + rent epoch (8 bytes).
	let mut account_buf = vec![0u8; account_buf_size];

	// RuntimeAccount header fields:
	// borrow_state = NON_DUP_MARKER (not borrowed)
	account_buf[0] = entrypoint::NON_DUP_MARKER;
	// is_signer
	account_buf[1] = u8::from(account.is_signer);
	// is_writable
	account_buf[2] = u8::from(account.is_writable);
	// executable
	account_buf[3] = u8::from(account.executable);
	// resize_delta = 0 (bytes 4-7 already zeroed)
	// address (bytes 8-39)
	account_buf[8..40].copy_from_slice(account.address.as_ref());
	// owner (bytes 40-71)
	account_buf[40..72].copy_from_slice(account.owner.as_ref());
	// lamports (bytes 72-79)
	account_buf[72..80].copy_from_slice(&account.lamports.to_le_bytes());
	// data_len (bytes 80-87)
	account_buf[80..88].copy_from_slice(&(data_len as u64).to_le_bytes());
	// Account data starts at byte 88 within the buffer.
	if !account.data.is_empty() {
		account_buf[88..88 + data_len].copy_from_slice(&account.data);
	}

	unsafe {
		input.write(&account_buf, offset);
	}
	offset += account_buf_size;

	// Alignment padding based on data_len.
	if padding > 0 {
		unsafe {
			input.write(&vec![0u8; padding], offset);
		}
		offset += padding;
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
	// Program ID.
	unsafe {
		input.write(TEST_PROGRAM_ID.as_ref(), offset);
	}

	input
}

/// Deserialize a single-account test input into an `AccountView`.
///
/// # Safety
///
/// `input` must be created by `create_test_input` and must outlive the
/// returned `AccountView`.
unsafe fn deserialize_test_input(input: &mut AlignedMemory) -> AccountView {
	let mut accounts = [MaybeUninit::<AccountView>::uninit(); 1];
	let (_program_id, account_views, _ix_data, _count) = {
		let (program_id, count, ix_data) =
			unsafe { entrypoint::deserialize::<1>(input.as_mut_ptr(), &mut accounts) };
		let account_views: &mut [AccountView] =
			unsafe { core::slice::from_raw_parts_mut(accounts.as_mut_ptr().cast(), count) };
		(program_id, account_views, ix_data, count)
	};
	account_views[0]
}

/// A test account: an `AccountView` plus the aligned memory it borrows.
///
/// The memory must outlive the view, so both are kept together.
struct TestAccount {
	view: AccountView,
	_memory: AlignedMemory,
}

/// Build a test account owned by `TEST_PROGRAM_ID`.
fn build_account_view(address: Address, data: &[u8]) -> TestAccount {
	build_account_view_with_owner(address, data, TEST_PROGRAM_ID)
}

/// Build a test account with a custom owner.
fn build_account_view_with_owner(address: Address, data: &[u8], owner: Address) -> TestAccount {
	let builder = AccountBuilder::new(address).owner(owner).data(data);
	let mut input = unsafe { create_test_input(&builder, &[]) };
	let view = unsafe { deserialize_test_input(&mut input) };
	TestAccount {
		view,
		_memory: input,
	}
}
