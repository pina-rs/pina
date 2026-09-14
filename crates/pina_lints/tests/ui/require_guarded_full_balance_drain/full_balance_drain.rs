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

struct CapState;

impl CapState {
	fn assert_within_window_cap(&self) -> Result<(), ()> {
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

fn non_handler_sweep(vault: &mut AccountView, recipient: &mut AccountView) -> Result<(), ()> {
	vault.send_owned(&ID, vault.lamports(), recipient)
}

fn main() {}

// check-warn
