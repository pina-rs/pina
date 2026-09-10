// normalize-stderr-test: "\n$" -> ""
// aux-build: pinocchio.rs
// aux-build: pina.rs
// aux-build: pina_sdk_ids.rs

#![allow(
	dead_code,
	unused_assignments,
	unused_variables,
	unused_mut,
	unreachable_code,
	unused_parens
)]

extern crate pina;
extern crate pina_sdk_ids;
extern crate pinocchio;

use pina::AccountInfoValidation;
use pina::ClockView;
use pina_sdk_ids::sysvar;
use pinocchio::account::AccountView;
use pinocchio::sysvars::clock::Clock;

struct Counter {
	value: u64,
}

impl Counter {
	fn value(&self) -> u64 {
		self.value
	}
}

struct SlotHolder {
	slot: u64,
}

impl SlotHolder {
	fn reset(&mut self) {
		self.slot = 0;
	}
}

struct Snapshot<'a> {
	clock: &'a ClockView,
	slot: u64,
}

fn process_closure_read_ignores_outside_proof(clock_account: &ClockView) -> Result<(), ()> {
	clock_account.assert_sysvar(&sysvar::clock::ID)?;
	let read = || clock_account.try_borrow();
	//~^ ERROR: raw sysvar access should be preceded by
	let _ = read()?;
	Ok(())
}

fn process_unproven_closure_read(clock_account: &ClockView) -> Result<(), ()> {
	let read = || clock_account.try_borrow();
	//~^ ERROR: raw sysvar access should be preceded by
	let _ = read()?;
	Ok(())
}

fn process_short_circuit_proof_does_not_dominate(
	clock_account: &ClockView,
	condition: bool,
) -> Result<(), ()> {
	let _ = condition
		&& clock_account
			.assert_sysvar(&sysvar::clock::ID)
			.map_err(|error| error)?
			.try_borrow()
			.is_ok();
	let _ = clock_account.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_arithmetic_and_index_expressions(
	clock_account: &ClockView,
	table: &[u64],
	index: usize,
) -> Result<(), ()> {
	clock_account.assert_sysvar(&sysvar::clock::ID)?;
	let slot = clock_account.slot();
	let _ = table[(slot as usize + index) % table.len()];
	let _ = clock_account.try_borrow()?;
	Ok(())
}

fn process_assignment_invalidates(
	clock_account: &ClockView,
	attacker: &ClockView,
) -> Result<(), ()> {
	let mut clock_account = clock_account;
	clock_account.assert_sysvar(&sysvar::clock::ID)?;
	clock_account = attacker;
	let _ = clock_account.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_reborrow_invalidates(clock_account: &ClockView, attacker: &ClockView) -> Result<(), ()> {
	let mut clock_account = clock_account;
	clock_account.assert_sysvar(&sysvar::clock::ID)?;
	let reborrow = &mut clock_account;
	*reborrow = attacker;
	let _ = clock_account.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_let_chain(clock_account: &ClockView, maybe: Option<u64>) -> Result<(), ()> {
	if let Some(slot) = maybe
		&& slot > 0
	{
		let _ = clock_account.try_borrow()?;
		//~^ ERROR: raw sysvar access should be preceded by
	}
	Ok(())
}

fn process_tuple_array_struct_literals(
	clock_account: &ClockView,
	rent_account: &ClockView,
) -> Result<(), ()> {
	let reads = (clock_account.try_borrow()?, rent_account.try_borrow()?);
	//~^ ERROR: raw sysvar access should be preceded by
	//~| ERROR: raw sysvar access should be preceded by
	let _ = reads;

	let snapshots = [Snapshot {
		clock: clock_account,
		slot: clock_account.slot(),
	}];
	//~^ ERROR: raw sysvar access should be preceded by
	let _ = snapshots;

	let base = Snapshot {
		clock: clock_account,
		slot: 0,
	};
	let updated = Snapshot { slot: 1, ..base };
	let _ = updated;
	Ok(())
}

fn process_repeat_and_unary_expressions(clock_account: &ClockView, flag: bool) {
	let _ = !flag;
	let _ = -1_i64;
	let reads = [clock_account.slot(); 2];
	//~^ ERROR: raw sysvar access should be preceded by
	let _ = reads;
}

fn process_if_let_temporary_scrutinee(clock_account: &ClockView) {
	if let Some(slot) = Some(clock_account.slot()) {
		let _ = slot;
	}
}

fn process_struct_literal_receiver(counter: &Counter) -> u64 {
	Counter { value: 1 }.value()
}

fn process_local_mutating_helper(clock_account: &ClockView) -> Result<(), ()> {
	clock_account.assert_sysvar(&sysvar::clock::ID)?;
	let mut holder = SlotHolder {
		slot: clock_account.slot(),
	};
	holder.reset();
	let _ = clock_account.try_borrow()?;
	Ok(())
}

fn process_trusted_mutable_method(clock_account: &AccountView) -> Result<(), ()> {
	let mut clock = Clock::from_account_view(clock_account)?;
	clock.advance(1);
	let _ = clock.slot;
	Ok(())
}

fn process_declared_then_assigned(clock_account: &ClockView, proven: bool) -> Result<(), ()> {
	const FALLBACK_SLOT: u64 = 0;
	let _ = FALLBACK_SLOT;

	let clock_view: &ClockView;
	if proven {
		clock_view = clock_account;
	} else {
		return Err(());
	}
	clock_view.assert_sysvar(&sysvar::clock::ID)?;
	let _ = clock_view.try_borrow()?;
	Ok(())
}

mod tests {
	use super::*;

	fn process_skipped_in_tests_module(clock_account: &ClockView) -> Result<(), ()> {
		let _ = clock_account.try_borrow()?;
		Ok(())
	}
}

fn main() {}

// compile-fail
