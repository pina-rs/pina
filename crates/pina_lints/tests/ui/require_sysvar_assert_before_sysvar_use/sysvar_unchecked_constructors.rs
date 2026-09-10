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
extern crate pinocchio;

use pinocchio::account::AccountView;
use pinocchio::sysvars::clock::Clock;
use pinocchio::sysvars::clock::ClockWrapper;
use pinocchio::sysvars::rent::Rent;
use pinocchio::sysvars::slot_hashes::SlotHashes;

fn process_rent_from_bytes_unchecked(data: &[u8]) {
	let rent = unsafe { Rent::from_bytes_unchecked(data) };
	//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
	let _ = rent.minimum_balance(8);
}

fn process_unchecked_constructor_function_values(data: &[u8]) {
	let parse = Clock::from_bytes_unchecked;
	//~^ ERROR: `Clock::from_bytes_unchecked` cannot be used as a function value
	let clock = unsafe { parse(data) };
	let _ = clock.slot;

	let rent_value = Rent::from_bytes_unchecked;
	//~^ ERROR: `Rent::from_bytes_unchecked` cannot be used as a function value
	let rent = unsafe { rent_value(data) };
	let _ = rent.minimum_balance(8);
}

fn process_clock_prefix_constructor_is_flagged(data: &[u8]) {
	// The trusted-type check matches the `Clock` path prefix, so this
	// wrapper constructor is rejected like `Clock::from_bytes`.
	let wrapper = ClockWrapper::from_bytes(data);
	//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
	let _ = wrapper;
}

fn process_trusted_from_account_view(clock_account: &AccountView) -> Result<(), ()> {
	let clock = Clock::from_account_view(clock_account)?;
	let _ = clock.slot;
	Ok(())
}

fn process_trusted_slot_hashes_loader(slot_hashes_account: &AccountView) -> Result<(), ()> {
	let slot_hashes = SlotHashes::from_account_view(slot_hashes_account)?;
	let _ = slot_hashes.len();
	Ok(())
}

fn main() {}

// compile-fail
