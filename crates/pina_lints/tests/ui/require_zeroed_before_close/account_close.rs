// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

struct AccountView;

const ID: () = ();

impl AccountView {
	fn zeroed(&mut self) -> Result<(), ()> {
		Ok(())
	}

	fn close(&mut self) -> Result<(), ()> {
		Ok(())
	}

	fn close_with_recipient(&mut self, _program_id: &(), _recipient: ()) -> Result<(), ()> {
		Ok(())
	}

	fn close_account_zeroed(&mut self, _program_id: &(), _recipient: ()) -> Result<(), ()> {
		Ok(())
	}
}

fn process_close(state: &mut AccountView) -> Result<(), ()> {
	state.zeroed()?;
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_unchecked_close(state: &mut AccountView) -> Result<(), ()> {
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_recipient_close(state: &mut AccountView, recipient: ()) -> Result<(), ()> {
	state.close_with_recipient(&ID, recipient)?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_combined_helper(state: &mut AccountView, recipient: ()) -> Result<(), ()> {
	// The combined helper zeroes and closes in one step, so it is exempt.
	state.close_account_zeroed(&ID, recipient)?;
	Ok(())
}

fn main() {}

// compile-fail
