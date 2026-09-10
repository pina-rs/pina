// normalize-stderr-test: "\n$" -> ""
// aux-build: pinocchio.rs
// aux-build: pina.rs
// aux-build: pina_sdk_ids.rs

#![feature(coroutines, coroutine_trait, stmt_expr_attributes, explicit_tail_calls)]
#![allow(
	dead_code,
	unused_assignments,
	unused_variables,
	unused_mut,
	unreachable_code,
	unused_parens,
	incomplete_features
)]

extern crate pina;
extern crate pina_sdk_ids;

use pina::AccountInfoValidation;
use pina::ClockView;
use pina_sdk_ids::sysvar;

fn process_generator_read(clock_account: &ClockView) {
	let generator = #[coroutine]
	|| {
		yield clock_account.slot();
	};
	//~^ ERROR: raw sysvar access should be preceded by
	let _ = generator;
}

fn process_tail_call_read(clock_account: &ClockView, rounds: u32) -> u32 {
	if rounds == 0 {
		0
	} else {
		let _ = clock_account.assert_sysvar(&sysvar::clock::ID);
		become process_tail_call_read(clock_account, rounds - 1)
	}
}

fn main() {}

// compile-fail
