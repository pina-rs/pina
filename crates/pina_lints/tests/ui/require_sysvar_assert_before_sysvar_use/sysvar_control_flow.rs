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

use pina::AccountInfoValidation;
use pina::ClockView;
use pina_sdk_ids::sysvar;

fn process_guarded_match_proof_does_not_dominate(
	clock_account: &ClockView,
	condition: bool,
	other: bool,
) -> Result<(), ()> {
	match condition {
		true if other => {
			clock_account.assert_sysvar(&sysvar::clock::ID)?;
		}
		_ => {}
	}
	let _ = clock_account.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_guard_reads_are_checked(clock_account: &ClockView, condition: bool) -> Result<(), ()> {
	match condition {
		true if clock_account.slot() == 0 => {}
		_ => {}
	}
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_else_reassignment_invalidates(
	first: &ClockView,
	second: &ClockView,
	condition: bool,
) -> Result<(), ()> {
	let mut clock_account = first;
	clock_account.assert_sysvar(&sysvar::clock::ID)?;
	if condition {
		let _ = clock_account.slot();
	} else {
		clock_account = second;
	}
	let _ = clock_account.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_loop_body_proof_does_not_dominate(
	clock_account: &ClockView,
	rounds: u32,
) -> Result<(), ()> {
	let mut remaining = rounds;
	loop {
		if remaining == 0 {
			break;
		}
		clock_account.assert_sysvar(&sysvar::clock::ID)?;
		remaining -= 1;
	}
	let _ = clock_account.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_loop_proof_before_entry(clock_account: &ClockView, rounds: u32) -> Result<(), ()> {
	clock_account.assert_sysvar(&sysvar::clock::ID)?;
	let mut remaining = rounds;
	loop {
		if remaining == 0 {
			break;
		}
		remaining -= 1;
	}
	let _ = clock_account.try_borrow()?;
	Ok(())
}

fn process_all_diverging_match(clock_account: &ClockView, condition: bool) -> Result<(), ()> {
	clock_account.assert_sysvar(&sysvar::clock::ID)?;
	match condition {
		true => return Ok(()),
		false => return Err(()),
	}
}

fn main() {}

// compile-fail
