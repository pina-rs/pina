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

fn process_full_path_proof(clock_account: &ClockView) -> Result<(), ()> {
	clock_account.assert_sysvar(&pina_sdk_ids::sysvar::clock::ID)?;
	let _ = clock_account.try_borrow()?;
	Ok(())
}

fn process_parenthesized_id_proof(clock_account: &ClockView) -> Result<(), ()> {
	clock_account.assert_sysvar((&pina_sdk_ids::sysvar::clock::ID))?;
	let _ = clock_account.try_borrow()?;
	Ok(())
}

fn process_dereferenced_id_alias(clock_account: &ClockView) -> Result<(), ()> {
	let double = &&pina_sdk_ids::sysvar::clock::ID;
	clock_account.assert_sysvar(*double)?;
	let _ = clock_account.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_non_id_item_is_not_canonical(clock_account: &ClockView) -> Result<(), ()> {
	clock_account.assert_sysvar(&pina_sdk_ids::sysvar::clock::NAME)?;
	let _ = clock_account.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn process_wrong_sysvar_id_full_path(clock_account: &ClockView) -> Result<(), ()> {
	clock_account.assert_sysvar(&pina_sdk_ids::sysvar::rent::ID)?;
	let _ = clock_account.try_borrow()?;
	//~^ ERROR: sysvar access should be preceded by
	Ok(())
}

fn process_non_path_id_expression(clock_account: &ClockView) -> Result<(), ()> {
	clock_account.assert_sysvar(if true {
		&pina_sdk_ids::sysvar::clock::ID
	} else {
		&pina_sdk_ids::sysvar::clock::ID
	})?;
	let _ = clock_account.try_borrow()?;
	//~^ ERROR: raw sysvar access should be preceded by
	Ok(())
}

fn main() {}

// compile-fail
