#![allow(unsafe_code)]

//! A program declared with `dispatch_entrypoint!`, run through its generated
//! `entrypoint` symbol over loader-format input.
//!
//! This lives in its own test binary because `entrypoint` is an unmangled
//! symbol, and other suites declare one with `nostd_entrypoint!`.

use core::cell::RefCell;
use core::mem::size_of;
use std::vec::Vec;

use pina::*;
use pinocchio::SUCCESS;
use pinocchio::account::MAX_PERMITTED_DATA_INCREASE;
use pinocchio::entrypoint::NON_DUP_MARKER;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

/// One route per account walk: exact, bounded by an optional account, and
/// unbounded through a trailing slice.
#[discriminator(entrypoint)]
pub enum Instruction {
	Initialize = 0,
	Close = 1,
	Sweep = 2,
}

#[derive(Accounts)]
pub struct InitializeAccounts<'a> {
	pub state: &'a mut AccountView,
	pub authority: &'a AccountView,
}

#[derive(Accounts)]
pub struct CloseAccounts<'a> {
	pub state: &'a mut AccountView,
	pub recipient: Option<&'a AccountView>,
}

#[derive(Accounts)]
pub struct SweepAccounts<'a> {
	pub authority: &'a AccountView,
	#[pina(remaining)]
	pub rest: &'a [AccountView],
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		handle(&[self.state, self.authority], data)
	}
}

impl<'a> ProcessAccountInfos<'a> for CloseAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let mut accounts = std::vec![&*self.state];
		accounts.extend(self.recipient);

		handle(&accounts, data)
	}
}

impl<'a> ProcessAccountInfos<'a> for SweepAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let mut accounts = std::vec![self.authority];
		accounts.extend(self.rest);

		handle(&accounts, data)
	}
}

dispatch_entrypoint!(Instruction);

std::thread_local! {
	/// The first address byte of every account the last handler received.
	static HANDLED: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
}

/// Records the accounts, then fails with the custom code a second data byte
/// names.
fn handle(accounts: &[&AccountView], data: &[u8]) -> ProgramResult {
	let addresses = accounts
		.iter()
		.map(|account| account.address().as_ref()[0])
		.collect();
	HANDLED.set(Some(addresses));

	match data {
		[_, code] => Err(ProgramError::Custom(u32::from(*code))),
		_ => Ok(()),
	}
}

#[derive(Clone, Copy)]
enum Slot {
	/// A serialized account whose address starts with this byte.
	Account { address: u8, writable: bool },
	/// A duplicate marker naming an earlier slot.
	Duplicate(u8),
}

fn account(address: u8) -> Slot {
	Slot::Account {
		address,
		writable: true,
	}
}

fn read_only(address: u8) -> Slot {
	Slot::Account {
		address,
		writable: false,
	}
}

/// Serializes a loader input and returns it with the offset of its instruction
/// data, which the SIMD-0321 pointer addresses. The words keep every record
/// 8-byte aligned, as the loader does.
fn serialize(slots: &[Slot], data: &[u8], program_id: &Address) -> (Vec<u64>, usize) {
	let mut bytes = (slots.len() as u64).to_le_bytes().to_vec();

	for slot in slots {
		match *slot {
			Slot::Account { address, writable } => {
				let mut header = [0; 88];
				header[0] = NON_DUP_MARKER;
				header[2] = u8::from(writable);
				header[8] = address;
				header[40..72].copy_from_slice(ID.as_ref());
				bytes.extend(header);
				// Empty data, the realloc region, alignment, and the rent epoch.
				bytes.resize(
					bytes.len()
						+ MAX_PERMITTED_DATA_INCREASE.next_multiple_of(8)
						+ size_of::<u64>(),
					0,
				);
			}
			Slot::Duplicate(index) => bytes.extend([index, 0, 0, 0, 0, 0, 0, 0]),
		}
	}

	bytes.extend((data.len() as u64).to_le_bytes());
	let offset = bytes.len();
	bytes.extend(data);
	bytes.extend(program_id.as_ref());

	let words = bytes
		.chunks(size_of::<u64>())
		.map(|chunk| {
			let mut word = [0; 8];
			word[..chunk.len()].copy_from_slice(chunk);
			u64::from_ne_bytes(word)
		})
		.collect();

	(words, offset)
}

/// Runs the declared entrypoint and returns its status with the accounts the
/// handler saw, if it ran.
fn run(slots: &[Slot], data: &[u8]) -> (u64, Option<Vec<u8>>) {
	run_for(&ID, slots, data)
}

fn run_for(program_id: &Address, slots: &[Slot], data: &[u8]) -> (u64, Option<Vec<u8>>) {
	let (mut input, offset) = serialize(slots, data, program_id);
	let input = input.as_mut_ptr().cast::<u8>();
	// SAFETY: `input` is a complete loader input and `offset` is where its
	// instruction data starts.
	let status = unsafe { entrypoint(input, input.add(offset)) };

	(status, HANDLED.take())
}

fn status(error: impl Into<ProgramError>) -> u64 {
	u64::from(error.into())
}

#[test]
fn routes_declare_their_account_walks() {
	assert_eq!(
		<InitializeAccounts<'static> as ParseAccounts<'static>>::ACCOUNT_LIMIT,
		2
	);
	assert_eq!(
		<InitializeAccounts<'static> as ParseAccounts<'static>>::ACCOUNT_MINIMUM,
		2
	);
	assert_eq!(
		<CloseAccounts<'static> as ParseAccounts<'static>>::ACCOUNT_LIMIT,
		2
	);
	assert_eq!(
		<CloseAccounts<'static> as ParseAccounts<'static>>::ACCOUNT_MINIMUM,
		1
	);
	assert_eq!(
		<SweepAccounts<'static> as ParseAccounts<'static>>::ACCOUNT_LIMIT,
		<SweepAccounts<'static> as ParseAccounts<'static>>::UNBOUNDED
	);
	assert_eq!(
		<SweepAccounts<'static> as ParseAccounts<'static>>::ACCOUNT_MINIMUM,
		1
	);
}

/// The exact route runs with its two accounts and rejects any other count
/// before the handler runs.
#[test]
fn exact_route_requires_its_account_count() {
	assert_eq!(
		run(&[account(1), account(2)], &[0]),
		(SUCCESS, Some(std::vec![1, 2]))
	);
	assert_eq!(
		run(&[account(1)], &[0]),
		(status(ProgramError::NotEnoughAccountKeys), None)
	);
	assert_eq!(
		run(&[account(1), account(2), account(3)], &[0]),
		(status(PinaProgramError::TooManyAccountKeys), None)
	);
}

/// The bounded route accepts its optional account's absence, and still
/// rejects an account past its limit and a missing required one.
#[test]
fn bounded_route_accepts_an_absent_optional_account() {
	assert_eq!(run(&[account(1)], &[1]), (SUCCESS, Some(std::vec![1])));
	assert_eq!(
		run(&[account(1), account(2)], &[1]),
		(SUCCESS, Some(std::vec![1, 2]))
	);
	assert_eq!(
		run(&[account(1), account(2), account(3)], &[1]),
		(status(PinaProgramError::TooManyAccountKeys), None)
	);
	assert_eq!(
		run(&[], &[1]),
		(status(ProgramError::NotEnoughAccountKeys), None)
	);
}

/// The unbounded route sees every account it is sent: a trailing slice makes
/// the entrypoint capacity the transaction maximum.
#[test]
fn unbounded_route_sees_every_account() {
	let slots = (1..=6).map(account).collect::<Vec<_>>();

	assert_eq!(
		Instruction::ENTRYPOINT_ACCOUNT_CAPACITY,
		pinocchio::MAX_TX_ACCOUNTS
	);
	assert_eq!(
		run(&slots, &[2]),
		(SUCCESS, Some((1..=6).collect::<Vec<u8>>()))
	);
	assert_eq!(run(&[account(1)], &[2]), (SUCCESS, Some(std::vec![1])));
}

/// The routed struct's own checks still run over the dispatch walk: a
/// read-only alias of a mutable account and a read-only mutable field fail.
#[test]
fn routed_struct_checks_still_run() {
	assert_eq!(
		run(&[account(1), Slot::Duplicate(0)], &[1]),
		(status(PinaProgramError::DuplicateMutableAccount), None)
	);
	assert_eq!(
		run(&[read_only(1), account(2)], &[0]),
		(status(ProgramError::InvalidAccountData), None)
	);
}

/// A wrong account count on an exact route fails before the struct's
/// per-account checks, which would otherwise reject the read-only `state`
/// first.
#[test]
fn account_count_precedes_per_account_checks() {
	assert_eq!(
		run(&[read_only(1)], &[0]),
		(status(ProgramError::NotEnoughAccountKeys), None)
	);
	assert_eq!(
		run(&[read_only(1), account(2), account(3)], &[0]),
		(status(PinaProgramError::TooManyAccountKeys), None)
	);
}

/// The program ID and discriminator are validated before any account walk.
#[test]
fn instruction_is_validated_before_the_accounts() {
	let other_program = Address::new_from_array([9; 32]);

	assert_eq!(
		run_for(&other_program, &[account(1), account(2)], &[0]),
		(status(ProgramError::IncorrectProgramId), None)
	);
	assert_eq!(
		run(&[account(1), account(2)], &[9]),
		(status(ProgramError::InvalidInstructionData), None)
	);
	assert_eq!(
		run(&[account(1), account(2)], &[]),
		(status(ProgramError::InvalidInstructionData), None)
	);
}

/// The handler's result becomes the entrypoint's status.
#[test]
fn handler_errors_become_the_status() {
	assert_eq!(
		run(&[account(1), account(2)], &[0, 7]),
		(7, Some(std::vec![1, 2]))
	);
}
