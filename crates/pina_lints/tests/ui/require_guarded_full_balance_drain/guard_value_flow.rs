// aux-build: pina.rs
// normalize-stderr-test: "\n$" -> ""

//! A guard's value may travel before it is enforced: through a local, a
//! failure-preserving combinator, or Pina's `assert` helper. It must still be
//! enforced on every path that reaches the drain.

#![allow(dead_code, unused_assignments)]

extern crate pina;

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

	fn is_paused(&self) -> bool {
		self.paused
	}
}

const ID: () = ();

fn process_bound_then_propagated(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	let checked = state.assert_within_window_cap();

	checked?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// Overwriting the bound result before propagating it discards the guard.
fn process_bound_then_overwritten(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	let mut checked = state.assert_within_window_cap();

	checked = Ok(());
	checked?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// A bound result propagated only in one branch does not gate the drain.
fn process_bound_then_branch_propagated(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
	strict: bool,
) -> Result<(), ()> {
	let checked = state.assert_within_window_cap();

	if strict {
		checked?;
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_pina_assert(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	pina::assert(!state.is_paused(), (), "paused")?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// `pina::assert` fails on `false`; asserting the failure itself inverts it.
fn process_pina_assert_inverted(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	pina::assert(state.assert_within_window_cap().is_err(), (), "inverted")?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_or_error(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	state.assert_within_window_cap().or(Err(()))?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_and_then(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	state.assert_within_window_cap().and_then(|()| Ok(()))?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_map_err(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	state.assert_within_window_cap().map_err(|_| ())?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_inspect_err(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	state.assert_within_window_cap().inspect_err(|_| {})?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// `or(Ok(..))` replaces the failure with success.
fn process_or_success(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	state.assert_within_window_cap().or(Ok(()))?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// `unwrap_or` swallows the failure.
fn process_unwrap_or(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	state.assert_within_window_cap().unwrap_or(());

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// The guard only runs when `bypass` is false.
fn process_short_circuit_bypass(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
	bypass: bool,
) -> Result<(), ()> {
	let _ = bypass || {
		state.assert_within_window_cap()?;
		true
	};

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// A closure's guard runs only if the closure is called.
fn process_guard_in_closure(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	let check = || state.assert_within_window_cap();
	let _ = check;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn main() {}

// check-warn
