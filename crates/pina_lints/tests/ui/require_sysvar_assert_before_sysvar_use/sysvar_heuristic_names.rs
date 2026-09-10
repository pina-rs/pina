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
use pina::RentView;
use pina_sdk_ids::sysvar;

fn load_current_index(account: &ClockView) -> usize {
	account.slot() as usize
}

fn load_instruction_at(_index: usize, account: &ClockView) -> i64 {
	account.unix_timestamp()
}

fn epoch_sysvar(account: &ClockView) -> u64 {
	account.slot()
}

fn some_instructions(account: &ClockView) -> u64 {
	account.slot()
}

fn rent(account: &RentView) -> u64 {
	account.lamports_per_byte()
}

fn process_unchecked_sysvar_shaped_calls(
	clock_account: &ClockView,
	rent_account: &RentView,
) -> Result<(), ()> {
	let _ = load_current_index(clock_account);
	//~^ ERROR: raw sysvar access should be preceded by
	let _ = load_instruction_at(0, clock_account);
	//~^ ERROR: raw sysvar access should be preceded by
	let _ = epoch_sysvar(clock_account);
	//~^ ERROR: raw sysvar access should be preceded by
	let _ = some_instructions(clock_account);
	//~^ ERROR: raw sysvar access should be preceded by
	let _ = rent(rent_account);
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_proven_sysvar_shaped_calls(
	clock_account: &ClockView,
	rent_account: &RentView,
) -> Result<(), ()> {
	clock_account.assert_sysvar(&sysvar::clock::ID)?;
	rent_account.assert_sysvar(&sysvar::rent::ID)?;
	let _ = load_current_index(clock_account);
	let _ = epoch_sysvar(clock_account);
	let _ = some_instructions(clock_account);
	let _ = rent(rent_account);
	Ok(())
}

fn main() {}

// compile-fail
