// normalize-stderr-test: "\n$" -> ""

//! A tested guard only counts when its failing outcome leaves the handler
//! with an error or a panic. A branch that returns `Ok` swallows the failure,
//! and a drain reached only on the failure side is not gated.

#![allow(dead_code)]

struct AccountView;

impl AccountView {
	fn lamports(&self) -> u64 {
		0
	}

	fn send_owned(
		&mut self,
		_program_id: &(),
		_lamports: u64,
		_recipient: &mut AccountView,
	) -> Result<(), ()> {
		Ok(())
	}
}

struct CapState {
	paused: bool,
}

impl CapState {
	fn assert_within_window_cap(&self) -> Result<(), ()> {
		if self.paused { Err(()) } else { Ok(()) }
	}
}

const ID: () = ();

// A wrapper that turns the guard's failure into success enforces nothing.
fn soft_policy(state: &CapState) -> Result<(), ()> {
	if state.assert_within_window_cap().is_err() {
		return Ok(());
	}

	Ok(())
}

fn soft_let_else_policy(state: &CapState) -> Result<(), ()> {
	let Ok(()) = state.assert_within_window_cap() else {
		return Ok(());
	};

	Ok(())
}

fn soft_match_policy(state: &CapState) -> Result<(), ()> {
	match state.assert_within_window_cap() {
		Ok(()) => {}
		Err(_) => return Ok(()),
	}

	Ok(())
}

// The same shapes with an error return do propagate the failure.
fn strict_policy(state: &CapState) -> Result<(), ()> {
	if state.assert_within_window_cap().is_err() {
		return Err(());
	}

	Ok(())
}

fn strict_match_policy(state: &CapState) -> Result<(), ()> {
	match state.assert_within_window_cap() {
		Ok(()) => Ok(()),
		Err(error) => Err(error),
	}
}

// A `bool` wrapper hides which outcome is the failure, so it never counts.
fn bool_policy(state: &CapState) -> bool {
	state.assert_within_window_cap().is_ok()
}

fn process_inverted_match(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	match state.assert_within_window_cap() {
		Ok(()) => return Ok(()),
		Err(_) => {}
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_inverted_if_let(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if let Ok(()) = state.assert_within_window_cap() {
		return Ok(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_inverted_is_ok(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if state.assert_within_window_cap().is_ok() {
		return Ok(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_inverted_let_else(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	let Err(()) = state.assert_within_window_cap() else {
		return Ok(());
	};

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// The failure arm returns, but with `Ok`: the failure is swallowed.
fn process_swallowing_match(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	match state.assert_within_window_cap() {
		Ok(()) => {}
		Err(_) => return Ok(()),
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// `!(bypass || passing)` is false whenever `bypass` holds, guard or not.
fn process_negated_or_bypass(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
	bypass: bool,
) -> Result<(), ()> {
	if !(bypass || state.assert_within_window_cap().is_ok()) {
		return Err(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_soft_wrapper(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	soft_policy(state)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_soft_let_else_wrapper(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	soft_let_else_policy(state)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_soft_match_wrapper(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	soft_match_policy(state)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_bool_wrapper_inverted(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if bool_policy(state) {
		return Ok(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// Even with the right polarity, a `bool` wrapper is not followed.
fn process_bool_wrapper_checked(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if !bool_policy(state) {
		return Err(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_strict_wrapper(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	strict_policy(state)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_strict_match_wrapper(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	strict_match_policy(state)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_failure_first_match(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	match state.assert_within_window_cap() {
		Err(error) => return Err(error),
		Ok(()) => {}
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_if_let_failure(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if let Err(error) = state.assert_within_window_cap() {
		return Err(error);
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_if_let_success_else(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if let Ok(()) = state.assert_within_window_cap() {
	} else {
		return Err(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// Either operand failing takes the erroring branch, so both are enforced.
fn process_or_of_failures(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
	closed: bool,
) -> Result<(), ()> {
	if closed || state.assert_within_window_cap().is_err() {
		return Err(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// `assert_eq!`/`assert_ne!` keep going only on the asserted value, which
// here is the failing one.
fn process_assert_eq_failure_true(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	assert_eq!(state.assert_within_window_cap().is_err(), true);

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_assert_ne_success_true(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	assert_ne!(state.assert_within_window_cap().is_ok(), true);

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_assert_failure(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	assert!(state.assert_within_window_cap().is_err());

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_is_ok_eq_true_returns(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if state.assert_within_window_cap().is_ok() == true {
		return Err(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_is_err_ne_false_else(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if state.assert_within_window_cap().is_err() != false {
	} else {
		return Err(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_and_passing_returns(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
	armed: bool,
) -> Result<(), ()> {
	if armed && state.assert_within_window_cap().is_ok() {
		return Err(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_bound_flag_negated(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	let failed = state.assert_within_window_cap().is_err();

	if !failed {
		return Err(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// The success arm consumes `Ok`, so the wildcard receives only the failure.
fn process_success_arm_then_wildcard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	match state.assert_within_window_cap() {
		Ok(()) => return Err(()),
		_ => {}
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_if_let_success_returns(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if let Ok(()) = state.assert_within_window_cap() {
		return Err(());
	} else {
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// `or(Some(..))` replaces the failure with success.
fn process_option_or_some(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	state.assert_within_window_cap().ok().or(Some(())).unwrap();

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// The failure arm consumes `Err`, so the wildcard receives only success.
fn process_failure_arm_then_wildcard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	match state.assert_within_window_cap() {
		Err(error) => return Err(error),
		_ => {}
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// Returning the scrutinee's own non-`Ok` binding propagates the failure.
fn process_returned_binding(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	match state.assert_within_window_cap() {
		Ok(()) => {}
		failure => return failure,
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// A catch-all binding that also receives `Ok` does not only propagate.
fn process_returned_catch_all(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
	strict: bool,
) -> Result<(), ()> {
	match state.assert_within_window_cap() {
		Ok(()) if strict => {}
		outcome => return outcome,
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn main() {}

// check-warn
