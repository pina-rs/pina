// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

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

fn main() {}
