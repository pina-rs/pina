// normalize-stderr-test: "\n$" -> ""
// aux-build: pinocchio.rs
// aux-build: pina.rs
// aux-build: pina_sdk_ids.rs

#![allow(dead_code, unused_assignments)]

extern crate pina;
extern crate pina_sdk_ids;
extern crate pinocchio;

use pina::AccountInfoValidation;
use pina::ClockView;
use pina::OtherView;
use pina::RentView;
use pina_sdk_ids::sysvar;
use pinocchio::account::AccountView;
use pinocchio::sysvars::clock::Clock;
use pinocchio::sysvars::instructions::Instructions;
use pinocchio::sysvars::rent::Rent;
use pinocchio::sysvars::slot_hashes::SlotHashes;

struct FakeClockView;

impl FakeClockView {
	fn assert_sysvar(&self, _id: &()) -> Result<(), ()> {
		Ok(())
	}

	fn try_borrow(&self) -> Result<(), ()> {
		Ok(())
	}
}

mod attacker {
	pub mod clock {
		pub static ID: () = ();
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

fn process_ignored_assertion(clock: &ClockView) -> Result<(), ()> {
	let _ = clock.assert_sysvar(&sysvar::clock::ID);
	let _ = clock.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_observed_assertion_failure(clock: &ClockView) -> Result<(), ()> {
	let _failed = clock.assert_sysvar(&sysvar::clock::ID).is_err();
	let _ = clock.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_conditional_assertion(clock: &ClockView, condition: bool) -> Result<(), ()> {
	if condition {
		clock.assert_sysvar(&sysvar::clock::ID)?;
	}
	let _ = clock.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_match_assertion(clock: &ClockView, condition: bool) -> Result<(), ()> {
	match condition {
		true => {
			clock.assert_sysvar(&sysvar::clock::ID)?;
		}
		false => {}
	}
	let _ = clock.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_assertion_on_every_path(clock: &ClockView, condition: bool) -> Result<(), ()> {
	if condition {
		clock.assert_sysvar(&sysvar::clock::ID)?;
	} else {
		clock.assert_sysvar(&sysvar::clock::ID)?;
	}
	let _ = clock.try_borrow()?;
	Ok(())
}

fn process_assertion_after_diverging_path(clock: &ClockView, condition: bool) -> Result<(), ()> {
	match condition {
		true => return Ok(()),
		false => {
			clock.assert_sysvar(&sysvar::clock::ID)?;
		}
	}
	let _ = clock.try_borrow()?;
	Ok(())
}

fn process_same_named_assertion(clock: &FakeClockView) -> Result<(), ()> {
	clock.assert_sysvar(&sysvar::clock::ID)?;
	let _ = clock.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_same_named_id(clock: &ClockView) -> Result<(), ()> {
	clock.assert_sysvar(&attacker::clock::ID)?;
	let _ = clock.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
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
	//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
	let _ = clock.slot;
	Ok(())
}

fn process_unsafe_unchecked_typed_sysvar(data: &[u8]) {
	let clock = unsafe { Clock::from_bytes_unchecked(data) };
	//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
	let _ = clock.slot;
}

fn process_unchecked_constructor_function_value(data: &[u8]) -> Result<(), ()> {
	let parse = Clock::from_bytes;
	//~^ ERROR: `Clock::from_bytes` cannot be used as a function value
	let clock = parse(data)?;
	let _ = clock.slot;

	let parse = Clock::from_bytes as fn(&[u8]) -> Result<Clock, ()>;
	//~^ ERROR: `Clock::from_bytes` cannot be used as a function value
	let _ = parse(data)?;

	let (parse,) = (Rent::from_bytes,);
	//~^ ERROR: `Rent::from_bytes` cannot be used as a function value
	let rent = parse(data)?;
	let _ = rent.minimum_balance(8);
	Ok(())
}

fn process_other_identity_unchecked_constructors(data: &[u8]) -> Result<(), ()> {
	let _ = unsafe { Instructions::new_unchecked(data) };
	//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
	let _ = SlotHashes::new(data)?;
	//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
	let _ = unsafe { SlotHashes::new_unchecked(data) };
	//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
	Ok(())
}

fn process_unchecked_typed_method_with_neutral_name(data: &[u8]) -> Result<(), ()> {
	let parsed = Rent::from_bytes(data)?;
	//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
	let _ = parsed.minimum_balance(8);
	Ok(())
}

fn process_inline_unchecked_typed_method(data: &[u8]) -> Result<(), ()> {
	let _ = Rent::from_bytes(data)?.minimum_balance(8);
	//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
	Ok(())
}

fn process_conditionally_unchecked_typed_method(
	rent_account: &AccountView,
	data: &[u8],
	condition: bool,
) -> Result<(), ()> {
	let parsed = if condition {
		Rent::from_account_view(rent_account)?
	} else {
		Rent::from_bytes(data)?
		//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
	};
	let _ = parsed.minimum_balance(8);
	Ok(())
}

fn process_conditionally_checked_typed_method(
	first: &AccountView,
	second: &AccountView,
	condition: bool,
) -> Result<(), ()> {
	let parsed = if condition {
		Rent::from_account_view(first)?
	} else {
		Rent::from_account_view(second)?
	};
	let _ = parsed.minimum_balance(8);
	Ok(())
}

fn process_unchecked_first_conditional_typed_method(
	rent_account: &AccountView,
	data: &[u8],
	condition: bool,
) -> Result<(), ()> {
	let parsed = if condition {
		Rent::from_bytes(data)?
		//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
	} else {
		Rent::from_account_view(rent_account)?
	};
	let _ = parsed.minimum_balance(8);

	let parsed = match condition {
		true => Rent::from_bytes(data)?,
		//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
		false => Rent::from_account_view(rent_account)?,
	};
	let _ = parsed.minimum_balance(16);
	Ok(())
}

fn process_unchecked_typed_destructure(data: &[u8]) -> Result<(), ()> {
	let Clock { slot, .. } = Clock::from_bytes(data)?;
	//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
	let _ = slot;
	Ok(())
}

fn process_checked_typed_destructure(clock_account: &AccountView) -> Result<(), ()> {
	let Clock { slot, .. } = Clock::from_account_view(clock_account)?;
	let _ = slot;
	Ok(())
}

fn process_unchecked_typed_match(data: &[u8]) -> Result<(), ()> {
	match Clock::from_bytes(data)? {
		//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
		Clock { slot, .. } => {
			let _ = slot;
		}
	}
	Ok(())
}

fn process_checked_typed_match(clock_account: &AccountView) -> Result<(), ()> {
	match Clock::from_account_view(clock_account)? {
		Clock { slot, .. } => {
			let _ = slot;
		}
	}
	Ok(())
}

fn process_checked_result_patterns(clock_account: &AccountView) -> Result<(), ()> {
	let Ok(clock) = Clock::from_account_view(clock_account) else {
		return Err(());
	};
	let _ = clock.slot;

	if let Ok(clock) = Clock::from_account_view(clock_account) {
		let _ = clock.slot;
	}

	match Clock::from_account_view(clock_account) {
		Ok(clock) => {
			let _ = clock.slot;
		}
		Err(()) => return Err(()),
	}

	let Some(Ok(clock)) = Some(Clock::from_account_view(clock_account)) else {
		return Err(());
	};
	let _ = clock.slot;
	Ok(())
}

fn process_checked_prebound_destructure(clock_account: &AccountView) -> Result<(), ()> {
	let clock = Clock::from_account_view(clock_account)?;
	let Clock { slot, .. } = clock;
	let _ = slot;
	Ok(())
}

fn process_unchecked_prebound_destructure(data: &[u8]) -> Result<(), ()> {
	let clock = Clock::from_bytes(data)?;
	//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
	let Clock { slot, .. } = clock;
	let _ = slot;
	Ok(())
}

fn process_unchecked_nested_pattern(data: &[u8]) -> Result<(), ()> {
	if let Some(Clock { slot, .. }) = Some(Clock::from_bytes(data)?) {
		//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
		let _ = slot;
	}
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
		//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
		let _ = clock.slot;
	}

	Ok(())
}

fn process_discarded_checked_typed_sysvar(
	clock_account: &AccountView,
	data: &[u8],
) -> Result<(), ()> {
	let clock =
		Clock::from_account_view(clock_account).map(|_| Clock::from_bytes(data).unwrap())?;
	//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
	let _ = clock.slot;
	Ok(())
}

fn process_replaced_and_then_typed_sysvar(
	clock_account: &AccountView,
	data: &[u8],
) -> Result<(), ()> {
	let clock = Clock::from_account_view(clock_account).and_then(|_| Clock::from_bytes(data))?;
	//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
	let _ = clock.slot;
	Ok(())
}

fn process_checked_result_adapters(clock_account: &AccountView) -> Result<(), ()> {
	let clock = Clock::from_account_view(clock_account).map(|clock| {
		let _ = clock.slot;
		clock
	})?;
	let _ = clock.slot;

	let clock = Clock::from_account_view(clock_account).inspect(|clock| {
		let _ = clock.slot;
	})?;
	let _ = clock.slot;

	let clock = Clock::from_account_view(clock_account).and_then(|clock| {
		let _ = clock.slot;
		Ok(clock)
	})?;
	let _ = clock.slot;
	Ok(())
}

fn process_adapter_does_not_validate_captured_value(
	clock_account: &AccountView,
	data: &[u8],
) -> Result<(), ()> {
	let unchecked = Clock::from_bytes(data)?;
	//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
	Clock::from_account_view(clock_account).inspect(|_| {
		let _ = unchecked.slot;
	})?;
	Ok(())
}

fn process_mutated_result_adapters(rent_account: &AccountView, data: &[u8]) -> Result<(), ()> {
	let rent = Rent::from_account_view(rent_account).map(|mut rent| {
		rent = Rent::from_bytes(data).unwrap();
		//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
		rent
	})?;
	let _ = rent.minimum_balance(8);

	let rent = Rent::from_account_view(rent_account).and_then(|mut rent| {
		rent = Rent::from_bytes(data)?;
		//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
		Ok(rent)
	})?;
	let _ = rent.minimum_balance(16);
	Ok(())
}

fn replacer_that_ignores_input(_rent: Rent) -> Result<Rent, ()> {
	Rent::from_bytes(&[])
	//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
}

fn process_arbitrary_result_replacer(rent_account: &AccountView) -> Result<(), ()> {
	let rent = Rent::from_account_view(rent_account).and_then(replacer_that_ignores_input)?;
	let _ = rent.minimum_balance(8);
	Ok(())
}

fn process_mutable_replacement(rent_account: &AccountView, data: &[u8]) -> Result<(), ()> {
	let mut rent = Rent::from_account_view(rent_account)?;
	let _ = core::mem::replace(&mut rent, Rent::from_bytes(data)?);
	//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
	let _ = rent.minimum_balance(8);
	Ok(())
}

fn process_checked_extraction_forms(rent_account: &AccountView) -> Result<(), ()> {
	let rent = Rent::from_account_view(rent_account).unwrap();
	let _ = rent.minimum_balance(8);

	let rent = Rent::from_account_view(rent_account).expect("checked loader");
	let _ = rent.minimum_balance(16);

	let (rent, number) = (Rent::from_account_view(rent_account)?, 1_u64);
	let _ = (rent.minimum_balance(32), number);

	let rent = match Rent::from_account_view(rent_account) {
		Ok(rent) => rent,
		Err(()) => return Err(()),
	};
	let _ = rent.minimum_balance(64);
	Ok(())
}

fn process_direct_asserted_raw_source(rent_account: &RentView) -> Result<(), ()> {
	rent_account.assert_sysvar(&sysvar::rent::ID)?;
	#[allow(require_sysvar_assert_before_sysvar_use)]
	let rent = Rent::from_bytes(rent_account.data())?;
	let _ = rent.minimum_balance(8);
	Ok(())
}

fn process_aliased_raw_source_needs_narrow_allow(rent_account: &RentView) -> Result<(), ()> {
	rent_account.assert_sysvar(&sysvar::rent::ID)?;
	let data = rent_account.data();
	let rent = Rent::from_bytes(data)?;
	//~^ ERROR: unchecked typed sysvar constructor requires a validated source account
	let _ = rent.minimum_balance(8);
	Ok(())
}

fn process_direct_checked_sysvar(rent_account: &AccountView) -> Result<(), ()> {
	let _ = Rent::from_account_view(rent_account)?.minimum_balance(8);
	Ok(())
}

fn process_checked_constructor_function_value(clock_account: &AccountView) -> Result<(), ()> {
	let parse = Clock::from_account_view;
	let clock = parse(clock_account)?;
	let _ = clock.slot;
	Ok(())
}

fn process_enforced_raw_sysvar_assertions(clock: &ClockView) -> Result<(), ()> {
	clock.assert_sysvar(&sysvar::clock::ID).unwrap();
	let _ = clock.try_borrow()?;

	clock
		.assert_sysvar(&sysvar::clock::ID)
		.expect("sysvar validation");
	let _ = clock.try_borrow()?;
	Ok(())
}

fn main() {}

// compile-fail
