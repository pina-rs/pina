//! Integration tests for pina framework.
//!
//! These tests exercise full program flows using pina's traits and macros,
//! covering account lifecycle, multi-instruction flows, error handling, and
//! CPI-related operations.
//!
//! Since `mollusk-svm` requires compiled ELF binaries and cannot test native
//! Rust functions directly, these tests construct raw account memory buffers
//! that mimic the SVM input format and use `pinocchio::entrypoint::deserialize`
//! to create `AccountView` instances. This allows testing the pina framework's
//! processing logic natively, including validation chains, account
//! deserialization, discriminator checks, and state management.

#![allow(unsafe_code, dead_code)]

use core::alloc::Layout;
use core::mem::MaybeUninit;
use core::ptr::copy_nonoverlapping;
use std::alloc::alloc;
use std::alloc::dealloc;
use std::vec;
use std::vec::Vec;

use pina::*;
use pinocchio::account::MAX_PERMITTED_DATA_INCREASE;

// ---------------------------------------------------------------------------
// Program and discriminator definitions for the test program
// ---------------------------------------------------------------------------

// A fake program ID for our test program.
const TEST_PROGRAM_ID: Address = address!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

/// Instruction discriminator for the test program.
#[discriminator(crate = ::pina)]
#[derive(Debug)]
pub enum TestInstruction {
	Initialize = 0,
	Update = 1,
	Close = 2,
}

/// Account discriminator for the test program.
#[discriminator(crate = ::pina)]
pub enum TestAccountType {
	TestState = 1,
}

/// On-chain state for the test program.
///
/// Layout (12 bytes total):
/// | offset | size | field         |
/// |--------|------|---------------|
/// | 0      | 1    | discriminator |
/// | 1      | 1    | bump          |
/// | 2      | 2    | padding       |
/// | 4      | 8    | value (PodU64)|
#[account(crate = ::pina, discriminator = TestAccountType)]
pub struct TestState {
	pub bump: u8,
	pub _padding: u8,
	pub _padding2: u8,
	pub value: PodU64,
}

/// Instruction data for Initialize.
#[instruction(crate = ::pina, discriminator = TestInstruction, variant = Initialize)]
pub struct InitializeInstr {
	pub bump: u8,
	pub initial_value: PodU64,
}

/// Instruction data for Update.
#[instruction(crate = ::pina, discriminator = TestInstruction, variant = Update)]
pub struct UpdateInstr {
	pub new_value: PodU64,
}

/// Instruction data for Close (no extra fields).
#[instruction(crate = ::pina, discriminator = TestInstruction, variant = Close)]
pub struct CloseInstr {}

/// Accounts for Initialize.
#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct InitializeAccounts<'a> {
	pub authority: &'a AccountView,
	pub state_account: &'a AccountView,
	pub system_program: &'a AccountView,
}

/// Accounts for Update.
#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct UpdateAccounts<'a> {
	pub authority: &'a AccountView,
	pub state_account: &'a AccountView,
}

/// Accounts for Close.
#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct CloseAccounts<'a> {
	pub authority: &'a AccountView,
	pub state_account: &'a AccountView,
}

// ---------------------------------------------------------------------------
// Instruction processors
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = InitializeInstr::try_from_bytes(data)?;

		// Validate accounts.
		self.authority.assert_signer()?;
		self.state_account.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;

		// In a real program, the CPI to system program creates the account
		// with zeroed data. We simulate that by writing the full state
		// (including discriminator) directly into the raw bytes.
		let new_state = TestState::builder()
			.bump(args.bump)
			._padding(0)
			._padding2(0)
			.value(args.initial_value)
			.build();
		let state_bytes = bytemuck::bytes_of(&new_state);

		// Write directly to the account's raw data, bypassing discriminator
		// validation (which would fail on zeroed/uninitialized data).
		let mut account_data = self.state_account.try_borrow_mut()?;
		if account_data.len() < state_bytes.len() {
			return Err(ProgramError::AccountDataTooSmall);
		}
		account_data[..state_bytes.len()].copy_from_slice(state_bytes);

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for UpdateAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = UpdateInstr::try_from_bytes(data)?;

		// Validate accounts.
		self.authority.assert_signer()?;
		self.state_account
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<TestState>(&TEST_PROGRAM_ID)?;

		// Update state.
		let state = self
			.state_account
			.as_account_mut::<TestState>(&TEST_PROGRAM_ID)?;
		state.value = args.new_value;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for CloseAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = CloseInstr::try_from_bytes(data)?;

		// Validate accounts.
		self.authority.assert_signer()?.assert_writable()?;
		self.state_account
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<TestState>(&TEST_PROGRAM_ID)?;

		// Zero account data before closing.
		let state = self
			.state_account
			.as_account_mut::<TestState>(&TEST_PROGRAM_ID)?;
		state.zeroed();

		// Transfer lamports back to authority (simulated via direct lamport
		// manipulation).
		self.state_account
			.send(self.state_account.lamports(), self.authority)?;

		Ok(())
	}
}

/// Top-level instruction dispatch.
fn process_instruction(
	program_id: &Address,
	accounts: &[AccountView],
	data: &[u8],
) -> ProgramResult {
	let instruction: TestInstruction = parse_instruction(program_id, &TEST_PROGRAM_ID, data)?;

	match instruction {
		TestInstruction::Initialize => InitializeAccounts::try_from(accounts)?.process(data),
		TestInstruction::Update => UpdateAccounts::try_from(accounts)?.process(data),
		TestInstruction::Close => CloseAccounts::try_from(accounts)?.process(data),
	}
}

// ---------------------------------------------------------------------------
// Test helpers — memory layout for creating AccountView instances
// ---------------------------------------------------------------------------

/// `assert_eq(core::mem::align_of::<u128>(), 8)` is true for BPF but not
/// for some host machines.
const BPF_ALIGN_OF_U128: usize = 8;
/// An uninitialized account view.
const UNINIT: MaybeUninit<AccountView> = MaybeUninit::<AccountView>::uninit();
/// The "static" size of an account in the input buffer (header + max data
/// increase).
const STATIC_ACCOUNT_DATA: usize = 88 + MAX_PERMITTED_DATA_INCREASE;

/// Builder for individual accounts in the test input buffer.
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
	fn new() -> Self {
		Self {
			address: Address::default(),
			owner: Address::default(),
			lamports: 0,
			data: Vec::new(),
			is_signer: false,
			is_writable: false,
			executable: false,
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

	fn is_signer(mut self, is_signer: bool) -> Self {
		self.is_signer = is_signer;
		self
	}

	fn is_writable(mut self, is_writable: bool) -> Self {
		self.is_writable = is_writable;
		self
	}

	fn executable(mut self, executable: bool) -> Self {
		self.executable = executable;
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

/// Compute the exact buffer size needed for a given set of accounts and
/// instruction data, following the SVM loader input format.
fn compute_input_size(accounts: &[AccountBuilder], instruction_data: &[u8]) -> usize {
	let mut size = size_of::<u64>(); // number of accounts

	for builder in accounts {
		let data_len = builder.data.len();
		let account_buf_size = STATIC_ACCOUNT_DATA + size_of::<u64>();
		size += account_buf_size;
		// Alignment padding based on data_len.
		let padding = (data_len + (BPF_ALIGN_OF_U128 - 1)) & !(BPF_ALIGN_OF_U128 - 1);
		size += padding;
	}

	size += size_of::<u64>(); // instruction data length
	size += instruction_data.len(); // instruction data
	size += 32; // program ID

	size
}

/// Creates a serialized input buffer with custom accounts and instruction data.
///
/// This mimics the SVM loader input format exactly as `pinocchio::entrypoint::deserialize`
/// expects. Each account occupies:
///
///   `STATIC_ACCOUNT_DATA + size_of::<u64>()` bytes for the header + spare + rent epoch,
///   followed by `align8(data_len)` bytes of alignment padding.
///
/// The RuntimeAccount header layout (88 bytes):
///   offset 0: borrow_state (NON_DUP_MARKER = 0xFF)
///   offset 1: is_signer
///   offset 2: is_writable
///   offset 3: executable
///   offset 4-7: resize_delta (i32, zeroed)
///   offset 8-39: address (32 bytes)
///   offset 40-71: owner (32 bytes)
///   offset 72-79: lamports (u64 LE)
///   offset 80-87: data_len (u64 LE)
///   offset 88+: account data, then spare space, then rent epoch
///
/// # Safety
///
/// The returned `AlignedMemory` must outlive any `AccountView` created from it.
unsafe fn create_test_input(accounts: &[AccountBuilder], instruction_data: &[u8]) -> AlignedMemory {
	let total_size = compute_input_size(accounts, instruction_data);
	let mut input = AlignedMemory::new(total_size);

	// Number of accounts.
	unsafe {
		input.write(&(accounts.len() as u64).to_le_bytes(), 0);
	}
	let mut offset = size_of::<u64>();

	for builder in accounts {
		let data_len = builder.data.len();

		// The account buffer: RuntimeAccount header (88 bytes) + spare
		// (MAX_PERMITTED_DATA_INCREASE) + rent epoch (8 bytes).
		// The data_len bytes of actual data sit inside the spare area starting at
		// offset 88.
		let account_buf_size = STATIC_ACCOUNT_DATA + size_of::<u64>();
		let mut account_buf = vec![0u8; account_buf_size];

		// RuntimeAccount header fields:
		// borrow_state = NON_DUP_MARKER (not borrowed)
		account_buf[0] = entrypoint::NON_DUP_MARKER;
		// is_signer
		account_buf[1] = u8::from(builder.is_signer);
		// is_writable
		account_buf[2] = u8::from(builder.is_writable);
		// executable
		account_buf[3] = u8::from(builder.executable);
		// resize_delta = 0 (bytes 4-7 already zeroed)
		// address (bytes 8-39)
		account_buf[8..40].copy_from_slice(builder.address.as_ref());
		// owner (bytes 40-71)
		account_buf[40..72].copy_from_slice(builder.owner.as_ref());
		// lamports (bytes 72-79)
		account_buf[72..80].copy_from_slice(&builder.lamports.to_le_bytes());
		// data_len (bytes 80-87)
		account_buf[80..88].copy_from_slice(&(data_len as u64).to_le_bytes());
		// Account data starts at byte 88 within the buffer.
		if !builder.data.is_empty() {
			account_buf[88..88 + data_len].copy_from_slice(&builder.data);
		}

		unsafe {
			input.write(&account_buf, offset);
		}
		offset += account_buf_size;

		// Alignment padding based on data_len (aligns pointer to
		// BPF_ALIGN_OF_U128).
		let padding = (data_len + (BPF_ALIGN_OF_U128 - 1)) & !(BPF_ALIGN_OF_U128 - 1);
		if padding > 0 {
			unsafe {
				input.write(&vec![0u8; padding], offset);
			}
			offset += padding;
		}
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

/// Helper to deserialize a test input into AccountViews and instruction data.
///
/// Returns (program_id, count, instruction_data).
///
/// The `accounts` array must be passed in by the caller (stack-allocated in
/// the test function) so that `AccountView` references remain valid for the
/// lifetime of the test. This prevents a use-after-free: if the accounts
/// array were local to this function, the returned `AccountView` slice would
/// point to dead stack memory.
///
/// # Safety
///
/// `input` must be created by `create_test_input` and must outlive any
/// `AccountView` created from it. `accounts` must be stack-allocated in the
/// calling function's frame.
unsafe fn deserialize_test_input<const MAX_ACCOUNTS: usize>(
	input: &mut AlignedMemory,
	accounts: &mut [MaybeUninit<AccountView>; MAX_ACCOUNTS],
) -> (
	&'static Address,
	&'static [AccountView],
	&'static [u8],
	usize,
) {
	let (program_id, count, ix_data) =
		unsafe { entrypoint::deserialize::<MAX_ACCOUNTS>(input.as_mut_ptr(), accounts) };
	let accounts: &[AccountView] =
		unsafe { core::slice::from_raw_parts(accounts.as_ptr().cast(), count) };
	(program_id, accounts, ix_data, count)
}

/// Create a `TestState` serialized as bytes with proper discriminator.
fn build_test_state_bytes(bump: u8, value: u64) -> Vec<u8> {
	let state = TestState::builder()
		.bump(bump)
		._padding(0)
		._padding2(0)
		.value(PodU64::from_primitive(value))
		.build();
	bytemuck::bytes_of(&state).to_vec()
}

// ---------------------------------------------------------------------------
// Test: Full account lifecycle
// ---------------------------------------------------------------------------

/// Tests the full lifecycle: initialize -> read -> update -> close.
#[test]
fn full_account_lifecycle() {
	let authority_key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let state_key: Address = address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki");
	let rent_lamports: u64 = 1_000_000;

	// --- Step 1: Initialize ---
	// The state account is pre-created (as if system program CPI already ran)
	// with the correct size and owner, but all data zeroed except discriminator
	// won't be written yet.
	let state_data = vec![0u8; size_of::<TestState>()];

	let init_data = InitializeInstr::builder()
		.bump(42)
		.initial_value(PodU64::from_primitive(100))
		.build();
	let init_bytes = bytemuck::bytes_of(&init_data);

	let accounts = [
		AccountBuilder::new()
			.address(authority_key)
			.owner(system::ID)
			.lamports(10_000_000)
			.is_signer(true)
			.is_writable(true),
		AccountBuilder::new()
			.address(state_key)
			.owner(TEST_PROGRAM_ID)
			.lamports(rent_lamports)
			.data(&state_data)
			.is_writable(true),
		AccountBuilder::new().address(system::ID).executable(true),
	];

	let mut input = unsafe { create_test_input(&accounts, init_bytes) };
	let mut accts = [UNINIT; 10];
	let (program_id, account_views, ix_data, _) =
		unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = process_instruction(program_id, account_views, ix_data);
	assert!(result.is_ok(), "Initialize should succeed, got: {result:?}");

	// Verify state was written.
	let state = account_views[1]
		.as_account::<TestState>(&TEST_PROGRAM_ID)
		.unwrap_or_else(|e| panic!("failed to read state after init: {e:?}"));
	assert_eq!(state.bump, 42, "bump should be 42");
	assert_eq!(u64::from(state.value), 100, "initial value should be 100");

	// --- Step 2: Update ---
	// Reuse the same memory (state account now has initialized data).
	let update_data = UpdateInstr::builder()
		.new_value(PodU64::from_primitive(999))
		.build();
	let update_bytes = bytemuck::bytes_of(&update_data);

	let state_bytes = build_test_state_bytes(42, 100);
	let accounts = [
		AccountBuilder::new()
			.address(authority_key)
			.owner(system::ID)
			.lamports(10_000_000)
			.is_signer(true)
			.is_writable(true),
		AccountBuilder::new()
			.address(state_key)
			.owner(TEST_PROGRAM_ID)
			.lamports(rent_lamports)
			.data(&state_bytes)
			.is_writable(true),
	];

	let mut input = unsafe { create_test_input(&accounts, update_bytes) };
	let mut accts = [UNINIT; 10];
	let (program_id, account_views, ix_data, _) =
		unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = process_instruction(program_id, account_views, ix_data);
	assert!(result.is_ok(), "Update should succeed, got: {result:?}");

	// Verify state was updated.
	let state = account_views[1]
		.as_account::<TestState>(&TEST_PROGRAM_ID)
		.unwrap_or_else(|e| panic!("failed to read state after update: {e:?}"));
	assert_eq!(u64::from(state.value), 999, "value should be 999");
	assert_eq!(state.bump, 42, "bump should remain 42");

	// --- Step 3: Close ---
	let state_bytes = build_test_state_bytes(42, 999);
	let close_data = CloseInstr::builder().build();
	let close_bytes = bytemuck::bytes_of(&close_data);

	let accounts = [
		AccountBuilder::new()
			.address(authority_key)
			.owner(system::ID)
			.lamports(10_000_000)
			.is_signer(true)
			.is_writable(true),
		AccountBuilder::new()
			.address(state_key)
			.owner(TEST_PROGRAM_ID)
			.lamports(rent_lamports)
			.data(&state_bytes)
			.is_writable(true),
	];

	let mut input = unsafe { create_test_input(&accounts, close_bytes) };
	let mut accts = [UNINIT; 10];
	let (program_id, account_views, ix_data, _) =
		unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let authority_lamports_before = account_views[0].lamports();
	let state_lamports_before = account_views[1].lamports();

	let result = process_instruction(program_id, account_views, ix_data);
	assert!(result.is_ok(), "Close should succeed, got: {result:?}");

	// Verify lamports were returned to authority.
	assert_eq!(
		account_views[0].lamports(),
		authority_lamports_before + state_lamports_before,
		"authority should receive all rent lamports"
	);
	assert_eq!(
		account_views[1].lamports(),
		0,
		"state account should have 0 lamports after close"
	);
}

// ---------------------------------------------------------------------------
// Test: Multi-instruction flow
// ---------------------------------------------------------------------------

/// Tests processing Initialize followed by Update on the same account type,
/// verifying state after each step.
#[test]
fn multi_instruction_flow_initialize_then_update() {
	let authority_key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let state_key: Address = address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki");

	// --- Initialize with value=50 ---
	let state_data = vec![0u8; size_of::<TestState>()];
	let init_data = InitializeInstr::builder()
		.bump(7)
		.initial_value(PodU64::from_primitive(50))
		.build();
	let init_bytes = bytemuck::bytes_of(&init_data);

	let accounts = [
		AccountBuilder::new()
			.address(authority_key)
			.owner(system::ID)
			.lamports(5_000_000)
			.is_signer(true)
			.is_writable(true),
		AccountBuilder::new()
			.address(state_key)
			.owner(TEST_PROGRAM_ID)
			.lamports(890_880)
			.data(&state_data)
			.is_writable(true),
		AccountBuilder::new().address(system::ID).executable(true),
	];

	let mut input = unsafe { create_test_input(&accounts, init_bytes) };
	let mut accts = [UNINIT; 10];
	let (program_id, account_views, ix_data, _) =
		unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = process_instruction(program_id, account_views, ix_data);
	assert!(result.is_ok(), "Initialize failed: {result:?}");

	// Verify initial state.
	let state = account_views[1]
		.as_account::<TestState>(&TEST_PROGRAM_ID)
		.unwrap_or_else(|e| panic!("read failed: {e:?}"));
	assert_eq!(u64::from(state.value), 50);
	assert_eq!(state.bump, 7);

	// --- Update to value=200 ---
	let state_bytes = build_test_state_bytes(7, 50);
	let update_data = UpdateInstr::builder()
		.new_value(PodU64::from_primitive(200))
		.build();
	let update_bytes = bytemuck::bytes_of(&update_data);

	let accounts = [
		AccountBuilder::new()
			.address(authority_key)
			.owner(system::ID)
			.lamports(5_000_000)
			.is_signer(true)
			.is_writable(true),
		AccountBuilder::new()
			.address(state_key)
			.owner(TEST_PROGRAM_ID)
			.lamports(890_880)
			.data(&state_bytes)
			.is_writable(true),
	];

	let mut input = unsafe { create_test_input(&accounts, update_bytes) };
	let mut accts = [UNINIT; 10];
	let (program_id, account_views, ix_data, _) =
		unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = process_instruction(program_id, account_views, ix_data);
	assert!(result.is_ok(), "Update failed: {result:?}");

	let state = account_views[1]
		.as_account::<TestState>(&TEST_PROGRAM_ID)
		.unwrap_or_else(|e| panic!("read failed: {e:?}"));
	assert_eq!(u64::from(state.value), 200);
	assert_eq!(state.bump, 7, "bump should be preserved across update");
}

/// Tests multiple sequential updates.
#[test]
fn multi_update_flow() {
	let authority_key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let state_key: Address = address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki");

	let mut current_value: u64 = 10;

	for new_value in [20u64, 30, 40, 50, u64::MAX] {
		let state_bytes = build_test_state_bytes(1, current_value);
		let update_data = UpdateInstr::builder()
			.new_value(PodU64::from_primitive(new_value))
			.build();
		let update_bytes = bytemuck::bytes_of(&update_data);

		let accounts = [
			AccountBuilder::new()
				.address(authority_key)
				.owner(system::ID)
				.lamports(5_000_000)
				.is_signer(true)
				.is_writable(true),
			AccountBuilder::new()
				.address(state_key)
				.owner(TEST_PROGRAM_ID)
				.lamports(890_880)
				.data(&state_bytes)
				.is_writable(true),
		];

		let mut input = unsafe { create_test_input(&accounts, update_bytes) };
		let mut accts = [UNINIT; 10];
		let (program_id, account_views, ix_data, _) =
			unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

		let result = process_instruction(program_id, account_views, ix_data);
		assert!(result.is_ok(), "Update to {new_value} failed: {result:?}");

		let state = account_views[1]
			.as_account::<TestState>(&TEST_PROGRAM_ID)
			.unwrap_or_else(|e| panic!("read failed: {e:?}"));
		assert_eq!(
			u64::from(state.value),
			new_value,
			"value should be {new_value}"
		);

		current_value = new_value;
	}
}

// ---------------------------------------------------------------------------
// Test: Error handling
// ---------------------------------------------------------------------------

/// Tests that a missing signer is rejected.
#[test]
fn error_missing_signer_rejected() {
	let authority_key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let state_key: Address = address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki");

	let state_data = vec![0u8; size_of::<TestState>()];
	let init_data = InitializeInstr::builder()
		.bump(1)
		.initial_value(PodU64::from_primitive(0))
		.build();
	let init_bytes = bytemuck::bytes_of(&init_data);

	// Authority is NOT a signer.
	let accounts = [
		AccountBuilder::new()
			.address(authority_key)
			.owner(system::ID)
			.lamports(5_000_000)
			.is_signer(false) // <-- not a signer
			.is_writable(true),
		AccountBuilder::new()
			.address(state_key)
			.owner(TEST_PROGRAM_ID)
			.lamports(890_880)
			.data(&state_data)
			.is_writable(true),
		AccountBuilder::new().address(system::ID).executable(true),
	];

	let mut input = unsafe { create_test_input(&accounts, init_bytes) };
	let mut accts = [UNINIT; 10];
	let (program_id, account_views, ix_data, _) =
		unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = process_instruction(program_id, account_views, ix_data);
	assert!(result.is_err(), "should fail without signer");
	assert_eq!(
		result.unwrap_err(),
		ProgramError::MissingRequiredSignature,
		"error should be MissingRequiredSignature"
	);
}

/// Tests that wrong program owner is rejected.
#[test]
fn error_wrong_program_owner_rejected() {
	let authority_key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let state_key: Address = address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki");

	let state_bytes = build_test_state_bytes(1, 100);
	let update_data = UpdateInstr::builder()
		.new_value(PodU64::from_primitive(200))
		.build();
	let update_bytes = bytemuck::bytes_of(&update_data);

	// Wrong owner — system::ID instead of TEST_PROGRAM_ID.
	let wrong_owner = system::ID;

	let accounts = [
		AccountBuilder::new()
			.address(authority_key)
			.owner(system::ID)
			.lamports(5_000_000)
			.is_signer(true)
			.is_writable(true),
		AccountBuilder::new()
			.address(state_key)
			.owner(wrong_owner) // <-- wrong owner
			.lamports(890_880)
			.data(&state_bytes)
			.is_writable(true),
	];

	let mut input = unsafe { create_test_input(&accounts, update_bytes) };
	let mut accts = [UNINIT; 10];
	let (program_id, account_views, ix_data, _) =
		unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = process_instruction(program_id, account_views, ix_data);
	assert!(result.is_err(), "should fail with wrong owner");
	assert_eq!(
		result.unwrap_err(),
		ProgramError::InvalidAccountOwner,
		"error should be InvalidAccountOwner"
	);
}

/// Tests that discriminator mismatch is rejected during `assert_type`.
#[test]
fn error_discriminator_mismatch_rejected() {
	let authority_key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let state_key: Address = address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki");

	// Wrong discriminator — first byte is 99 instead of
	// TestAccountType::TestState.
	let mut bad_data = vec![0u8; size_of::<TestState>()];
	bad_data[0] = 99; // Wrong discriminator.

	let update_data = UpdateInstr::builder()
		.new_value(PodU64::from_primitive(200))
		.build();
	let update_bytes = bytemuck::bytes_of(&update_data);

	let accounts = [
		AccountBuilder::new()
			.address(authority_key)
			.owner(system::ID)
			.lamports(5_000_000)
			.is_signer(true)
			.is_writable(true),
		AccountBuilder::new()
			.address(state_key)
			.owner(TEST_PROGRAM_ID)
			.lamports(890_880)
			.data(&bad_data)
			.is_writable(true),
	];

	let mut input = unsafe { create_test_input(&accounts, update_bytes) };
	let mut accts = [UNINIT; 10];
	let (program_id, account_views, ix_data, _) =
		unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = process_instruction(program_id, account_views, ix_data);
	assert!(result.is_err(), "should fail with discriminator mismatch");
	assert_eq!(
		result.unwrap_err(),
		ProgramError::InvalidAccountData,
		"error should be InvalidAccountData for discriminator mismatch"
	);
}

/// Tests that data length mismatch is rejected.
#[test]
fn error_data_length_mismatch_rejected() {
	let authority_key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let state_key: Address = address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki");

	// Data is too short — only 5 bytes when TestState needs
	// size_of::<TestState>().
	let short_data = vec![TestAccountType::TestState as u8, 0, 0, 0, 0];

	let update_data = UpdateInstr::builder()
		.new_value(PodU64::from_primitive(200))
		.build();
	let update_bytes = bytemuck::bytes_of(&update_data);

	let accounts = [
		AccountBuilder::new()
			.address(authority_key)
			.owner(system::ID)
			.lamports(5_000_000)
			.is_signer(true)
			.is_writable(true),
		AccountBuilder::new()
			.address(state_key)
			.owner(TEST_PROGRAM_ID)
			.lamports(890_880)
			.data(&short_data)
			.is_writable(true),
	];

	let mut input = unsafe { create_test_input(&accounts, update_bytes) };
	let mut accts = [UNINIT; 10];
	let (program_id, account_views, ix_data, _) =
		unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = process_instruction(program_id, account_views, ix_data);
	assert!(result.is_err(), "should fail with data length mismatch");
	// assert_type checks both discriminator and size; with wrong size it
	// returns AccountDataTooSmall.
	assert_eq!(
		result.unwrap_err(),
		ProgramError::AccountDataTooSmall,
		"error should be AccountDataTooSmall for size mismatch"
	);
}

/// Tests that an invalid instruction discriminator is rejected.
#[test]
fn error_invalid_instruction_discriminator() {
	let authority_key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let state_key: Address = address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki");

	// Invalid instruction discriminator byte.
	let bad_ix_data = [99u8, 0, 0, 0, 0, 0, 0, 0, 0, 0];

	let accounts = [
		AccountBuilder::new()
			.address(authority_key)
			.owner(system::ID)
			.lamports(5_000_000)
			.is_signer(true)
			.is_writable(true),
		AccountBuilder::new()
			.address(state_key)
			.owner(TEST_PROGRAM_ID)
			.lamports(890_880)
			.is_writable(true),
	];

	let mut input = unsafe { create_test_input(&accounts, &bad_ix_data) };
	let mut accts = [UNINIT; 10];
	let (program_id, account_views, ix_data, _) =
		unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = process_instruction(program_id, account_views, ix_data);
	assert!(result.is_err(), "should fail with invalid discriminator");
	assert_eq!(
		result.unwrap_err(),
		ProgramError::InvalidInstructionData,
		"error should be InvalidInstructionData"
	);
}

/// Tests that an empty instruction data is rejected.
#[test]
fn error_empty_instruction_data() {
	let authority_key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");

	let accounts = [AccountBuilder::new()
		.address(authority_key)
		.owner(system::ID)
		.lamports(5_000_000)
		.is_signer(true)
		.is_writable(true)];

	let mut input = unsafe { create_test_input(&accounts, &[]) };
	let mut accts = [UNINIT; 10];
	let (program_id, account_views, ix_data, _) =
		unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = process_instruction(program_id, account_views, ix_data);
	assert!(result.is_err(), "should fail with empty data");
	assert_eq!(
		result.unwrap_err(),
		ProgramError::InvalidInstructionData,
		"error should be InvalidInstructionData for empty data"
	);
}

/// Tests that wrong program ID is rejected.
#[test]
fn error_wrong_program_id() {
	let authority_key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let wrong_program_id = system::ID;

	let init_data = InitializeInstr::builder()
		.bump(1)
		.initial_value(PodU64::from_primitive(0))
		.build();
	let init_bytes = bytemuck::bytes_of(&init_data);

	let accounts = [AccountBuilder::new()
		.address(authority_key)
		.owner(system::ID)
		.lamports(5_000_000)
		.is_signer(true)
		.is_writable(true)];

	// Process with wrong program ID.
	let result =
		parse_instruction::<TestInstruction>(&wrong_program_id, &TEST_PROGRAM_ID, init_bytes);
	assert!(result.is_err(), "should fail with wrong program ID");
	assert_eq!(
		result.unwrap_err(),
		ProgramError::IncorrectProgramId,
		"error should be IncorrectProgramId"
	);
	drop(accounts);
}

/// Tests that not enough accounts is rejected.
#[test]
fn error_not_enough_accounts() {
	let authority_key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");

	let init_data = InitializeInstr::builder()
		.bump(1)
		.initial_value(PodU64::from_primitive(0))
		.build();
	let init_bytes = bytemuck::bytes_of(&init_data);

	// Only 1 account, but Initialize needs 3.
	let accounts = [AccountBuilder::new()
		.address(authority_key)
		.owner(system::ID)
		.lamports(5_000_000)
		.is_signer(true)
		.is_writable(true)];

	let mut input = unsafe { create_test_input(&accounts, init_bytes) };
	let mut accts = [UNINIT; 10];
	let (program_id, account_views, ix_data, _) =
		unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = process_instruction(program_id, account_views, ix_data);
	assert!(result.is_err(), "should fail with not enough accounts");
	assert_eq!(
		result.unwrap_err(),
		ProgramError::NotEnoughAccountKeys,
		"error should be NotEnoughAccountKeys"
	);
}

/// Tests that non-writable state account is rejected during update.
#[test]
fn error_non_writable_rejected() {
	let authority_key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let state_key: Address = address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki");

	let state_bytes = build_test_state_bytes(1, 100);
	let update_data = UpdateInstr::builder()
		.new_value(PodU64::from_primitive(200))
		.build();
	let update_bytes = bytemuck::bytes_of(&update_data);

	// State account is NOT writable.
	let accounts = [
		AccountBuilder::new()
			.address(authority_key)
			.owner(system::ID)
			.lamports(5_000_000)
			.is_signer(true)
			.is_writable(true),
		AccountBuilder::new()
			.address(state_key)
			.owner(TEST_PROGRAM_ID)
			.lamports(890_880)
			.data(&state_bytes)
			.is_writable(false), // <-- not writable
	];

	let mut input = unsafe { create_test_input(&accounts, update_bytes) };
	let mut accts = [UNINIT; 10];
	let (program_id, account_views, ix_data, _) =
		unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = process_instruction(program_id, account_views, ix_data);
	assert!(result.is_err(), "should fail with non-writable account");
	assert_eq!(
		result.unwrap_err(),
		ProgramError::InvalidAccountData,
		"error should be InvalidAccountData for non-writable"
	);
}

/// Tests that operating on an empty account (uninitialized) is rejected by
/// assert_not_empty.
#[test]
fn error_empty_account_rejected_for_update() {
	let authority_key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let state_key: Address = address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki");

	let update_data = UpdateInstr::builder()
		.new_value(PodU64::from_primitive(200))
		.build();
	let update_bytes = bytemuck::bytes_of(&update_data);

	// Account has no data — it's empty.
	let accounts = [
		AccountBuilder::new()
			.address(authority_key)
			.owner(system::ID)
			.lamports(5_000_000)
			.is_signer(true)
			.is_writable(true),
		AccountBuilder::new()
			.address(state_key)
			.owner(TEST_PROGRAM_ID)
			.lamports(0)
			.is_writable(true),
	];

	let mut input = unsafe { create_test_input(&accounts, update_bytes) };
	let mut accts = [UNINIT; 10];
	let (program_id, account_views, ix_data, _) =
		unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = process_instruction(program_id, account_views, ix_data);
	assert!(result.is_err(), "should fail on empty account");
	assert_eq!(
		result.unwrap_err(),
		ProgramError::UninitializedAccount,
		"error should be UninitializedAccount for empty account"
	);
}

// ---------------------------------------------------------------------------
// Test: CPI helpers (system_program transfer simulation)
// ---------------------------------------------------------------------------

/// Tests the lamport transfer trait (`send`) on AccountView: debit sender,
/// credit recipient.
#[test]
fn lamport_transfer_send() {
	let sender_key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let recipient_key: Address = address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki");

	let accounts = [
		AccountBuilder::new()
			.address(sender_key)
			.owner(TEST_PROGRAM_ID) // Must be owned by program to send
			.lamports(1_000_000)
			.is_writable(true),
		AccountBuilder::new()
			.address(recipient_key)
			.owner(TEST_PROGRAM_ID)
			.lamports(500_000)
			.is_writable(true),
	];

	let dummy_data: &[u8] = &[0u8];
	let mut input = unsafe { create_test_input(&accounts, dummy_data) };
	let mut accts = [UNINIT; 10];
	let (_, account_views, ..) = unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	// Transfer 300_000 lamports from sender to recipient.
	let result = account_views[0].send(300_000, &account_views[1]);
	assert!(result.is_ok(), "send should succeed: {result:?}");

	assert_eq!(
		account_views[0].lamports(),
		700_000,
		"sender should have 700_000"
	);
	assert_eq!(
		account_views[1].lamports(),
		800_000,
		"recipient should have 800_000"
	);
}

/// Tests that sending more lamports than available fails with
/// InsufficientFunds.
#[test]
fn lamport_transfer_insufficient_funds() {
	let sender_key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let recipient_key: Address = address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki");

	let accounts = [
		AccountBuilder::new()
			.address(sender_key)
			.owner(TEST_PROGRAM_ID)
			.lamports(100)
			.is_writable(true),
		AccountBuilder::new()
			.address(recipient_key)
			.owner(TEST_PROGRAM_ID)
			.lamports(0)
			.is_writable(true),
	];

	let dummy_data: &[u8] = &[0u8];
	let mut input = unsafe { create_test_input(&accounts, dummy_data) };
	let mut accts = [UNINIT; 10];
	let (_, account_views, ..) = unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = account_views[0].send(101, &account_views[1]);
	assert!(result.is_err(), "should fail with insufficient funds");
	assert_eq!(
		result.unwrap_err(),
		ProgramError::InsufficientFunds,
		"error should be InsufficientFunds"
	);

	// Balances should be unchanged.
	assert_eq!(account_views[0].lamports(), 100);
	assert_eq!(account_views[1].lamports(), 0);
}

/// Tests that sending to the same account fails with InvalidArgument.
#[test]
fn lamport_transfer_same_account_rejected() {
	let key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");

	let accounts = [AccountBuilder::new()
		.address(key)
		.owner(TEST_PROGRAM_ID)
		.lamports(1_000)
		.is_writable(true)];

	let dummy_data: &[u8] = &[0u8];
	let mut input = unsafe { create_test_input(&accounts, dummy_data) };
	let mut accts = [UNINIT; 10];
	let (_, account_views, ..) = unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = account_views[0].send(500, &account_views[0]);
	assert!(result.is_err(), "should fail sending to self");
	assert_eq!(
		result.unwrap_err(),
		ProgramError::InvalidArgument,
		"error should be InvalidArgument for same account"
	);
}

/// Tests close_with_recipient: zero lamports + data clearing.
#[test]
fn close_account_with_recipient() {
	let account_key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let recipient_key: Address = address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki");

	let state_data = build_test_state_bytes(1, 42);

	let accounts = [
		AccountBuilder::new()
			.address(account_key)
			.owner(TEST_PROGRAM_ID)
			.lamports(1_000_000)
			.data(&state_data)
			.is_writable(true),
		AccountBuilder::new()
			.address(recipient_key)
			.owner(system::ID)
			.lamports(500_000)
			.is_writable(true),
	];

	let dummy_data: &[u8] = &[0u8];
	let mut input = unsafe { create_test_input(&accounts, dummy_data) };
	let mut accts = [UNINIT; 10];
	let (_, account_views, ..) = unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = account_views[0].close_with_recipient(&account_views[1]);
	assert!(result.is_ok(), "close should succeed: {result:?}");

	assert_eq!(
		account_views[0].lamports(),
		0,
		"closed account should have 0 lamports"
	);
	assert_eq!(
		account_views[1].lamports(),
		1_500_000,
		"recipient should have 1_500_000 lamports"
	);
}

// ---------------------------------------------------------------------------
// Test: AccountView validation chain
// ---------------------------------------------------------------------------

/// Tests various validation chains on AccountView.
#[test]
fn account_view_validation_chain() {
	let key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let state_bytes = build_test_state_bytes(5, 77);

	let accounts = [AccountBuilder::new()
		.address(key)
		.owner(TEST_PROGRAM_ID)
		.lamports(1_000_000)
		.data(&state_bytes)
		.is_signer(true)
		.is_writable(true)];

	let dummy_data: &[u8] = &[0u8];
	let mut input = unsafe { create_test_input(&accounts, dummy_data) };
	let mut accts = [UNINIT; 10];
	let (_, account_views, ..) = unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let account = &account_views[0];

	// Chain of successful assertions.
	let result = account
		.assert_signer()
		.and_then(|a| a.assert_writable())
		.and_then(|a| a.assert_not_empty())
		.and_then(|a| a.assert_owner(&TEST_PROGRAM_ID))
		.and_then(|a| a.assert_address(&key))
		.and_then(|a| a.assert_data_len(size_of::<TestState>()))
		.and_then(|a| a.assert_type::<TestState>(&TEST_PROGRAM_ID));

	assert!(
		result.is_ok(),
		"validation chain should succeed: {result:?}"
	);
}

/// Tests that validation chain short-circuits on first failure.
#[test]
fn account_view_validation_chain_short_circuits() {
	let key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let state_bytes = build_test_state_bytes(5, 77);

	// Account is NOT a signer.
	let accounts = [AccountBuilder::new()
		.address(key)
		.owner(TEST_PROGRAM_ID)
		.lamports(1_000_000)
		.data(&state_bytes)
		.is_signer(false) // <-- not a signer
		.is_writable(true)];

	let dummy_data: &[u8] = &[0u8];
	let mut input = unsafe { create_test_input(&accounts, dummy_data) };
	let mut accts = [UNINIT; 10];
	let (_, account_views, ..) = unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let account = &account_views[0];

	// Should fail at assert_signer, never reaching later assertions.
	let result = account
		.assert_signer()
		.and_then(|a| a.assert_writable())
		.and_then(|a| a.assert_owner(&TEST_PROGRAM_ID));

	assert!(result.is_err());
	assert_eq!(result.unwrap_err(), ProgramError::MissingRequiredSignature);
}

// ---------------------------------------------------------------------------
// Test: Account deserialization round-trips
// ---------------------------------------------------------------------------

/// Tests that account data can be written and read back through AccountView.
#[test]
fn account_data_roundtrip_through_account_view() {
	let key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let state_data = vec![0u8; size_of::<TestState>()];

	let accounts = [AccountBuilder::new()
		.address(key)
		.owner(TEST_PROGRAM_ID)
		.lamports(1_000_000)
		.data(&state_data)
		.is_writable(true)];

	let dummy_data: &[u8] = &[0u8];
	let mut input = unsafe { create_test_input(&accounts, dummy_data) };
	let mut accts = [UNINIT; 10];
	let (_, account_views, ..) = unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	// Write state directly to the raw account bytes (simulates initialization
	// of a freshly created account with zeroed data).
	{
		let new_state = TestState::builder()
			.bump(123)
			._padding(0)
			._padding2(0)
			.value(PodU64::from_primitive(u64::MAX))
			.build();
		let state_bytes = bytemuck::bytes_of(&new_state);
		let mut account_data = account_views[0]
			.try_borrow_mut()
			.unwrap_or_else(|e| panic!("borrow failed: {e:?}"));
		account_data[..state_bytes.len()].copy_from_slice(state_bytes);
	}

	// Read state back via as_account (discriminator is now valid).
	let state = account_views[0]
		.as_account::<TestState>(&TEST_PROGRAM_ID)
		.unwrap_or_else(|e| panic!("read failed: {e:?}"));
	assert_eq!(state.bump, 123);
	assert_eq!(u64::from(state.value), u64::MAX);
}

/// Tests that modifying state through as_account_mut persists.
#[test]
fn account_data_mutation_persists() {
	let key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let state_bytes = build_test_state_bytes(10, 500);

	let accounts = [AccountBuilder::new()
		.address(key)
		.owner(TEST_PROGRAM_ID)
		.lamports(1_000_000)
		.data(&state_bytes)
		.is_writable(true)];

	let dummy_data: &[u8] = &[0u8];
	let mut input = unsafe { create_test_input(&accounts, dummy_data) };
	let mut accts = [UNINIT; 10];
	let (_, account_views, ..) = unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	// Mutate value.
	{
		let state = account_views[0]
			.as_account_mut::<TestState>(&TEST_PROGRAM_ID)
			.unwrap_or_else(|e| panic!("write failed: {e:?}"));
		state.value = PodU64::from_primitive(12345);
	}

	// Verify persistence.
	let state = account_views[0]
		.as_account::<TestState>(&TEST_PROGRAM_ID)
		.unwrap_or_else(|e| panic!("read failed: {e:?}"));
	assert_eq!(u64::from(state.value), 12345);
	assert_eq!(state.bump, 10, "bump should be unchanged");
}

// ---------------------------------------------------------------------------
// Test: TryFromAccountInfos derive
// ---------------------------------------------------------------------------

/// Tests that TryFromAccountInfos correctly maps accounts to named fields.
#[test]
fn try_from_account_infos_maps_correctly() {
	let authority_key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let state_key: Address = address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki");

	let state_bytes = build_test_state_bytes(1, 100);

	let accounts = [
		AccountBuilder::new()
			.address(authority_key)
			.owner(system::ID)
			.lamports(5_000_000)
			.is_signer(true)
			.is_writable(true),
		AccountBuilder::new()
			.address(state_key)
			.owner(TEST_PROGRAM_ID)
			.lamports(890_880)
			.data(&state_bytes)
			.is_writable(true),
	];

	let dummy_data: &[u8] = &[0u8];
	let mut input = unsafe { create_test_input(&accounts, dummy_data) };
	let mut accts = [UNINIT; 10];
	let (_, account_views, ..) = unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let update_accounts = UpdateAccounts::try_from(account_views)
		.unwrap_or_else(|e| panic!("failed to deserialize accounts: {e:?}"));

	assert_eq!(
		update_accounts.authority.address(),
		&authority_key,
		"authority should match"
	);
	assert_eq!(
		update_accounts.state_account.address(),
		&state_key,
		"state_account should match"
	);
}

/// Tests that too many accounts triggers TooManyAccountKeys.
#[test]
fn try_from_account_infos_rejects_too_many() {
	let accounts = [
		AccountBuilder::new()
			.address(address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY"))
			.is_signer(true),
		AccountBuilder::new().address(address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki")),
		AccountBuilder::new().address(address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")),
	];

	let dummy_data: &[u8] = &[0u8];
	let mut input = unsafe { create_test_input(&accounts, dummy_data) };
	let mut accts = [UNINIT; 10];
	let (_, account_views, ..) = unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	// UpdateAccounts expects exactly 2 accounts; 3 should fail.
	let result = UpdateAccounts::try_from(account_views);
	assert!(result.is_err(), "should fail with too many accounts");
	assert!(
		result.is_err_and(|error| error.eq(&PinaProgramError::TooManyAccountKeys.into())),
		"error should be TooManyAccountKeys"
	);
}

// ---------------------------------------------------------------------------
// Test: PDA seed verification
// ---------------------------------------------------------------------------

/// Tests PDA derivation and verification round-trip (pure function tests,
/// no AccountView).
#[test]
fn pda_derive_and_verify_roundtrip() {
	let seeds: &[&[u8]] = &[b"test", b"pda"];
	let (pda, bump) = try_find_program_address(seeds, &TEST_PROGRAM_ID)
		.unwrap_or_else(|| panic!("should derive PDA"));

	// Verify round-trip via create_program_address.
	let bump_seed = [bump];
	let seeds_with_bump: &[&[u8]] = &[b"test", b"pda", &bump_seed];
	let recreated = create_program_address(seeds_with_bump, &TEST_PROGRAM_ID)
		.unwrap_or_else(|e| panic!("failed to recreate: {e:?}"));

	assert_eq!(pda, recreated, "PDA should match after round-trip");

	// Verify determinism.
	let (pda2, bump2) = try_find_program_address(seeds, &TEST_PROGRAM_ID)
		.unwrap_or_else(|| panic!("second derivation failed"));
	assert_eq!(pda, pda2, "PDA derivation should be deterministic");
	assert_eq!(bump, bump2, "bump should be deterministic");
}

/// Tests assert_seeds_with_bump on an AccountView whose address is a valid
/// PDA.
///
/// Note: `assert_seeds` / `assert_canonical_bump` internally call
/// `try_find_program_address`, which allocates a `Vec` on the heap during
/// iteration. On some native testing platforms this heap activity can
/// invalidate the raw pointer held by `AccountView` (which points into an
/// `AlignedMemory` test buffer). `assert_seeds_with_bump` uses
/// `create_program_address` instead, which does not iterate and has fewer
/// heap allocations, but still uses `sha2::Sha256` internally.
///
/// To avoid this issue entirely, we call `create_program_address` directly
/// (outside the AccountView) and compare the result manually, which
/// exercises the same validation logic without coupling PDA derivation to
/// the AccountView memory layout.
#[test]
fn pda_assert_seeds_with_bump_on_account_view() {
	let seeds: &[&[u8]] = &[b"view", b"test"];
	// Derive the PDA BEFORE creating the AccountView buffer.
	let (pda, bump) = try_find_program_address(seeds, &TEST_PROGRAM_ID)
		.unwrap_or_else(|| panic!("should derive PDA"));

	let bump_seed = [bump];
	let seeds_with_bump: &[&[u8]] = &[b"view", b"test", &bump_seed];

	let accounts = [AccountBuilder::new()
		.address(pda)
		.owner(TEST_PROGRAM_ID)
		.lamports(1_000_000)
		.is_writable(true)];

	let dummy_data: &[u8] = &[0u8];
	let mut input = unsafe { create_test_input(&accounts, dummy_data) };
	let mut accts = [UNINIT; 10];
	let (_, account_views, ..) = unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	// Verify address stored correctly.
	assert_eq!(
		account_views[0].address(),
		&pda,
		"account address should match the PDA"
	);

	// Verify the PDA round-trip using create_program_address. This exercises
	// the same code path as assert_seeds_with_bump.
	let recreated = create_program_address(seeds_with_bump, &TEST_PROGRAM_ID)
		.unwrap_or_else(|e| panic!("create_program_address failed: {e:?}"));
	assert_eq!(
		account_views[0].address(),
		&recreated,
		"AccountView address should match PDA from create_program_address"
	);

	// Also test assert_seeds_with_bump directly on the AccountView.
	let result = account_views[0].assert_seeds_with_bump(seeds_with_bump, &TEST_PROGRAM_ID);
	assert!(
		result.is_ok(),
		"assert_seeds_with_bump should pass: {result:?}"
	);

	// Test assert_seeds (which calls try_find_program_address internally).
	let result = account_views[0].assert_seeds(seeds, &TEST_PROGRAM_ID);
	assert!(result.is_ok(), "assert_seeds should pass: {result:?}");

	// Test assert_canonical_bump.
	let result_bump = account_views[0]
		.assert_canonical_bump(seeds, &TEST_PROGRAM_ID)
		.unwrap_or_else(|e| panic!("assert_canonical_bump failed: {e:?}"));
	assert_eq!(result_bump, bump, "canonical bump should match");
}

/// Tests that assert_seeds fails for a wrong address.
#[test]
fn pda_assert_seeds_rejects_wrong_address() {
	let seeds: &[&[u8]] = &[b"test", b"pda"];
	let wrong_address: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");

	let accounts = [AccountBuilder::new()
		.address(wrong_address)
		.owner(TEST_PROGRAM_ID)
		.lamports(1_000_000)
		.is_writable(true)];

	let dummy_data: &[u8] = &[0u8];
	let mut input = unsafe { create_test_input(&accounts, dummy_data) };
	let mut accts = [UNINIT; 10];
	let (_, account_views, ..) = unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = account_views[0].assert_seeds(seeds, &TEST_PROGRAM_ID);
	assert!(result.is_err(), "should fail with wrong address");
	assert_eq!(result.unwrap_err(), ProgramError::InvalidSeeds);
}

/// Tests that assert_canonical_bump returns the expected bump.
///
/// Note: `assert_canonical_bump` calls `try_find_program_address` internally
/// and compares the result against `self.address()`. To avoid memory layout
/// issues with `AccountView` and PDA derivation in tests, we test the raw
/// PDA derivation here and separately verify that AccountView addresses
/// are stored correctly (in `assert_address_succeeds`).
#[test]
fn pda_assert_canonical_bump() {
	let seeds: &[&[u8]] = &[b"canonical", b"bump"];
	let (pda, expected_bump) = try_find_program_address(seeds, &TEST_PROGRAM_ID)
		.unwrap_or_else(|| panic!("should derive PDA"));

	// The bump is always a valid u8 by type.

	// Verify the PDA is not on the ed25519 curve (which is the point of
	// PDAs).
	let bump_seed = [expected_bump];
	let seeds_with_bump: &[&[u8]] = &[b"canonical", b"bump", &bump_seed];
	let recreated = create_program_address(seeds_with_bump, &TEST_PROGRAM_ID)
		.unwrap_or_else(|e| panic!("failed to recreate with bump: {e:?}"));
	assert_eq!(pda, recreated, "PDA should match with canonical bump");

	// Verify that a non-canonical bump (expected_bump - 1, if > 0)
	// gives a different PDA.
	if expected_bump > 0 {
		let non_canonical_bump = [expected_bump - 1];
		let non_canonical_seeds: &[&[u8]] = &[b"canonical", b"bump", &non_canonical_bump];
		// create_program_address may succeed or fail for non-canonical bumps.
		if let Ok(other_pda) = create_program_address(non_canonical_seeds, &TEST_PROGRAM_ID) {
			assert_ne!(
				pda, other_pda,
				"non-canonical bump should produce a different PDA"
			);
		}
	}
}

// ---------------------------------------------------------------------------
// Test: Discriminator dispatch
// ---------------------------------------------------------------------------

/// Tests that instruction discriminators dispatch correctly through
/// parse_instruction.
#[test]
fn discriminator_dispatch_all_variants() {
	for (byte, expected_name) in [(0u8, "Initialize"), (1u8, "Update"), (2u8, "Close")] {
		let data = [byte];
		let result: TestInstruction = parse_instruction(&TEST_PROGRAM_ID, &TEST_PROGRAM_ID, &data)
			.unwrap_or_else(|e| panic!("parse variant {expected_name} failed: {e:?}"));

		match (byte, result) {
			(0, TestInstruction::Initialize) => {}
			(1, TestInstruction::Update) => {}
			(2, TestInstruction::Close) => {}
			_ => panic!("unexpected dispatch for byte {byte}"),
		}
	}
}

/// Tests that HasDiscriminator::matches_discriminator works for account types.
#[test]
fn has_discriminator_matches_for_account_type() {
	assert!(TestState::matches_discriminator(&[
		TestAccountType::TestState as u8
	]));
	assert!(!TestState::matches_discriminator(&[0u8]));
	assert!(!TestState::matches_discriminator(&[99u8]));
	assert!(!TestState::matches_discriminator(&[]));
}

// ---------------------------------------------------------------------------
// Test: assert_address and assert_addresses
// ---------------------------------------------------------------------------

/// Tests assert_address succeeds for matching address.
#[test]
fn assert_address_succeeds() {
	let key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");

	let accounts = [AccountBuilder::new().address(key)];
	let dummy_data: &[u8] = &[0u8];
	let mut input = unsafe { create_test_input(&accounts, dummy_data) };
	let mut accts = [UNINIT; 10];
	let (_, account_views, ..) = unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = account_views[0].assert_address(&key);
	assert!(result.is_ok());
}

/// Tests assert_address fails for non-matching address.
#[test]
fn assert_address_fails_for_wrong_address() {
	let key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let wrong: Address = address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki");

	let accounts = [AccountBuilder::new().address(key)];
	let dummy_data: &[u8] = &[0u8];
	let mut input = unsafe { create_test_input(&accounts, dummy_data) };
	let mut accts = [UNINIT; 10];
	let (_, account_views, ..) = unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = account_views[0].assert_address(&wrong);
	assert!(result.is_err());
	assert_eq!(result.unwrap_err(), ProgramError::InvalidAccountData);
}

/// Tests assert_addresses succeeds when account matches one of the addresses.
#[test]
fn assert_addresses_succeeds_for_matching() {
	let key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let other: Address = address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki");

	let accounts = [AccountBuilder::new().address(key)];
	let dummy_data: &[u8] = &[0u8];
	let mut input = unsafe { create_test_input(&accounts, dummy_data) };
	let mut accts = [UNINIT; 10];
	let (_, account_views, ..) = unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = account_views[0].assert_addresses(&[other, key]);
	assert!(result.is_ok());
}

/// Tests assert_addresses fails when account matches none of the addresses.
#[test]
fn assert_addresses_fails_for_no_match() {
	let key: Address = address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
	let other1: Address = address!("3Jiy8N6ZGv3ueH9k3svLRaHscmQbE6v7W9FHJaGH2mki");
	let other2: Address = address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

	let accounts = [AccountBuilder::new().address(key)];
	let dummy_data: &[u8] = &[0u8];
	let mut input = unsafe { create_test_input(&accounts, dummy_data) };
	let mut accts = [UNINIT; 10];
	let (_, account_views, ..) = unsafe { deserialize_test_input::<10>(&mut input, &mut accts) };

	let result = account_views[0].assert_addresses(&[other1, other2]);
	assert!(result.is_err());
	assert_eq!(result.unwrap_err(), ProgramError::InvalidAccountData);
}
