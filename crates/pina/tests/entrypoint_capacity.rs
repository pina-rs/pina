#![allow(unsafe_code)]

//! A program whose entrypoint array is `ENTRYPOINT_ACCOUNT_CAPACITY`, run
//! through its declared `entrypoint` symbol. A route with a
//! `#[pina(remaining)]` slice must see every account it is sent, and a
//! bounded route keeps accepting its omitted optional accounts.

use core::cell::Cell;
use std::vec::Vec;

use pina::*;

#[path = "support/loader_input.rs"]
mod loader_input;

use loader_input::Slot;
use loader_input::serialize;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator(entrypoint)]
pub enum Instruction {
	Sweep = 0,
	Close = 1,
}

/// One positional account plus a trailing slice: two declared slots, but no
/// limit on the accounts it accepts.
#[derive(Accounts)]
pub struct SweepAccounts<'a> {
	pub first: &'a mut AccountView,
	#[pina(remaining)]
	pub rest: &'a [AccountView],
}

/// A required account and an optional one the caller may omit.
#[derive(Accounts)]
pub struct CloseAccounts<'a> {
	pub state: &'a mut AccountView,
	pub recipient: Option<&'a AccountView>,
}

impl<'a> ProcessAccountInfos<'a> for SweepAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		SEEN.set(Some(self.rest.len()));
		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for CloseAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		SEEN.set(Some(usize::from(self.recipient.is_some())));
		Ok(())
	}
}

nostd_entrypoint!(
	Instruction::process_instruction,
	Instruction::ENTRYPOINT_ACCOUNT_CAPACITY
);

std::thread_local! {
	/// What the last handler counted: the remaining accounts, or whether the
	/// optional account was present.
	static SEEN: Cell<Option<usize>> = const { Cell::new(None) };
}

fn account(address: u8) -> Slot {
	Slot::Account { address, owner: ID }
}

/// Runs the declared entrypoint and returns its status with what the handler
/// counted, if it ran.
fn run(slots: &[Slot], data: &[u8]) -> (u64, Option<usize>) {
	let mut input = serialize(slots, data, &ID);
	// SAFETY: `input` is a complete loader input.
	let status = unsafe { entrypoint(input.as_mut_ptr().cast()) };

	(status, SEEN.take())
}

#[test]
fn a_remaining_slice_makes_the_capacity_the_transaction_maximum() {
	type Sweep = SweepAccounts<'static>;

	// The declared bound keeps counting the slice as one slot, so
	// `MAX_INSTRUCTION_ACCOUNTS` is unchanged; the limit is what sizes the array.
	assert_eq!(<Sweep as ParseAccounts<'static>>::ACCOUNT_BOUND, 2);
	assert_eq!(
		<Sweep as ParseAccounts<'static>>::ACCOUNT_LIMIT,
		<Sweep as ParseAccounts<'static>>::UNBOUNDED
	);
	assert_eq!(Instruction::MAX_INSTRUCTION_ACCOUNTS, 2);
	assert_eq!(
		Instruction::ENTRYPOINT_ACCOUNT_CAPACITY,
		pinocchio::MAX_TX_ACCOUNTS
	);
}

/// Every trailing account reaches the handler, not only those that fit a
/// capacity derived from the declared slots.
#[test]
fn a_remaining_route_sees_every_trailing_account() {
	let slots = (1..=5).map(account).collect::<Vec<_>>();

	assert_eq!(run(&slots, &[0]), (pinocchio::SUCCESS, Some(4)));
}

/// A writable account duplicated past the declared slots is still found.
#[test]
fn a_remaining_route_rejects_a_writable_duplicate_past_the_declared_slots() {
	let slots = [account(1), account(2), account(3), Slot::Duplicate(0)];

	assert_eq!(
		run(&slots, &[0]),
		(
			u64::from(ProgramError::from(
				PinaProgramError::DuplicateMutableAccount
			)),
			None
		)
	);
}

/// The optional account may be omitted, and a bounded route still rejects an
/// account past its limit.
#[test]
fn an_optional_route_accepts_its_omitted_account() {
	assert_eq!(run(&[account(1)], &[1]), (pinocchio::SUCCESS, Some(0)));
	assert_eq!(
		run(&[account(1), account(2)], &[1]),
		(pinocchio::SUCCESS, Some(1))
	);
	assert_eq!(
		run(&[account(1), account(2), account(3)], &[1]),
		(
			u64::from(ProgramError::from(PinaProgramError::TooManyAccountKeys)),
			None
		)
	);
}
