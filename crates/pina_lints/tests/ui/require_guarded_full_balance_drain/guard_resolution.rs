// normalize-stderr-test: "\n$" -> ""

//! A call is judged by the body that actually runs: trait calls resolve to
//! the implementation, wrappers are followed to a fixed depth, and a local
//! callee that can only succeed is not a guard whatever its name.

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

	// Named like a pause check, but always reports "not paused".
	fn is_paused(&self) -> bool {
		false
	}
}

const ID: () = ();
const DEFAULT_LIMIT: u64 = 10;

trait AsCap {
	fn as_cap(&self) -> &CapState;
}

impl AsCap for CapState {
	fn as_cap(&self) -> &CapState {
		self
	}
}

trait Policy: AsCap {
	// The default enforces the cap...
	fn apply_rules(&self) -> Result<(), ()> {
		self.as_cap().assert_within_window_cap()?;

		Ok(())
	}

	fn apply_default_rules(&self) -> Result<(), ()> {
		self.as_cap().assert_within_window_cap()?;

		Ok(())
	}

	fn enforce_cap(&self) -> Result<(), ()>;
}

impl Policy for CapState {
	// ...but this implementation overrides it with a no-op.
	fn apply_rules(&self) -> Result<(), ()> {
		Ok(())
	}

	fn enforce_cap(&self) -> Result<(), ()> {
		self.assert_within_window_cap()
	}
}

fn check_limit_of(value: u64) -> Result<(), ()> {
	if value > DEFAULT_LIMIT {
		Err(())
	} else {
		Ok(())
	}
}

// Named like a cap check, but can only succeed.
fn cap_ok(_state: &CapState) -> Result<(), ()> {
	Ok(())
}

// Returns early with success when `bypass` is set, skipping the guard.
fn bypassable_policy(state: &CapState, bypass: bool) -> Result<(), ()> {
	if bypass {
		return Ok(());
	}

	state.assert_within_window_cap()?;

	Ok(())
}

// Checks a pause flag without a guard-named call.
fn live_check(paused: bool) -> Result<(), ()> {
	if paused {
		return Err(());
	}

	Ok(())
}

fn w1(state: &CapState) -> Result<(), ()> {
	w2(state)
}

fn w2(state: &CapState) -> Result<(), ()> {
	w3(state)
}

fn w3(state: &CapState) -> Result<(), ()> {
	w4(state)
}

fn w4(state: &CapState) -> Result<(), ()> {
	state.assert_within_window_cap()
}

fn process_trait_override(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	state.apply_rules()?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// The implementation inherits the default, which enforces the cap.
fn process_trait_default(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	state.apply_default_rules()?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_trait_implementation(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	state.enforce_cap()?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// Unresolved generic calls fall back to the method name alone.
fn process_generic_named<P: Policy>(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	policy: &P,
) -> Result<(), ()> {
	policy.enforce_cap()?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_generic_unnamed<P: Policy>(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	policy: &P,
) -> Result<(), ()> {
	policy.apply_default_rules()?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_wrapper_depth_three(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	w2(state)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// Four wrappers deep is past the analysis limit.
fn process_wrapper_depth_four(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	w1(state)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_bypassable_wrapper(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
	bypass: bool,
) -> Result<(), ()> {
	bypassable_policy(state, bypass)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_constant_success(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	cap_ok(state)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn process_constant_flag(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if state.is_paused() {
		return Err(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// A literal bound to a local is still not handler state.
fn process_local_literal(vault: &mut AccountView, recipient: &mut AccountView) -> Result<(), ()> {
	let zero = 0;

	check_limit_of(zero)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// A value derived from a parameter through locals is handler state.
fn process_derived_argument(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	amount: u64,
) -> Result<(), ()> {
	let doubled = amount * 2;
	let requested = doubled;

	check_limit_of(requested)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// `for` desugars to an `Iterator::next` call that typeck records without
// instantiation arguments; the analysis must skip it rather than resolve it.
// A guard inside the loop body may never run, so the drain is still flagged.
fn process_guard_in_for_loop(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	amounts: &[u64],
) -> Result<(), ()> {
	for amount in amounts {
		check_limit_of(*amount)?;
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// Known limit: a pause check with no guard-named call is not recognized.
fn process_unnamed_pause_helper(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	live_check(state.paused)?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

fn main() {}

// check-warn
