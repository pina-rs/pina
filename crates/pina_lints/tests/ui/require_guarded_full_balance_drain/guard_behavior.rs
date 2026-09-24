// normalize-stderr-test: "\n$" -> ""

//! A call satisfies the guard requirement by what it does, not by its name
//! alone: its failure must stop the handler before the drain, and it must
//! read the state it checks.

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

	fn within_withdrawal_limit(&self) -> bool {
		!self.paused
	}

	// Named like a guard, but cannot fail: calling it gates nothing.
	fn check_limits(&self) {}

	// A differently named wrapper that enforces the named guard.
	fn enforce_policy(&self) -> Result<(), ()> {
		self.assert_within_window_cap()?;

		Ok(())
	}
}

const ID: () = ();

const GLOBAL_LIMIT: u64 = 10;

fn check_limits() -> Result<(), ()> {
	if GLOBAL_LIMIT == 0 { Err(()) } else { Ok(()) }
}

fn assert_within_cap(state: &CapState) -> Result<(), ()> {
	state.assert_within_window_cap()
}

fn check_limit_of(value: u64) -> Result<(), ()> {
	if value > GLOBAL_LIMIT {
		Err(())
	} else {
		Ok(())
	}
}

// Returns the named guard's result, so a failure reaches the caller.
fn enforce_withdrawal_policy(state: &CapState) -> Result<(), ()> {
	state.assert_within_window_cap()
}

// Calls the named guard but discards its result, so it enforces nothing.
fn lax_withdrawal_policy(state: &CapState) -> Result<(), ()> {
	let _ = state.assert_within_window_cap();

	Ok(())
}

// Only reaches the guard on some paths, through itself.
fn recursive_policy(state: &CapState, depth: u8) -> Result<(), ()> {
	if depth == 0 {
		return Ok(());
	}

	recursive_policy(state, depth - 1)
}

// A guard-named call that cannot fail does not gate the drain.
fn process_unit_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	state.check_limits();

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// A guard whose result is discarded does not gate the drain.
fn process_discarded_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	let _ = state.assert_within_window_cap();

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// A zero-argument guard cannot read the state it claims to check.
fn process_zero_argument_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
) -> Result<(), ()> {
	check_limits()?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// A guard fed only compile-time values reads no state either.
fn process_literal_argument_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
) -> Result<(), ()> {
	check_limit_of(0)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// A propagated guard that takes the state gates the drain.
fn process_state_argument_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	assert_within_cap(state)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// A differently named wrapper counts when its body enforces the guard.
fn process_wrapped_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	enforce_withdrawal_policy(state)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_wrapped_method_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	state.enforce_policy()?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// A wrapper's result must be enforced as much as a named guard's.
fn process_discarded_wrapper(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	let _ = enforce_withdrawal_policy(state);

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// A wrapper that discards the guard inside enforces nothing.
fn process_lax_wrapper(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	lax_withdrawal_policy(state)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// Recursion never reaches a guard, and the analysis must still terminate.
fn process_recursive_wrapper(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	recursive_policy(state, 2)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// Enforcing the guard through a match that returns on failure counts.
fn process_matched_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	match state.assert_within_window_cap() {
		Ok(()) => {}
		Err(error) => return Err(error),
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// A match whose failure arm continues does not enforce the guard.
fn process_ignored_match_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	match state.assert_within_window_cap() {
		Ok(()) => {}
		Err(()) => {}
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_let_else_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	let Ok(()) = state.assert_within_window_cap() else {
		return Err(());
	};

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_mapped_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	state.assert_within_window_cap().map_err(|_| ())?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// A boolean guard counts when its failure branch returns.
fn process_boolean_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if !state.within_withdrawal_limit() {
		return Err(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// A boolean guard whose branch continues does not gate the drain.
fn process_logged_boolean_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
	log: &mut u64,
) -> Result<(), ()> {
	if !state.within_withdrawal_limit() {
		*log += 1;
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn main() {}

// check-warn
