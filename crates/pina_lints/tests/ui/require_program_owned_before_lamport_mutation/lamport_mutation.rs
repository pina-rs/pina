// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

struct AccountView;

const OWNER: () = ();

impl AccountView {
	fn assert_owner(&self, _program: &()) -> Result<(), ()> {
		Ok(())
	}

	fn assert_owners(&self, _programs: &[()]) -> Result<(), ()> {
		Ok(())
	}

	fn assert_type<T>(&self, _program: &()) -> Result<(), ()> {
		Ok(())
	}

	fn send(&self, _amount: u64, _recipient: ()) -> Result<(), ()> {
		Ok(())
	}
}

fn process_withdraw(vault: &AccountView, recipient: ()) -> Result<(), ()> {
	vault.assert_owner(&OWNER)?;
	vault.send(5, recipient)?;
	Ok(())
}

fn process_unchecked(vault: &AccountView, recipient: ()) -> Result<(), ()> {
	vault.send(10, recipient)?;
	//~^ ERROR: lamport mutation should be preceded by
	Ok(())
}

fn process_wrong_receiver(
	vault: &AccountView,
	other: &AccountView,
	recipient: (),
) -> Result<(), ()> {
	other.assert_owner(&OWNER)?;
	vault.send(15, recipient)?;
	//~^ ERROR: lamport mutation should be preceded by
	Ok(())
}

fn process_order_does_not_help(vault: &AccountView, recipient: ()) -> Result<(), ()> {
	vault.send(20, recipient)?;
	//~^ ERROR: lamport mutation should be preceded by
	vault.assert_owner(&OWNER)?;
	Ok(())
}

fn process_assert_type_guards(vault: &AccountView, recipient: ()) -> Result<(), ()> {
	// `assert_type` performs an ownership check for typed accounts.
	vault.assert_type::<u8>(&OWNER)?;
	vault.send(25, recipient)?;
	Ok(())
}

fn main() {}
