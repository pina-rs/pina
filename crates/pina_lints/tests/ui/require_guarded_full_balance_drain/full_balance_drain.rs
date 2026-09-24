// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

struct AccountView;

impl AccountView {
	fn lamports(&self) -> u64 {
		0
	}

	fn send(
		&mut self,
		_program_id: &(),
		_lamports: u64,
		_recipient: &mut AccountView,
	) -> Result<(), ()> {
		Ok(())
	}

	fn send_owned(
		&mut self,
		_program_id: &(),
		_lamports: u64,
		_recipient: &mut AccountView,
	) -> Result<(), ()> {
		Ok(())
	}

	fn zeroed(&mut self) -> Result<(), ()> {
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

	fn touch(&self) -> Result<(), ()> {
		Ok(())
	}
}

const ID: () = ();

fn process_sweep(vault: &mut AccountView, recipient: &mut AccountView) -> Result<(), ()> {
	vault.send_owned(&ID, vault.lamports(), recipient)?;
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
	Ok(())
}

fn process_sweep_aliased(vault: &mut AccountView, recipient: &mut AccountView) -> Result<(), ()> {
	let balance = vault.lamports();

	vault.send(&ID, balance, recipient)?;
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
	Ok(())
}

fn process_sweep_capped(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	state.assert_within_window_cap()?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_close_drain(vault: &mut AccountView, recipient: &mut AccountView) -> Result<(), ()> {
	vault.zeroed()?;

	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn process_partial_send(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	amount: u64,
) -> Result<(), ()> {
	vault.send_owned(&ID, amount, recipient)
}

// A share of the balance is not a drain: only the exact balance is reported.
fn process_half_balance_send(
	vault: &mut AccountView,
	recipient: &mut AccountView,
) -> Result<(), ()> {
	let half = vault.lamports() / 2;

	vault.send_owned(&ID, half, recipient)
}

fn process_reserve_kept_send(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	reserve: u64,
) -> Result<(), ()> {
	let amount = vault.lamports() - reserve;

	vault.send_owned(&ID, amount, recipient)
}

fn process_inline_fraction_send(
	vault: &mut AccountView,
	recipient: &mut AccountView,
) -> Result<(), ()> {
	vault.send_owned(&ID, vault.lamports() / 2, recipient)
}

// Rebinding the balance before sending keeps it a drain.
fn process_rebound_drain(vault: &mut AccountView, recipient: &mut AccountView) -> Result<(), ()> {
	let balance = vault.lamports();
	let amount = balance;

	vault.send_owned(&ID, amount, recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// The amount must belong to the account being drained: another account's
// balance is not this account's full balance.
fn process_other_account_balance(
	vault: &mut AccountView,
	other: &AccountView,
	recipient: &mut AccountView,
) -> Result<(), ()> {
	let amount = other.lamports();

	vault.send_owned(&ID, amount, recipient)
}

// An unrelated call that is not guard-shaped must not suppress the warning.
fn process_unrelated_call(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	state.touch()?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// A guard confined to one branch does not dominate a drain on another path.
fn process_branch_local_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
	enabled: bool,
) -> Result<(), ()> {
	if enabled {
		state.assert_within_window_cap()?;
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// A guard that runs after the sweep cannot make the sweep safe.
fn process_guard_after_drain(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	vault.send_owned(&ID, vault.lamports(), recipient)?;
	//~^ WARN: an instruction path can sweep an account's entire balance in one call

	state.assert_within_window_cap()?;

	Ok(())
}

// An early-return guard in the condition runs on every path that reaches the
// drain, so it suppresses the warning.
fn process_early_return_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	if state.assert_within_window_cap().is_err() {
		return Err(());
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
}

// A guard inside a loop body may never run, so it does not dominate.
fn process_loop_body_guard(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	state: &CapState,
) -> Result<(), ()> {
	for _ in 0..1 {
		state.assert_within_window_cap()?;
	}

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// A close call on a different account does not make this drain safe.
fn process_other_account_close(
	vault: &mut AccountView,
	recipient: &mut AccountView,
	other: &mut AccountView,
) -> Result<(), ()> {
	other.zeroed()?;

	vault.send_owned(&ID, vault.lamports(), recipient)
	//~^ WARN: an instruction path can sweep an account's entire balance in one call
}

// A close that runs after the sweep does not make the sweep safe.
fn process_close_after_drain(
	vault: &mut AccountView,
	recipient: &mut AccountView,
) -> Result<(), ()> {
	vault.send_owned(&ID, vault.lamports(), recipient)?;
	//~^ WARN: an instruction path can sweep an account's entire balance in one call

	vault.zeroed()?;

	Ok(())
}

fn non_handler_sweep(vault: &mut AccountView, recipient: &mut AccountView) -> Result<(), ()> {
	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn main() {}

// check-warn
