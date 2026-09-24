// normalize-stderr-test: "\n$" -> ""

//! A failing branch is recognized by its shape, not by its type: a tail
//! `return Err(..)`, a panic, an assertion macro, or `Err(..)?` all leave the
//! handler even when the branch's type was coerced to `()`.

#![allow(dead_code, clippy::bool_comparison)]

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

	fn is_paused(&self) -> bool {
		self.paused
	}
}

const ID: () = ();

fn enforce_withdrawal_policy(state: &CapState) -> Result<(), ()> {
	state.assert_within_window_cap()?;

	Ok(())
}

// A tail `return Err(..)` without a semicolon.
fn process_return_tail(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if !state.within_withdrawal_limit() {
		return Err(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_return_statement(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if !state.within_withdrawal_limit() {
		return Err(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_panic_tail(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if state.is_paused() {
		panic!("paused")
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_panic_statement(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if state.is_paused() {
		panic!("paused");
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_match_block_arm(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	match state.assert_within_window_cap() {
		Ok(()) => {}
		Err(error) => {
			return Err(error);
		}
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_unwrap(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	state.assert_within_window_cap().unwrap();

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_wrapper_propagated(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	enforce_withdrawal_policy(state)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_assert_macro(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	assert!(!state.is_paused());

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_assert_eq_macro(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	assert_eq!(state.is_paused(), false);

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_assert_eq_fallible(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	assert_eq!(state.assert_within_window_cap(), Ok(()));

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_eq_false(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if state.within_withdrawal_limit() == false {
		return Err(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_else_branch(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if state.within_withdrawal_limit() {
	} else {
		return Err(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_error_question_mark(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if state.is_paused() {
		Err(())?;
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// Returning `Ok` from the paused branch skips the drain but reports success:
// the pause did not fail the instruction, yet nothing is swept either way.
fn process_paused_ok_return(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if state.is_paused() {
		return Ok(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// A branch that only logs neither returns nor panics.
fn process_logging_branch(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
	log: &mut u64,
) -> Result<(), ()> {
	if state.is_paused() {
		*log += 1;
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// `debug_assert!` is compiled out of release builds.
fn process_debug_assert(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	debug_assert!(!state.is_paused());

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// `assert_ne!` against `Ok` panics only when the guard passes.
fn process_assert_ne_fallible(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	assert_ne!(state.assert_within_window_cap(), Ok(()));

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn reject() -> Result<(), ()> {
	Err(())
}

fn allow() -> Result<(), ()> {
	Ok(())
}

// Returning a local helper that can only fail is an error return.
fn process_return_rejecting_helper(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if state.is_paused() {
		return reject();
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// Returning a local helper that can only succeed is not.
fn process_return_allowing_helper(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if state.is_paused() {
		return allow();
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_error_conversion(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if state.is_paused() {
		return Err(().into());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn main() {}

// check-warn
