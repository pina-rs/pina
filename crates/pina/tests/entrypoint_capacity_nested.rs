#![allow(unsafe_code)]

//! A program whose only unbounded route takes its trailing slice through a
//! nested account group, run through its declared `entrypoint` symbol with
//! `ENTRYPOINT_ACCOUNT_CAPACITY`. The group's slice must see every account it
//! is sent.

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
	Grouped = 0,
	Pair = 1,
}

/// A group that ends with a trailing slice.
#[derive(Accounts)]
pub struct SweepGroup<'a> {
	pub first: &'a mut AccountView,
	#[pina(remaining)]
	pub rest: &'a [AccountView],
}

/// A route whose only trailing slice is inside a nested group.
#[derive(Accounts)]
pub struct GroupedAccounts<'a> {
	pub authority: &'a AccountView,
	pub group: SweepGroup<'a>,
}

/// An exact route, which alone would bound the array at three slots.
#[derive(Accounts)]
pub struct PairAccounts<'a> {
	pub first: &'a AccountView,
	pub second: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for GroupedAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		SEEN.set(Some(self.group.rest.len()));
		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for PairAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		SEEN.set(Some(0));
		Ok(())
	}
}

nostd_entrypoint!(
	Instruction::process_instruction,
	Instruction::ENTRYPOINT_ACCOUNT_CAPACITY
);

std::thread_local! {
	/// How many trailing accounts the last handler received.
	static SEEN: Cell<Option<usize>> = const { Cell::new(None) };
}

fn account(address: u8) -> Slot {
	Slot::Account { address, owner: ID }
}

fn run(slots: &[Slot], data: &[u8]) -> (u64, Option<usize>) {
	let mut input = serialize(slots, data, &ID);
	// SAFETY: `input` is a complete loader input.
	let status = unsafe { entrypoint(input.as_mut_ptr().cast()) };

	(status, SEEN.take())
}

#[test]
fn a_nested_remaining_slice_makes_the_capacity_the_transaction_maximum() {
	type Group = SweepGroup<'static>;
	type Grouped = GroupedAccounts<'static>;
	type Pair = PairAccounts<'static>;

	assert_eq!(<Group as ParseAccounts<'static>>::ACCOUNT_BOUND, 2);
	assert_eq!(<Grouped as ParseAccounts<'static>>::ACCOUNT_BOUND, 3);
	assert_eq!(
		<Grouped as ParseAccounts<'static>>::ACCOUNT_LIMIT,
		<Grouped as ParseAccounts<'static>>::UNBOUNDED
	);
	assert_eq!(<Pair as ParseAccounts<'static>>::ACCOUNT_LIMIT, 2);
	assert_eq!(Instruction::MAX_INSTRUCTION_ACCOUNTS, 3);
	assert_eq!(
		Instruction::ENTRYPOINT_ACCOUNT_CAPACITY,
		pinocchio::MAX_TX_ACCOUNTS
	);
}

#[test]
fn a_nested_remaining_slice_sees_every_trailing_account() {
	let slots = (1..=6).map(account).collect::<Vec<_>>();

	assert_eq!(run(&slots, &[0]), (pinocchio::SUCCESS, Some(4)));
}

#[test]
fn a_nested_group_rejects_a_writable_duplicate_past_the_declared_slots() {
	let slots = [
		account(1),
		account(2),
		account(3),
		account(4),
		Slot::Duplicate(1),
	];

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
