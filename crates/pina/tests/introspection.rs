//! Tests for `pina::introspection` helpers.
//!
//! These tests construct fake Instructions sysvar account data following the
//! exact binary layout that pinocchio's `Instructions` parser expects, then
//! exercise each introspection function end-to-end.
//!
//! ## Sysvar binary layout
//!
//! ```text
//! u16 LE  num_instructions
//! [u16 LE; N]  offset table (byte offset of each instruction from buffer start)
//! instruction 0..N-1:
//!     u16 LE  num_accounts
//!     [account; num_accounts]:
//!         u8       flags (bit 0 = signer, bit 1 = writable)
//!         [u8; 32] account key
//!     [u8; 32] program_id
//!     u16 LE   data_len
//!     [u8; data_len] instruction data
//! u16 LE  current_instruction_index  (last 2 bytes of buffer)
//! ```

#![allow(unsafe_code, dead_code)]

use core::alloc::Layout;
use core::mem::MaybeUninit;
use core::mem::size_of;
use core::ptr::copy_nonoverlapping;
use std::alloc::alloc;
use std::alloc::dealloc;

use pina::Address;
use pina::introspection::assert_no_cpi;
use pina::introspection::get_current_instruction_index;
use pina::introspection::get_instruction_count;
use pina::introspection::has_instruction_after;
use pina::introspection::has_instruction_before;
use pina::pinocchio::AccountView;
use pina::pinocchio::account::MAX_PERMITTED_DATA_INCREASE;
use pina::pinocchio::entrypoint;
use pina::pinocchio::error::ProgramError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const BPF_ALIGN: usize = 8;
const STATIC_ACCOUNT_DATA: usize = 88 + MAX_PERMITTED_DATA_INCREASE;

/// Instructions sysvar address.
const INSTRUCTIONS_SYSVAR_ID: Address =
	pina::address!("Sysvar1nstructions1111111111111111111111111");

/// Sysvar owner (the native sysvar program).
const SYSVAR_OWNER: Address = pina::address!("Sysvar1111111111111111111111111111111111111");

/// Fake program IDs for testing.
const PROGRAM_A: Address = pina::address!("6UkV1fMrN1Fuf5kJCbSEkmy5Tapenny9CLYq3LRacQ9");
const PROGRAM_B: Address = pina::address!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");
const PROGRAM_C: Address = pina::address!("11111111111111111111111111111111");

// ---------------------------------------------------------------------------
// Sysvar data builder
// ---------------------------------------------------------------------------

/// A fake instruction used to build the Instructions sysvar buffer.
struct FakeInstruction {
	program_id: Address,
	accounts: Vec<(Address, bool, bool)>, // (key, is_signer, is_writable)
	data: Vec<u8>,
}

impl FakeInstruction {
	fn simple(program_id: Address) -> Self {
		Self {
			program_id,
			accounts: vec![],
			data: vec![],
		}
	}

	fn with_data(program_id: Address, data: &[u8]) -> Self {
		Self {
			program_id,
			accounts: vec![],
			data: data.to_vec(),
		}
	}
}

/// Build the serialized Instructions sysvar data buffer.
fn build_sysvar_data(instructions: &[FakeInstruction], current_index: u16) -> Vec<u8> {
	let num_ix = instructions.len() as u16;
	let mut buf = Vec::new();

	// u16: num_instructions
	buf.extend_from_slice(&num_ix.to_le_bytes());

	// Offset table placeholder (u16 per instruction)
	let offset_table_pos = buf.len();
	for _ in 0..num_ix {
		buf.extend_from_slice(&0u16.to_le_bytes());
	}

	// Serialize each instruction
	for (i, ix) in instructions.iter().enumerate() {
		let offset = buf.len() as u16;
		// Backfill offset in the table
		buf[offset_table_pos + i * 2..offset_table_pos + i * 2 + 2]
			.copy_from_slice(&offset.to_le_bytes());

		// u16: num_accounts
		buf.extend_from_slice(&(ix.accounts.len() as u16).to_le_bytes());

		// Each account: u8 flags + [u8; 32] key
		for (key, is_signer, is_writable) in &ix.accounts {
			let flags = (*is_signer as u8) | ((*is_writable as u8) << 1);
			buf.push(flags);
			buf.extend_from_slice(key.as_ref());
		}

		// [u8; 32]: program_id
		buf.extend_from_slice(ix.program_id.as_ref());

		// u16: data_len + data bytes
		buf.extend_from_slice(&(ix.data.len() as u16).to_le_bytes());
		buf.extend_from_slice(&ix.data);
	}

	// u16: current_instruction_index (last 2 bytes)
	buf.extend_from_slice(&current_index.to_le_bytes());

	buf
}

// ---------------------------------------------------------------------------
// SVM input buffer helpers (adapted from integration.rs)
// ---------------------------------------------------------------------------

struct AlignedMemory {
	ptr: *mut u8,
	layout: Layout,
}

impl AlignedMemory {
	fn new(len: usize) -> Self {
		let layout =
			Layout::from_size_align(len, BPF_ALIGN).unwrap_or_else(|e| panic!("layout: {e:?}"));
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
		unsafe { dealloc(self.ptr, self.layout) }
	}
}

struct AccountBuilder {
	address: Address,
	owner: Address,
	lamports: u64,
	data: Vec<u8>,
}

impl AccountBuilder {
	fn sysvar(data: Vec<u8>) -> Self {
		Self {
			address: INSTRUCTIONS_SYSVAR_ID,
			owner: SYSVAR_OWNER,
			lamports: 1,
			data,
		}
	}
}

/// Compute buffer size for a single-account input.
fn compute_input_size(data_len: usize, ix_data: &[u8]) -> usize {
	let mut size = size_of::<u64>(); // num accounts
	let account_buf_size = STATIC_ACCOUNT_DATA + size_of::<u64>();
	size += account_buf_size;
	let padding = (data_len + (BPF_ALIGN - 1)) & !(BPF_ALIGN - 1);
	size += padding;
	size += size_of::<u64>(); // ix data len
	size += ix_data.len();
	size += 32; // program id
	size
}

/// Create a serialized SVM input buffer with a single sysvar account.
unsafe fn create_sysvar_input(builder: &AccountBuilder, ix_data: &[u8]) -> AlignedMemory {
	let data_len = builder.data.len();
	let total_size = compute_input_size(data_len, ix_data);
	let mut input = AlignedMemory::new(total_size);

	// 1 account
	unsafe { input.write(&1u64.to_le_bytes(), 0) };
	let mut offset = size_of::<u64>();

	let account_buf_size = STATIC_ACCOUNT_DATA + size_of::<u64>();
	let mut account_buf = vec![0u8; account_buf_size];

	account_buf[0] = entrypoint::NON_DUP_MARKER; // borrow_state
	account_buf[1] = 0; // not signer
	account_buf[2] = 0; // not writable
	account_buf[3] = 0; // not executable
	account_buf[8..40].copy_from_slice(builder.address.as_ref());
	account_buf[40..72].copy_from_slice(builder.owner.as_ref());
	account_buf[72..80].copy_from_slice(&builder.lamports.to_le_bytes());
	account_buf[80..88].copy_from_slice(&(data_len as u64).to_le_bytes());
	if !builder.data.is_empty() {
		account_buf[88..88 + data_len].copy_from_slice(&builder.data);
	}

	unsafe { input.write(&account_buf, offset) };
	offset += account_buf_size;

	let padding = (data_len + (BPF_ALIGN - 1)) & !(BPF_ALIGN - 1);
	if padding > 0 {
		unsafe { input.write(&vec![0u8; padding], offset) };
		offset += padding;
	}

	// Instruction data
	unsafe { input.write(&ix_data.len().to_le_bytes(), offset) };
	offset += size_of::<u64>();
	unsafe { input.write(ix_data, offset) };
	offset += ix_data.len();

	// Program ID (dummy)
	unsafe { input.write(PROGRAM_A.as_ref(), offset) };

	input
}

const UNINIT: MaybeUninit<AccountView> = MaybeUninit::<AccountView>::uninit();

/// Deserialize a single-account input.
unsafe fn deserialize_input(
	input: &mut AlignedMemory,
	accounts: &mut [MaybeUninit<AccountView>; 1],
) -> &'static AccountView {
	let (_program_id, count, _ix_data) =
		unsafe { entrypoint::deserialize::<1>(input.as_mut_ptr(), accounts) };
	assert_eq!(count, 1, "expected 1 account");
	unsafe { &*accounts[0].as_ptr() }
}

// ---------------------------------------------------------------------------
// Helper: build sysvar AccountView
// ---------------------------------------------------------------------------

/// Construct a sysvar AccountView from fake instructions.
///
/// Returns (input, accounts) which must be kept alive.
/// Use the returned AccountView reference for testing.
macro_rules! sysvar_account {
	($instructions:expr, $current_index:expr) => {{
		let sysvar_data = build_sysvar_data($instructions, $current_index);
		let builder = AccountBuilder::sysvar(sysvar_data);
		let mut input = unsafe { create_sysvar_input(&builder, &[]) };
		let mut accounts = [UNINIT];
		let account = unsafe { deserialize_input(&mut input, &mut accounts) };
		(input, accounts, account)
	}};
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn get_instruction_count_single() {
	let sysvar_data = build_sysvar_data(&[FakeInstruction::simple(PROGRAM_A)], 0);
	let builder = AccountBuilder::sysvar(sysvar_data);
	let mut input = unsafe { create_sysvar_input(&builder, &[]) };
	let mut accounts = [UNINIT];
	let account = unsafe { deserialize_input(&mut input, &mut accounts) };

	let count = get_instruction_count(account)
		.unwrap_or_else(|e| panic!("get_instruction_count failed: {e:?}"));
	assert_eq!(count, 1);
}

#[test]
fn get_instruction_count_multiple() {
	let instructions = vec![
		FakeInstruction::simple(PROGRAM_A),
		FakeInstruction::simple(PROGRAM_B),
		FakeInstruction::simple(PROGRAM_C),
	];
	let sysvar_data = build_sysvar_data(&instructions, 1);
	let builder = AccountBuilder::sysvar(sysvar_data);
	let mut input = unsafe { create_sysvar_input(&builder, &[]) };
	let mut accounts = [UNINIT];
	let account = unsafe { deserialize_input(&mut input, &mut accounts) };

	let count = get_instruction_count(account)
		.unwrap_or_else(|e| panic!("get_instruction_count failed: {e:?}"));
	assert_eq!(count, 3);
}

#[test]
fn get_current_instruction_index_returns_correct_value() {
	let instructions = vec![
		FakeInstruction::simple(PROGRAM_A),
		FakeInstruction::simple(PROGRAM_B),
		FakeInstruction::simple(PROGRAM_C),
	];
	let sysvar_data = build_sysvar_data(&instructions, 2);
	let builder = AccountBuilder::sysvar(sysvar_data);
	let mut input = unsafe { create_sysvar_input(&builder, &[]) };
	let mut accounts = [UNINIT];
	let account = unsafe { deserialize_input(&mut input, &mut accounts) };

	let index = get_current_instruction_index(account)
		.unwrap_or_else(|e| panic!("get_current_instruction_index failed: {e:?}"));
	assert_eq!(index, 2);
}

#[test]
fn assert_no_cpi_passes_when_top_level() {
	let instructions = vec![FakeInstruction::simple(PROGRAM_A)];
	let sysvar_data = build_sysvar_data(&instructions, 0);
	let builder = AccountBuilder::sysvar(sysvar_data);
	let mut input = unsafe { create_sysvar_input(&builder, &[]) };
	let mut accounts = [UNINIT];
	let account = unsafe { deserialize_input(&mut input, &mut accounts) };

	assert_no_cpi(account, &PROGRAM_A)
		.unwrap_or_else(|e| panic!("assert_no_cpi should pass for top-level call: {e:?}"));
}

#[test]
fn assert_no_cpi_fails_when_program_id_mismatch() {
	// Current instruction is PROGRAM_A, but we claim to be PROGRAM_B
	let instructions = vec![FakeInstruction::simple(PROGRAM_A)];
	let sysvar_data = build_sysvar_data(&instructions, 0);
	let builder = AccountBuilder::sysvar(sysvar_data);
	let mut input = unsafe { create_sysvar_input(&builder, &[]) };
	let mut accounts = [UNINIT];
	let account = unsafe { deserialize_input(&mut input, &mut accounts) };

	let result = assert_no_cpi(account, &PROGRAM_B);
	assert_eq!(result, Err(ProgramError::InvalidAccountData));
}

#[test]
fn assert_no_cpi_checks_correct_index() {
	// 3 instructions: [A, B, C], current=1 → B is the current program
	let instructions = vec![
		FakeInstruction::simple(PROGRAM_A),
		FakeInstruction::simple(PROGRAM_B),
		FakeInstruction::simple(PROGRAM_C),
	];
	let sysvar_data = build_sysvar_data(&instructions, 1);
	let builder = AccountBuilder::sysvar(sysvar_data);
	let mut input = unsafe { create_sysvar_input(&builder, &[]) };
	let mut accounts = [UNINIT];
	let account = unsafe { deserialize_input(&mut input, &mut accounts) };

	// B is at index 1, so this should pass
	assert_no_cpi(account, &PROGRAM_B)
		.unwrap_or_else(|e| panic!("should pass for PROGRAM_B at index 1: {e:?}"));

	// A is NOT at index 1, so this should fail
	let result = assert_no_cpi(account, &PROGRAM_A);
	assert_eq!(result, Err(ProgramError::InvalidAccountData));
}

#[test]
fn has_instruction_before_finds_earlier_program() {
	// [A, B, C], current=2 → both A and B are before C
	let instructions = vec![
		FakeInstruction::simple(PROGRAM_A),
		FakeInstruction::simple(PROGRAM_B),
		FakeInstruction::simple(PROGRAM_C),
	];
	let sysvar_data = build_sysvar_data(&instructions, 2);
	let builder = AccountBuilder::sysvar(sysvar_data);
	let mut input = unsafe { create_sysvar_input(&builder, &[]) };
	let mut accounts = [UNINIT];
	let account = unsafe { deserialize_input(&mut input, &mut accounts) };

	assert!(
		has_instruction_before(account, &PROGRAM_A).unwrap_or_else(|e| panic!("failed: {e:?}"))
	);
	assert!(
		has_instruction_before(account, &PROGRAM_B).unwrap_or_else(|e| panic!("failed: {e:?}"))
	);
	// C is at the current index, not before
	assert!(
		!has_instruction_before(account, &PROGRAM_C).unwrap_or_else(|e| panic!("failed: {e:?}"))
	);
}

#[test]
fn has_instruction_before_returns_false_when_first() {
	// [A, B], current=0 → nothing before A
	let instructions = vec![
		FakeInstruction::simple(PROGRAM_A),
		FakeInstruction::simple(PROGRAM_B),
	];
	let sysvar_data = build_sysvar_data(&instructions, 0);
	let builder = AccountBuilder::sysvar(sysvar_data);
	let mut input = unsafe { create_sysvar_input(&builder, &[]) };
	let mut accounts = [UNINIT];
	let account = unsafe { deserialize_input(&mut input, &mut accounts) };

	assert!(
		!has_instruction_before(account, &PROGRAM_A).unwrap_or_else(|e| panic!("failed: {e:?}"))
	);
	assert!(
		!has_instruction_before(account, &PROGRAM_B).unwrap_or_else(|e| panic!("failed: {e:?}"))
	);
}

#[test]
fn has_instruction_after_finds_later_program() {
	// [A, B, C], current=0 → both B and C are after A
	let instructions = vec![
		FakeInstruction::simple(PROGRAM_A),
		FakeInstruction::simple(PROGRAM_B),
		FakeInstruction::simple(PROGRAM_C),
	];
	let sysvar_data = build_sysvar_data(&instructions, 0);
	let builder = AccountBuilder::sysvar(sysvar_data);
	let mut input = unsafe { create_sysvar_input(&builder, &[]) };
	let mut accounts = [UNINIT];
	let account = unsafe { deserialize_input(&mut input, &mut accounts) };

	assert!(has_instruction_after(account, &PROGRAM_B).unwrap_or_else(|e| panic!("failed: {e:?}")));
	assert!(has_instruction_after(account, &PROGRAM_C).unwrap_or_else(|e| panic!("failed: {e:?}")));
	// A is at current index, not after
	assert!(
		!has_instruction_after(account, &PROGRAM_A).unwrap_or_else(|e| panic!("failed: {e:?}"))
	);
}

#[test]
fn has_instruction_after_returns_false_when_last() {
	// [A, B], current=1 → nothing after B
	let instructions = vec![
		FakeInstruction::simple(PROGRAM_A),
		FakeInstruction::simple(PROGRAM_B),
	];
	let sysvar_data = build_sysvar_data(&instructions, 1);
	let builder = AccountBuilder::sysvar(sysvar_data);
	let mut input = unsafe { create_sysvar_input(&builder, &[]) };
	let mut accounts = [UNINIT];
	let account = unsafe { deserialize_input(&mut input, &mut accounts) };

	assert!(
		!has_instruction_after(account, &PROGRAM_A).unwrap_or_else(|e| panic!("failed: {e:?}"))
	);
	assert!(
		!has_instruction_after(account, &PROGRAM_B).unwrap_or_else(|e| panic!("failed: {e:?}"))
	);
}

#[test]
fn instructions_with_accounts_and_data() {
	// Test that instructions with actual account metas and data serialize correctly
	let account_key = pina::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
	let instructions = vec![FakeInstruction {
		program_id: PROGRAM_A,
		accounts: vec![(account_key, true, true)],
		data: vec![0x42, 0x43],
	}];
	let sysvar_data = build_sysvar_data(&instructions, 0);
	let builder = AccountBuilder::sysvar(sysvar_data);
	let mut input = unsafe { create_sysvar_input(&builder, &[]) };
	let mut accounts = [UNINIT];
	let account = unsafe { deserialize_input(&mut input, &mut accounts) };

	// Should still be able to read the instruction count and program ID
	let count = get_instruction_count(account).unwrap_or_else(|e| panic!("failed: {e:?}"));
	assert_eq!(count, 1);

	assert_no_cpi(account, &PROGRAM_A)
		.unwrap_or_else(|e| panic!("assert_no_cpi should pass: {e:?}"));
}

#[test]
fn wrong_sysvar_address_rejected() {
	// Build valid sysvar data but put it in a non-sysvar account
	let sysvar_data = build_sysvar_data(&[FakeInstruction::simple(PROGRAM_A)], 0);
	let builder = AccountBuilder {
		address: PROGRAM_A, // wrong address — not the sysvar ID
		owner: SYSVAR_OWNER,
		lamports: 1,
		data: sysvar_data,
	};
	let mut input = unsafe { create_sysvar_input(&builder, &[]) };
	let mut accounts = [UNINIT];
	let account = unsafe { deserialize_input(&mut input, &mut accounts) };

	let result = get_instruction_count(account);
	assert_eq!(result, Err(ProgramError::UnsupportedSysvar));
}
