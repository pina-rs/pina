// normalize-stderr-test: "\n$" -> ""

#![allow(const_item_mutation, dead_code, unused_assignments)]

struct AccountView {
	data: [u8; 8],
}

struct CloseAccounts<'a> {
	escrow: &'a mut AccountView,
	vault: &'a mut AccountView,
}

const ID: () = ();
const SCRATCH: [u8; 8] = [1; 8];

impl AccountView {
	fn try_borrow_mut(&mut self) -> Result<&mut [u8], ()> {
		Ok(&mut self.data)
	}

	// A same-named method is not the slice `fill` that zeroes the buffer.
	fn fill(&mut self, _value: u8) -> Result<(), ()> {
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

fn process_zeroed_close(state: &mut AccountView) -> Result<(), ()> {
	state.try_borrow_mut()?.fill(0);
	state.close()?;
	Ok(())
}

fn process_zeroed_buffer_close(state: &mut AccountView, recipient: ()) -> Result<(), ()> {
	let data = state.try_borrow_mut()?;
	data.fill(0);
	state.close_with_recipient(&ID, recipient)?;
	Ok(())
}

fn process_dereferenced_buffer_close(state: &mut AccountView) -> Result<(), ()> {
	let data = state.try_borrow_mut()?;
	(*data).fill(0);
	state.close()?;
	Ok(())
}

// Writing a field is not a rebinding, so it does not disturb the proof.
fn process_field_write_then_zeroed_close(state: &mut AccountView) -> Result<(), ()> {
	state.data[0] = 1;
	state.try_borrow_mut()?.fill(0);
	state.close()?;
	Ok(())
}

// Zeroing through an alias proves the aliased account.
fn process_zeroed_through_alias(mut state: &mut AccountView) -> Result<(), ()> {
	let alias = &mut state;
	alias.try_borrow_mut()?.fill(0);
	state.close()?;
	Ok(())
}

// Closing through an alias is closing the zeroed account.
fn process_closed_through_alias(state: &mut AccountView) -> Result<(), ()> {
	state.try_borrow_mut()?.fill(0);
	let alias = &mut *state;
	alias.close()?;
	Ok(())
}

fn process_zeroed_field_close(accounts: CloseAccounts<'_>) -> Result<(), ()> {
	accounts.escrow.try_borrow_mut()?.fill(0);
	accounts.escrow.close()?;
	Ok(())
}

fn process_combined_helper(state: &mut AccountView, recipient: ()) -> Result<(), ()> {
	// The combined helper zeroes and closes in one step, so it is exempt.
	state.close_account_zeroed(&ID, recipient)?;
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

fn process_other_account_zeroed(
	state: &mut AccountView,
	other: &mut AccountView,
) -> Result<(), ()> {
	other.try_borrow_mut()?.fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_other_account_zeroed_through_alias(
	state: &mut AccountView,
	other: &mut AccountView,
) -> Result<(), ()> {
	let alias = &mut *other;
	alias.try_borrow_mut()?.fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_other_field_zeroed(accounts: CloseAccounts<'_>) -> Result<(), ()> {
	accounts.vault.try_borrow_mut()?.fill(0);
	accounts.escrow.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// The alias records its binding-time value; after reassignment it names
// `other`, so zeroing through it does not prove `state`.
fn process_reassigned_alias(state: &mut AccountView, other: &mut AccountView) -> Result<(), ()> {
	let mut alias = &mut *state;
	alias = &mut *other;
	alias.try_borrow_mut()?.fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_reassigned_buffer(state: &mut AccountView, other: &mut AccountView) -> Result<(), ()> {
	let mut data = state.try_borrow_mut()?;
	data = other.try_borrow_mut()?;
	data.fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_partial_fill(state: &mut AccountView) -> Result<(), ()> {
	state.try_borrow_mut()?[..4].fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_partial_buffer_fill(state: &mut AccountView) -> Result<(), ()> {
	let data = state.try_borrow_mut()?;
	data[..4].fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_nonzero_fill(state: &mut AccountView) -> Result<(), ()> {
	state.try_borrow_mut()?.fill(1);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_unborrowed_buffer_fill(state: &mut AccountView) -> Result<(), ()> {
	let mut scratch = [1_u8; 8];
	let data = &mut scratch;
	data.fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// Only a literal zero is a recognizable wipe.
fn process_non_literal_fill(state: &mut AccountView) -> Result<(), ()> {
	let value = 0;
	state.try_borrow_mut()?.fill(value);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// Filling a constant's temporary copy does not touch any account.
fn process_constant_fill(state: &mut AccountView) -> Result<(), ()> {
	SCRATCH.fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_same_named_fill(state: &mut AccountView) -> Result<(), ()> {
	state.fill(0)?;
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_zeroed_after_close(state: &mut AccountView) -> Result<(), ()> {
	state.close()?;
	//~^ ERROR: account close should be preceded by
	state.try_borrow_mut()?.fill(0);
	Ok(())
}

fn main() {}

// compile-fail
