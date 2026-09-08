// normalize-stderr-test: "\n$" -> ""
// aux-build: pinocchio.rs

#![allow(dead_code)]

extern crate pinocchio;

use pinocchio::account::AccountView;
use pinocchio::sysvars::clock::Clock;
use pinocchio::sysvars::instructions::Instructions;
use pinocchio::sysvars::rent::Rent;
use pinocchio::sysvars::slot_hashes::SlotHashes;

struct ClockView;
struct RentView;
struct OtherView;

mod sysvar {
	pub mod clock {
		pub static ID: () = ();
	}

	pub mod rent {
		pub static ID: () = ();
	}
}

impl ClockView {
	fn assert_sysvar(&self, _id: &()) -> Result<(), ()> {
		Ok(())
	}

	fn try_borrow(&self) -> Result<ClockView, ()> {
		Ok(ClockView)
	}

	fn slot(&self) -> u64 {
		0
	}

	fn unix_timestamp(&self) -> i64 {
		0
	}
}

impl RentView {
	fn try_borrow(&self) -> Result<RentView, ()> {
		Ok(RentView)
	}

	fn lamports_per_byte(&self) -> u64 {
		0
	}
}

fn process_clock(clock: &ClockView) -> Result<(), ()> {
	clock.assert_sysvar(&sysvar::clock::ID)?;
	let view = clock.try_borrow()?;
	let _ = view.slot();
	Ok(())
}

fn process_epoch(clock: &ClockView) -> Result<(), ()> {
	let view = clock.try_borrow()?;
	//~^ ERROR: sysvar access should be preceded by
	let _ = view.unix_timestamp();
	Ok(())
}

fn process_asserted(clock: &ClockView) -> Result<(), ()> {
	clock.assert_sysvar(&sysvar::clock::ID)?;
	let view = clock.try_borrow()?;
	let _ = view.unix_timestamp();
	Ok(())
}

fn process_mismatched_id(clock: &ClockView) -> Result<(), ()> {
	// Asserting a different sysvar id does not guard this receiver.
	clock.assert_sysvar(&sysvar::rent::ID)?;
	let view = clock.try_borrow()?;
	//~^ ERROR: sysvar access should be preceded by
	let _ = view.slot();
	Ok(())
}

fn process_rent(rent: &RentView) -> Result<(), ()> {
	let view = rent.try_borrow()?;
	//~^ ERROR: sysvar access should be preceded by
	let _ = view.lamports_per_byte();
	Ok(())
}

fn process_suffixed_account(clock_account: &ClockView) -> Result<(), ()> {
	let _ = clock_account.try_borrow()?;
	//~^ ERROR: sysvar access should be preceded by
	Ok(())
}

fn process_asserted_suffixed_account(clock_account: &ClockView) -> Result<(), ()> {
	clock_account.assert_sysvar(&sysvar::clock::ID)?;
	let _ = clock_account.try_borrow()?;
	Ok(())
}

impl OtherView {
	fn try_borrow(&self) -> Result<(), ()> {
		Ok(())
	}
}

fn process_plain(other: &OtherView) -> Result<(), ()> {
	// Receivers that are not sysvar-shaped are not inspected.
	let _ = other.try_borrow();
	Ok(())
}

fn process_typed_sysvars(
	clock_account: &AccountView,
	rent_account: &AccountView,
	instructions_account: &AccountView,
	slot_hashes_account: &AccountView,
) -> Result<(), ()> {
	let clock = Clock::from_account_view(clock_account)?;
	let _ = clock.slot;

	let rent = Rent::from_account_view(rent_account)?;
	let _ = rent.minimum_balance(8);
	let rent = Rent::from_account_view(rent_account).map_err(|error| error)?;
	let _ = rent.minimum_balance(16);

	let instructions = Instructions::try_from(instructions_account)?;
	let _ = instructions.load_current_index();

	let slot_hashes = SlotHashes::from_account_view(slot_hashes_account)?;
	let _ = slot_hashes.len();

	Ok(())
}

fn process_unchecked_typed_sysvar(data: &[u8]) -> Result<(), ()> {
	let clock = Clock::from_bytes(data)?;
	let _ = clock.slot;
	//~^ ERROR: sysvar access should be preceded by
	Ok(())
}

fn process_shadowed_unchecked_typed_sysvar(
	clock_account: &AccountView,
	data: &[u8],
) -> Result<(), ()> {
	let clock = Clock::from_account_view(clock_account)?;
	let _ = clock.slot;

	{
		let clock = Clock::from_bytes(data)?;
		let _ = clock.slot;
		//~^ ERROR: sysvar access should be preceded by
	}

	Ok(())
}

fn process_discarded_checked_typed_sysvar(
	clock_account: &AccountView,
	data: &[u8],
) -> Result<(), ()> {
	let clock =
		Clock::from_account_view(clock_account).map(|_| Clock::from_bytes(data).unwrap())?;
	let _ = clock.slot;
	//~^ ERROR: sysvar access should be preceded by
	Ok(())
}

fn process_direct_checked_sysvar(rent_account: &AccountView) -> Result<(), ()> {
	let _ = Rent::from_account_view(rent_account)?.minimum_balance(8);
	Ok(())
}

fn main() {}

// compile-fail
