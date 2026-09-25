// normalize-stderr-test: "\n$" -> ""

//! A wrapper is judged by what its body does for the concrete caller: a
//! `return` whose value the lint cannot see into may be a success that
//! swallows the guard's failure, and a trait call inside a generic wrapper
//! runs the impl the caller picked, not whatever the trait method is named.

#![allow(dead_code)]

use core::convert::identity;

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

trait Finish {
	fn finish(&self) -> Result<(), ()>;
}

struct Done;

impl Finish for Done {
	fn finish(&self) -> Result<(), ()> {
		Ok(())
	}
}

trait Guarded {
	fn check_cap(&self) -> Result<(), ()>;
}

// Named like a cap check, but lets everything through.
struct Lax;

impl Guarded for Lax {
	fn check_cap(&self) -> Result<(), ()> {
		Ok(())
	}
}

impl Guarded for CapState {
	fn check_cap(&self) -> Result<(), ()> {
		self.assert_within_window_cap()
	}
}

fn into_soft_policy(state: &CapState) -> Result<(), ()> {
	if state.assert_within_window_cap().is_err() {
		return Ok(()).into();
	}

	Ok(())
}

fn identity_soft_policy(state: &CapState) -> Result<(), ()> {
	if state.assert_within_window_cap().is_err() {
		return identity(Ok(()));
	}

	Ok(())
}

fn map_soft_policy(state: &CapState) -> Result<(), ()> {
	match state.assert_within_window_cap() {
		Ok(()) => {}
		Err(_) => return Ok(()).map(|value: ()| value),
	}

	Ok(())
}

fn dyn_soft_policy(state: &CapState, finish: &dyn Finish) -> Result<(), ()> {
	if state.assert_within_window_cap().is_err() {
		return finish.finish();
	}

	Ok(())
}

fn generic_soft_policy<F: Finish>(state: &CapState, finish: &F) -> Result<(), ()> {
	let Ok(()) = state.assert_within_window_cap() else {
		return finish.finish();
	};

	Ok(())
}

fn generic_policy<T>(guarded: &T) -> Result<(), ()>
where
	T: Guarded,
{
	guarded.check_cap()
}

fn dyn_policy(guarded: &dyn Guarded) -> Result<(), ()> {
	guarded.check_cap()
}

fn process_into_soft(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	into_soft_policy(state)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_identity_soft(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	identity_soft_policy(state)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_map_soft(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	map_soft_policy(state)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_dyn_soft(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	dyn_soft_policy(state, &Done)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_generic_soft(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	generic_soft_policy(state, &Done)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// The generic wrapper runs `Lax::check_cap`, which lets everything through.
fn process_generic_wrapper_lax(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	lax: &Lax,
) -> Result<(), ()> {
	generic_policy(lax)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// The same wrapper instantiated with the real guard is accepted.
fn process_generic_wrapper_capped(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	generic_policy(state)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// A `dyn` call inside a wrapper cannot be resolved, so it is unknown.
fn process_dyn_wrapper(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	dyn_policy(state)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// In the handler itself any `return` skips the drain, so an opaque returned
// value on the failing side keeps the name rule's leniency.
fn process_handler_opaque_return(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if state.assert_within_window_cap().is_err() {
		return identity(Err(()));
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn main() {}

// check-warn
