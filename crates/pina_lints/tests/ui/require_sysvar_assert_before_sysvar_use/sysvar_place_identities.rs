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

struct Holder<'a> {
	clock: &'a ClockView,
}

fn process_field_proof(holder: &Holder<'_>) -> Result<(), ()> {
	holder.clock.assert_sysvar(&sysvar::clock::ID)?;
	let _ = holder.clock.try_borrow()?;
	Ok(())
}

fn process_base_reassignment_invalidates_field_proof(
	first: &ClockView,
	second: &ClockView,
) -> Result<(), ()> {
	let mut holder = Holder { clock: first };
	holder.clock.assert_sysvar(&sysvar::clock::ID)?;
	holder = Holder { clock: second };
	let _ = holder.clock.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_field_write_keeps_local_proof<'a>(
	clock_account: &'a ClockView,
	holder: &mut Holder<'a>,
) -> Result<(), ()> {
	clock_account.assert_sysvar(&sysvar::clock::ID)?;
	holder.clock = clock_account;
	let _ = clock_account.try_borrow()?;
	Ok(())
}

fn process_deref_reassignment_invalidates<'a>(
	holder: &mut Holder<'a>,
	second: &'a ClockView,
) -> Result<(), ()> {
	holder.clock.assert_sysvar(&sysvar::clock::ID)?;
	*holder = Holder { clock: second };
	let _ = holder.clock.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_addr_of_alias_keeps_proof(clock_account: &ClockView) -> Result<(), ()> {
	clock_account.assert_sysvar(&sysvar::clock::ID)?;
	let clock_account = &clock_account;
	let _ = clock_account.try_borrow()?;
	Ok(())
}

fn process_aliased_sysvar_id_is_not_canonical(clock_account: &ClockView) -> Result<(), ()> {
	let clock_id = sysvar::clock::ID;
	clock_account.assert_sysvar(&clock_id)?;
	let _ = clock_account.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_unwrap_or_branch_argument(
	clock_account: &ClockView,
	fallback: &ClockView,
) -> Result<(), ()> {
	let clock_account = Some(clock_account).unwrap_or(fallback);
	let _ = clock_account.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_try_desugar_identity_chain(clock_account: &ClockView) -> Result<(), ()> {
	let view = clock_account
		.assert_sysvar(&sysvar::clock::ID)?
		.try_borrow()?;
	let _ = view.slot();
	Ok(())
}

fn main() {}

// compile-fail
