#![allow(unsafe_code)]

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
use pinocchio::entrypoint;

const TEST_PROGRAM_ID: Address = address!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");
const BPF_ALIGN_OF_U128: usize = 8;
const UNINIT: MaybeUninit<AccountView> = MaybeUninit::<AccountView>::uninit();
const STATIC_ACCOUNT_DATA: usize = 88 + MAX_PERMITTED_DATA_INCREASE;

#[discriminator(crate = ::pina)]
pub enum TestAccountKind {
	BalanceState = 1,
}

#[account(crate = ::pina, discriminator = TestAccountKind)]
pub struct BalanceState {
	pub amount: PodU64,
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
struct DuplicateMutablePair<'a> {
	pub source: &'a AccountView,
	pub destination: &'a AccountView,
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
struct RemainingPassthrough<'a> {
	pub first: &'a AccountView,
	#[pina(remaining)]
	pub rest: &'a [AccountView],
}

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

	fn is_writable(mut self, is_writable: bool) -> Self {
		self.is_writable = is_writable;
		self
	}

	fn executable(mut self, executable: bool) -> Self {
		self.executable = executable;
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
			.unwrap_or_else(|error| panic!("invalid layout: {error:?}"));
		unsafe {
			let ptr = alloc(layout);
			if ptr.is_null() {
				std::alloc::handle_alloc_error(layout);
			}

			Self { ptr, layout }
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

fn build_balance_state_bytes(amount: u64) -> Vec<u8> {
	let state = BalanceState::builder()
		.amount(PodU64::from_primitive(amount))
		.build();

	bytemuck::bytes_of(&state).to_vec()
}

fn fake_address(byte: u8) -> Address {
	Address::new_from_array([byte; ADDRESS_BYTES])
}

fn align_to_bpf(data_len: usize) -> usize {
	(data_len + (BPF_ALIGN_OF_U128 - 1)) & !(BPF_ALIGN_OF_U128 - 1)
}

fn compute_input_size(
	unique_accounts: &[AccountBuilder],
	duplicate_count: usize,
	instruction_data: &[u8],
) -> usize {
	let mut size = size_of::<u64>();

	for builder in unique_accounts {
		size += STATIC_ACCOUNT_DATA + size_of::<u64>();
		size += align_to_bpf(builder.data.len());
	}

	size += duplicate_count * size_of::<u64>();
	size += size_of::<u64>();
	size += instruction_data.len();
	size += ADDRESS_BYTES;

	size
}

unsafe fn create_test_input(
	unique_accounts: &[AccountBuilder],
	duplicate_count: usize,
	instruction_data: &[u8],
) -> AlignedMemory {
	assert!(
		duplicate_count == 0 || !unique_accounts.is_empty(),
		"duplicate accounts require at least one unique account"
	);

	let total_accounts = unique_accounts.len() + duplicate_count;
	let total_size = compute_input_size(unique_accounts, duplicate_count, instruction_data);
	let mut input = AlignedMemory::new(total_size);

	unsafe {
		input.write(&(total_accounts as u64).to_le_bytes(), 0);
	}
	let mut offset = size_of::<u64>();

	for builder in unique_accounts {
		let data_len = builder.data.len();
		let account_buf_size = STATIC_ACCOUNT_DATA + size_of::<u64>();
		let mut account_buf = vec![0u8; account_buf_size];

		account_buf[0] = entrypoint::NON_DUP_MARKER;
		account_buf[1] = u8::from(builder.is_signer);
		account_buf[2] = u8::from(builder.is_writable);
		account_buf[3] = u8::from(builder.executable);
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

		let padding = align_to_bpf(data_len);
		if padding > 0 {
			unsafe {
				input.write(&vec![0u8; padding], offset);
			}
			offset += padding;
		}
	}

	if duplicate_count > 0 {
		let duplicate_index = (unique_accounts.len() - 1) as u8;
		for _ in 0..duplicate_count {
			unsafe {
				input.write(&[duplicate_index, 0, 0, 0, 0, 0, 0, 0], offset);
			}
			offset += size_of::<u64>();
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

macro_rules! load_accounts {
	($unique_accounts:expr, $duplicate_count:expr, $max_accounts:expr) => {{
		let mut input = unsafe { create_test_input($unique_accounts, $duplicate_count, &[]) };
		let mut accounts = [UNINIT; $max_accounts];
		let (account_views, _) =
			unsafe { deserialize_test_input::<$max_accounts>(&mut input, &mut accounts) };

		(input, accounts, account_views)
	}};
}

fn assert_distinct_mutable_accounts(
	left: &AccountView,
	right: &AccountView,
) -> Result<(), ProgramError> {
	if left.address() == right.address() {
		return Err(ProgramError::InvalidArgument);
	}

	Ok(())
}

fn find_non_canonical_pda_fixture() -> ([u8; 2], Address, u8, Address, u8) {
	for counter in 0u16..=u16::MAX {
		let seed_bytes = counter.to_le_bytes();
		let seeds: &[&[u8]] = &[b"noncanonical", &seed_bytes];
		let (canonical_address, canonical_bump) = try_find_program_address(seeds, &TEST_PROGRAM_ID)
			.unwrap_or_else(|| panic!("should derive canonical PDA for counter {counter}"));

		for non_canonical_bump in 0..canonical_bump {
			let bump_seed = [non_canonical_bump];
			let seeds_with_bump: &[&[u8]] = &[b"noncanonical", &seed_bytes, &bump_seed];
			if let Ok(non_canonical_address) =
				create_program_address(seeds_with_bump, &TEST_PROGRAM_ID)
			{
				if non_canonical_address != canonical_address {
					return (
						seed_bytes,
						canonical_address,
						canonical_bump,
						non_canonical_address,
						non_canonical_bump,
					);
				}
			}
		}
	}

	panic!("could not find a non-canonical PDA fixture")
}

#[cfg(feature = "token")]
fn write_address_bytes(data: &mut [u8], offset: usize, address: &Address) {
	data[offset..offset + ADDRESS_BYTES].copy_from_slice(address.as_ref());
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
fn duplicate_mutable_accounts_share_runtime_borrow_state() {
	let shared_key = fake_address(11);
	let state_bytes = build_balance_state_bytes(7);
	let unique_accounts = [AccountBuilder::new()
		.address(shared_key)
		.owner(TEST_PROGRAM_ID)
		.lamports(1_000)
		.data(&state_bytes)
		.is_writable(true)];

	let (_input, _accounts, account_views) = load_accounts!(&unique_accounts, 1, 4);
	let pair = DuplicateMutablePair::try_from(account_views)
		.unwrap_or_else(|error| panic!("failed to derive duplicate pair: {error:?}"));

	assert_eq!(pair.source, pair.destination);
	assert_eq!(pair.source.address(), pair.destination.address());
	assert!(matches!(
		assert_distinct_mutable_accounts(pair.source, pair.destination),
		Err(ProgramError::InvalidArgument)
	));

	let borrowed = pair
		.source
		.try_borrow_mut()
		.unwrap_or_else(|error| panic!("expected first mutable borrow to succeed: {error:?}"));
	assert!(matches!(
		pair.destination.try_borrow_mut(),
		Err(ProgramError::AccountBorrowFailed)
	));
	assert!(matches!(
		pair.destination.try_borrow(),
		Err(ProgramError::AccountBorrowFailed)
	));
	assert_eq!(borrowed.len(), size_of::<BalanceState>());
}

#[test]
fn remaining_accounts_preserve_duplicate_order_and_aliasing() {
	let first_key = fake_address(12);
	let duplicated_key = fake_address(13);
	let unique_accounts = [
		AccountBuilder::new()
			.address(first_key)
			.owner(TEST_PROGRAM_ID)
			.lamports(10)
			.is_writable(true),
		AccountBuilder::new()
			.address(duplicated_key)
			.owner(TEST_PROGRAM_ID)
			.lamports(20)
			.data(&build_balance_state_bytes(11))
			.is_writable(true),
	];

	let (_input, _accounts, account_views) = load_accounts!(&unique_accounts, 2, 6);
	let parsed = RemainingPassthrough::try_from(account_views)
		.unwrap_or_else(|error| panic!("failed to derive remaining accounts: {error:?}"));

	assert_eq!(parsed.first.address(), &first_key);
	assert_eq!(parsed.rest.len(), 3);
	assert_eq!(parsed.rest[0].address(), &duplicated_key);
	assert_eq!(parsed.rest[1], parsed.rest[0]);
	assert_eq!(parsed.rest[2], parsed.rest[0]);

	let duplicated_borrow = parsed.rest[1]
		.try_borrow_mut()
		.unwrap_or_else(|error| panic!("duplicate borrow should succeed once: {error:?}"));
	assert!(matches!(
		parsed.rest[0].try_borrow_mut(),
		Err(ProgramError::AccountBorrowFailed)
	));
	assert!(matches!(
		parsed.rest[2].try_borrow_mut(),
		Err(ProgramError::AccountBorrowFailed)
	));
	drop(duplicated_borrow);
}

#[test]
fn send_conserves_lamports_on_success() {
	let unique_accounts = [
		AccountBuilder::new()
			.address(fake_address(14))
			.owner(TEST_PROGRAM_ID)
			.lamports(400)
			.is_writable(true),
		AccountBuilder::new()
			.address(fake_address(15))
			.owner(TEST_PROGRAM_ID)
			.lamports(600)
			.is_writable(true),
	];

	let (_input, _accounts, account_views) = load_accounts!(&unique_accounts, 0, 4);
	let total_before = account_views[0].lamports() + account_views[1].lamports();

	account_views[0]
		.send(125, &account_views[1])
		.unwrap_or_else(|error| panic!("send should succeed: {error:?}"));

	let total_after = account_views[0].lamports() + account_views[1].lamports();
	assert_eq!(account_views[0].lamports(), 275);
	assert_eq!(account_views[1].lamports(), 725);
	assert_eq!(total_after, total_before);
}

#[test]
fn send_overflow_preserves_balances() {
	let unique_accounts = [
		AccountBuilder::new()
			.address(fake_address(16))
			.owner(TEST_PROGRAM_ID)
			.lamports(25)
			.is_writable(true),
		AccountBuilder::new()
			.address(fake_address(17))
			.owner(TEST_PROGRAM_ID)
			.lamports(u64::MAX)
			.is_writable(true),
	];

	let (_input, _accounts, account_views) = load_accounts!(&unique_accounts, 0, 4);
	let sender_before = account_views[0].lamports();
	let recipient_before = account_views[1].lamports();

	let result = account_views[0].send(1, &account_views[1]);
	assert_eq!(result, Err(ProgramError::ArithmeticOverflow));
	assert_eq!(account_views[0].lamports(), sender_before);
	assert_eq!(account_views[1].lamports(), recipient_before);
}

#[test]
fn close_with_recipient_conserves_lamports_and_zeroes_source() {
	let state_bytes = build_balance_state_bytes(33);
	let unique_accounts = [
		AccountBuilder::new()
			.address(fake_address(18))
			.owner(TEST_PROGRAM_ID)
			.lamports(700)
			.data(&state_bytes)
			.is_writable(true),
		AccountBuilder::new()
			.address(fake_address(19))
			.owner(system::ID)
			.lamports(300)
			.is_writable(true),
	];

	let (_input, _accounts, account_views) = load_accounts!(&unique_accounts, 0, 4);
	let total_before = account_views[0].lamports() + account_views[1].lamports();

	account_views[0]
		.close_with_recipient(&account_views[1])
		.unwrap_or_else(|error| panic!("close should succeed: {error:?}"));

	let total_after = account_views[0].lamports() + account_views[1].lamports();
	assert_eq!(account_views[0].lamports(), 0);
	assert_eq!(account_views[0].data_len(), 0);
	assert!(account_views[0].is_data_empty());
	assert_eq!(account_views[1].lamports(), 1_000);
	assert_eq!(total_after, total_before);
}

#[test]
fn close_with_recipient_overflow_preserves_balances_and_data() {
	let state_bytes = build_balance_state_bytes(44);
	let unique_accounts = [
		AccountBuilder::new()
			.address(fake_address(20))
			.owner(TEST_PROGRAM_ID)
			.lamports(1)
			.data(&state_bytes)
			.is_writable(true),
		AccountBuilder::new()
			.address(fake_address(21))
			.owner(system::ID)
			.lamports(u64::MAX)
			.is_writable(true),
	];

	let (_input, _accounts, account_views) = load_accounts!(&unique_accounts, 0, 4);
	let source_before = account_views[0].lamports();
	let recipient_before = account_views[1].lamports();
	let data_len_before = account_views[0].data_len();

	let result = account_views[0].close_with_recipient(&account_views[1]);
	assert_eq!(result, Err(ProgramError::ArithmeticOverflow));
	assert_eq!(account_views[0].lamports(), source_before);
	assert_eq!(account_views[1].lamports(), recipient_before);
	assert_eq!(account_views[0].data_len(), data_len_before);
	assert!(!account_views[0].is_data_empty());
}

#[test]
fn assert_type_rejects_wrong_owner_before_trusting_bytes() {
	let unique_accounts = [AccountBuilder::new()
		.address(fake_address(22))
		.owner(system::ID)
		.lamports(100)
		.data(&build_balance_state_bytes(55))
		.is_writable(true)];

	let (_input, _accounts, account_views) = load_accounts!(&unique_accounts, 0, 4);
	let result = account_views[0].assert_type::<BalanceState>(&TEST_PROGRAM_ID);
	assert_eq!(result, Err(ProgramError::InvalidAccountOwner));
}

#[test]
fn assert_type_rejects_wrong_discriminator() {
	let mut wrong_bytes = vec![0u8; size_of::<BalanceState>()];
	wrong_bytes[0] = 99;
	let unique_accounts = [AccountBuilder::new()
		.address(fake_address(23))
		.owner(TEST_PROGRAM_ID)
		.lamports(100)
		.data(&wrong_bytes)
		.is_writable(true)];

	let (_input, _accounts, account_views) = load_accounts!(&unique_accounts, 0, 4);
	let result = account_views[0].assert_type::<BalanceState>(&TEST_PROGRAM_ID);
	assert_eq!(result, Err(ProgramError::InvalidAccountData));
}

#[test]
fn assert_program_rejects_wrong_identity_and_non_executable_targets() {
	let unique_accounts = [
		AccountBuilder::new()
			.address(fake_address(24))
			.owner(TEST_PROGRAM_ID)
			.lamports(0)
			.executable(true),
		AccountBuilder::new()
			.address(system::ID)
			.owner(TEST_PROGRAM_ID)
			.lamports(0)
			.executable(false),
	];

	let (_input, _accounts, account_views) = load_accounts!(&unique_accounts, 0, 4);

	assert_eq!(
		account_views[0].assert_program(&system::ID),
		Err(ProgramError::InvalidAccountData)
	);
	assert_eq!(
		account_views[1].assert_program(&system::ID),
		Err(ProgramError::InvalidAccountData)
	);
}

#[test]
fn non_canonical_pda_requires_explicit_bump_verification() {
	let (seed_bytes, _canonical_address, canonical_bump, non_canonical_address, non_canonical_bump) =
		find_non_canonical_pda_fixture();
	let unique_accounts = [AccountBuilder::new()
		.address(non_canonical_address)
		.owner(TEST_PROGRAM_ID)
		.lamports(1)
		.is_writable(true)];

	let (_input, _accounts, account_views) = load_accounts!(&unique_accounts, 0, 4);
	let canonical_seeds: &[&[u8]] = &[b"noncanonical", &seed_bytes];
	let bump_seed = [non_canonical_bump];
	let seeds_with_bump: &[&[u8]] = &[b"noncanonical", &seed_bytes, &bump_seed];

	assert!(
		account_views[0]
			.assert_seeds_with_bump(seeds_with_bump, &TEST_PROGRAM_ID)
			.is_ok(),
		"stored non-canonical bump should still derive a valid PDA"
	);
	assert_eq!(
		account_views[0].assert_seeds(canonical_seeds, &TEST_PROGRAM_ID),
		Err(ProgramError::InvalidSeeds)
	);
	assert_eq!(
		account_views[0].assert_canonical_bump(canonical_seeds, &TEST_PROGRAM_ID),
		Err(ProgramError::InvalidSeeds)
	);
	assert_ne!(canonical_bump, non_canonical_bump);
}

// Token balance overflow and insufficient-funds semantics are enforced by the
// SPL Token programs during CPI. This suite focuses on the invariants that
// Pina itself owns: token account identity, owner allowlists, and ATA checks.
#[cfg(feature = "token")]
#[test]
fn token_checked_loader_rejects_wrong_owner() {
	let mint = fake_address(25);
	let owner = fake_address(26);
	let unique_accounts = [AccountBuilder::new()
		.address(fake_address(27))
		.owner(system::ID)
		.lamports(1)
		.data(&build_token_account_bytes(&mint, &owner, 55))
		.is_writable(true)];

	let (_input, _accounts, account_views) = load_accounts!(&unique_accounts, 0, 4);
	let result = account_views[0].as_token_account_checked();
	assert!(matches!(result, Err(ProgramError::InvalidAccountOwner)));
}

#[cfg(feature = "token")]
#[test]
fn associated_token_loader_rejects_wrong_ata_address() {
	let wallet = fake_address(28);
	let mint = fake_address(29);
	let wrong_ata = fake_address(30);
	let unique_accounts = [AccountBuilder::new()
		.address(wrong_ata)
		.owner(token::ID)
		.lamports(1)
		.data(&build_token_account_bytes(&mint, &wallet, 99))
		.is_writable(true)];

	let (_input, _accounts, account_views) = load_accounts!(&unique_accounts, 0, 4);
	let result = account_views[0].as_associated_token_account_checked(&wallet, &mint, &token::ID);
	assert!(matches!(result, Err(ProgramError::InvalidSeeds)));
}
