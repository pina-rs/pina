// normalize-stderr-test: "\n$" -> ""

//! Core trait methods chained onto a guard's `Result` are owned by the
//! trait, not by an impl of `Result`. The lint must classify them without
//! asking the trait for a self type (which used to crash the driver), and it
//! follows the ones that keep the failure observable.

#![allow(dead_code, clippy::useless_conversion, clippy::clone_on_copy)]

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

fn process_cloned_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	state.assert_within_window_cap().clone()?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_converted_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	let checked: Result<(), ()> = state.assert_within_window_cap().into();

	checked?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_from_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	Result::<(), ()>::from(state.assert_within_window_cap())?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// `guard.eq(&Ok(..))` is false exactly when the guard failed.
fn process_compared_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if state.assert_within_window_cap().eq(&Ok(())) {
	} else {
		return Err(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_compared_guard_ne(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if state.assert_within_window_cap().ne(&Ok(())) {
		return Err(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// Returning early when the guard *passed* leaves the drain to the failure.
fn process_compared_guard_inverted(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if state.assert_within_window_cap().eq(&Ok(())) {
		return Err(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// A cloned guard whose result is discarded still gates nothing.
fn process_cloned_guard_discarded(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	let _ = state.assert_within_window_cap().clone();

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn main() {}

// check-warn
