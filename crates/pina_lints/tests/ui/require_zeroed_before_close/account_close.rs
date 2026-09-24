// normalize-stderr-test: "\n$" -> ""
// aux-build: solana_account_view.rs

#![allow(const_item_mutation, dead_code, unused_assignments, unused_mut)]

extern crate solana_account_view;

use solana_account_view::AccountView;
use solana_account_view::ProgramError;

struct Ctx<'a> {
	escrow: &'a mut AccountView,
	vault: &'a mut AccountView,
}

struct Iter<'a>(core::slice::IterMut<'a, AccountView>);

struct Accounts {
	list: [AccountView; 2],
}

// A user type whose `try_borrow_mut` returns an owned copy of the data.
struct CopyView {
	data: [u8; 8],
}

const ID: () = ();
const SCRATCH: [u8; 8] = [1; 8];

trait CloseAccountWithRecipient {
	fn close_with_recipient(&mut self, program_id: &(), recipient: ()) -> Result<(), ProgramError>;
	fn close_account_zeroed(&mut self, program_id: &(), recipient: ()) -> Result<(), ProgramError>;
	// A same-named method is not the slice `fill` that zeroes the buffer.
	fn fill(&mut self, value: u8) -> Result<(), ProgramError>;
}

impl CloseAccountWithRecipient for AccountView {
	fn close_with_recipient(&mut self, _: &(), _: ()) -> Result<(), ProgramError> {
		self.close()
	}

	fn close_account_zeroed(&mut self, _: &(), _: ()) -> Result<(), ProgramError> {
		self.close()
	}

	fn fill(&mut self, _: u8) -> Result<(), ProgramError> {
		Ok(())
	}
}

impl Accounts {
	fn escrow(&mut self) -> &mut AccountView {
		&mut self.list[0]
	}

	fn vault(&mut self) -> &mut AccountView {
		&mut self.list[1]
	}
}

impl CopyView {
	fn try_borrow_mut(&self) -> Result<[u8; 8], ProgramError> {
		Ok(self.data)
	}

	fn close(&mut self) -> Result<(), ProgramError> {
		Ok(())
	}
}

fn next_account<'a>(iter: &mut Iter<'a>) -> Result<&'a mut AccountView, ProgramError> {
	iter.0.next().ok_or(ProgramError::AccountBorrowFailed)
}

fn pick<'a>(_first: &'a mut AccountView, second: &'a mut AccountView) -> &'a mut AccountView {
	second
}

fn doubler() -> fn(u8) -> u8 {
	|value| value * 2
}

// ---------------------------------------------------------------------------
// Accepted: the same account is zeroed on every path before its close.
// ---------------------------------------------------------------------------

fn process_zeroed_close(state: &mut AccountView) -> Result<(), ProgramError> {
	state.try_borrow_mut()?.fill(0);
	state.close()?;
	Ok(())
}

fn process_zeroed_buffer_close(state: &mut AccountView) -> Result<(), ProgramError> {
	let mut data = state.try_borrow_mut()?;
	data.fill(0);
	drop(data);
	state.close_with_recipient(&ID, ())?;
	Ok(())
}

fn process_dereferenced_buffer_close(state: &mut AccountView) -> Result<(), ProgramError> {
	let mut data = state.try_borrow_mut()?;
	(*data).fill(0);
	drop(data);
	state.close()?;
	Ok(())
}

// F03: a suffixed zero literal.
fn process_suffixed_zero(state: &mut AccountView) -> Result<(), ProgramError> {
	state.try_borrow_mut()?.fill(0u8);
	state.close()?;
	Ok(())
}

// F04: `[..]` addresses the whole buffer.
fn process_full_range(state: &mut AccountView) -> Result<(), ProgramError> {
	state.try_borrow_mut()?[..].fill(0);
	state.close()?;
	Ok(())
}

fn process_full_range_buffer(state: &mut AccountView) -> Result<(), ProgramError> {
	let mut data = state.try_borrow_mut()?;
	data[..].fill(0);
	drop(data);
	state.close()?;
	Ok(())
}

// F05: a scoped block is not a branch.
fn process_scoped_block(state: &mut AccountView) -> Result<(), ProgramError> {
	{
		let mut data = state.try_borrow_mut()?;
		data.fill(0);
	}
	state.close()?;
	Ok(())
}

// F06: a destructured binding is its own account.
fn process_destructured(accounts: &mut [AccountView]) -> Result<(), ProgramError> {
	let [state, _rest @ ..] = accounts else {
		return Err(ProgramError::AccountBorrowFailed);
	};
	state.try_borrow_mut()?.fill(0);
	state.close()?;
	Ok(())
}

// A binding from a call is its own account, so zeroing and closing it agree.
fn process_derived_same_binding(accounts: &mut [AccountView]) -> Result<(), ProgramError> {
	let mut iter = Iter(accounts.iter_mut());
	let state = next_account(&mut iter)?;
	state.try_borrow_mut()?.fill(0);
	state.close()?;
	Ok(())
}

// Zeroing through an alias proves the aliased account.
fn process_zeroed_through_alias(mut state: &mut AccountView) -> Result<(), ProgramError> {
	let alias = &mut state;
	alias.try_borrow_mut()?.fill(0);
	state.close()?;
	Ok(())
}

// Closing through an alias is closing the zeroed account.
fn process_closed_through_alias(state: &mut AccountView) -> Result<(), ProgramError> {
	state.try_borrow_mut()?.fill(0);
	let alias = &mut *state;
	alias.close()?;
	Ok(())
}

fn process_zeroed_field_close(ctx: Ctx<'_>) -> Result<(), ProgramError> {
	ctx.escrow.try_borrow_mut()?.fill(0);
	ctx.escrow.close()?;
	Ok(())
}

// F01: a field reached through an alias of its root.
fn process_field_through_root_alias(ctx: Ctx<'_>) -> Result<(), ProgramError> {
	let mut ctx = ctx;
	let root = &mut ctx;
	root.escrow.try_borrow_mut()?.fill(0);
	ctx.escrow.close()?;
	Ok(())
}

fn process_field_alias(ctx: Ctx<'_>) -> Result<(), ProgramError> {
	let escrow = &mut *ctx.escrow;
	escrow.try_borrow_mut()?.fill(0);
	ctx.escrow.close()?;
	Ok(())
}

// Zeroing inside the branch that closes proves that close.
fn process_zeroed_in_closing_branch(
	state: &mut AccountView,
	close: bool,
) -> Result<(), ProgramError> {
	if close {
		state.try_borrow_mut()?.fill(0);
		state.close()?;
	}
	Ok(())
}

// Each branch zeroes before its own close. Calls that are not `drop` and a
// closure that captures nothing leave the proof alone.
fn process_zeroed_in_each_branch(state: &mut AccountView, now: bool) -> Result<(), ProgramError> {
	let constant = || 0_u8;
	let _ = constant();
	let triple = |value: u8| value * 3;
	let _ = triple(1);
	let _ = doubler()(1);
	if now {
		state.try_borrow_mut()?.fill(0);
		state.close()?;
	} else {
		state.try_borrow_mut()?.fill(0);
		state.close()?;
	}
	Ok(())
}

fn process_combined_helper(state: &mut AccountView) -> Result<(), ProgramError> {
	// The combined helper zeroes and closes in one step, so it is exempt.
	state.close_account_zeroed(&ID, ())?;
	Ok(())
}

// ---------------------------------------------------------------------------
// Flagged: no zeroing of the closed account.
// ---------------------------------------------------------------------------

fn process_unchecked_close(state: &mut AccountView) -> Result<(), ProgramError> {
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_recipient_close(state: &mut AccountView) -> Result<(), ProgramError> {
	state.close_with_recipient(&ID, ())?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_zeroed_after_close(state: &mut AccountView) -> Result<(), ProgramError> {
	state.close()?;
	//~^ ERROR: account close should be preceded by
	state.try_borrow_mut()?.fill(0);
	Ok(())
}

// ---------------------------------------------------------------------------
// Flagged: a different account was zeroed.
// ---------------------------------------------------------------------------

fn process_other_account_zeroed(
	state: &mut AccountView,
	other: &mut AccountView,
) -> Result<(), ProgramError> {
	other.try_borrow_mut()?.fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_other_account_zeroed_through_alias(
	state: &mut AccountView,
	other: &mut AccountView,
) -> Result<(), ProgramError> {
	let alias = &mut *other;
	alias.try_borrow_mut()?.fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_other_field_zeroed(ctx: Ctx<'_>) -> Result<(), ProgramError> {
	ctx.vault.try_borrow_mut()?.fill(0);
	ctx.escrow.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// P01: indexed receivers have no identity.
fn process_indexed(accounts: &mut [AccountView]) -> Result<(), ProgramError> {
	accounts[0].try_borrow_mut()?.fill(0);
	accounts[1].close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// P13: neither indexed close is proven.
fn process_indexed_two_closes(accounts: &mut [AccountView]) -> Result<(), ProgramError> {
	accounts[0].try_borrow_mut()?.fill(0);
	accounts[0].close()?;
	//~^ ERROR: account close should be preceded by
	accounts[1].close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// P02: two values drawn from one iterator are distinct accounts.
fn process_iterator_accounts(accounts: &mut [AccountView]) -> Result<(), ProgramError> {
	let mut iter = Iter(accounts.iter_mut());
	let state = next_account(&mut iter)?;
	let vault = next_account(&mut iter)?;
	vault.try_borrow_mut()?.fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// P03: method-call receivers have no identity.
fn process_getters(accounts: &mut Accounts) -> Result<(), ProgramError> {
	accounts.vault().try_borrow_mut()?.fill(0);
	accounts.escrow().close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// P10: a function result is its own account, not its first argument.
fn process_function_result(
	state: &mut AccountView,
	other: &mut AccountView,
) -> Result<(), ProgramError> {
	let picked = pick(&mut *state, &mut *other);
	picked.try_borrow_mut()?.fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// ---------------------------------------------------------------------------
// Flagged: the binding may hold a different account by the close.
// ---------------------------------------------------------------------------

// P04: a shadowed field root is a different binding.
fn process_shadowed_field_root(first: Ctx<'_>, second: Ctx<'_>) -> Result<(), ProgramError> {
	let ctx = first;
	ctx.escrow.try_borrow_mut()?.fill(0);
	let ctx = second;
	ctx.escrow.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// Lending a field's slot lets the borrower replace the account it holds.
fn process_field_slot_lent<'a>(
	ctx: Ctx<'a>,
	spare: &'a mut AccountView,
) -> Result<(), ProgramError> {
	let mut ctx = ctx;
	let mut spare = spare;
	ctx.escrow.try_borrow_mut()?.fill(0);
	core::mem::swap(&mut ctx.escrow, &mut spare);
	ctx.escrow.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// P05: a reassigned field root.
fn process_reassigned_field_root(first: Ctx<'_>, second: Ctx<'_>) -> Result<(), ProgramError> {
	let mut ctx = first;
	ctx.escrow.try_borrow_mut()?.fill(0);
	ctx = second;
	ctx.escrow.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_reassigned_alias(
	state: &mut AccountView,
	other: &mut AccountView,
) -> Result<(), ProgramError> {
	let mut alias = &mut *state;
	alias = &mut *other;
	alias.try_borrow_mut()?.fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// P06: `mem::swap` retargets the alias through its slot.
fn process_swapped_alias(
	state: &mut AccountView,
	other: &mut AccountView,
) -> Result<(), ProgramError> {
	let mut first = &mut *state;
	let mut second = &mut *other;
	core::mem::swap(&mut first, &mut second);
	first.try_borrow_mut()?.fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// P07: a write through a reference to the alias retargets it.
fn process_retargeted_through_reference(
	state: &mut AccountView,
	other: &mut AccountView,
) -> Result<(), ProgramError> {
	let mut alias = &mut *state;
	let slot = &mut alias;
	*slot = &mut *other;
	alias.try_borrow_mut()?.fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// A closure capture could rewrite the binding, so it proves nothing.
fn process_captured_alias(state: &mut AccountView) -> Result<(), ProgramError> {
	let alias = &mut *state;
	let inspect = || {
		let _ = &alias;
	};
	inspect();
	alias.try_borrow_mut()?.fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_reassigned_buffer(
	state: &mut AccountView,
	other: &mut AccountView,
) -> Result<(), ProgramError> {
	let mut data = state.try_borrow_mut()?;
	data = other.try_borrow_mut()?;
	data.fill(0);
	drop(data);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// ---------------------------------------------------------------------------
// Flagged: not a whole-buffer zero fill of the account's data.
// ---------------------------------------------------------------------------

fn process_partial_fill(state: &mut AccountView) -> Result<(), ProgramError> {
	state.try_borrow_mut()?[..4].fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_partial_buffer_fill(state: &mut AccountView) -> Result<(), ProgramError> {
	let mut data = state.try_borrow_mut()?;
	data[..4].fill(0);
	drop(data);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_nonzero_fill(state: &mut AccountView) -> Result<(), ProgramError> {
	state.try_borrow_mut()?.fill(1);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// Only a literal zero is a recognizable wipe.
fn process_non_literal_fill(state: &mut AccountView) -> Result<(), ProgramError> {
	let value = 0;
	state.try_borrow_mut()?.fill(value);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_unborrowed_buffer_fill(state: &mut AccountView) -> Result<(), ProgramError> {
	let mut scratch = [1_u8; 8];
	let data = &mut scratch;
	data.fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// Filling a constant's temporary copy does not touch any account.
fn process_constant_fill(state: &mut AccountView) -> Result<(), ProgramError> {
	SCRATCH.fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_same_named_fill(state: &mut AccountView) -> Result<(), ProgramError> {
	state.fill(0)?;
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// P09: a same-named `try_borrow_mut` returning a copy is not the account data.
fn process_copied_data(state: &mut CopyView) -> Result<(), ProgramError> {
	state.try_borrow_mut()?.fill(0);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// ---------------------------------------------------------------------------
// Flagged: the zeroing does not hold on every path to the close.
// ---------------------------------------------------------------------------

// P11: zeroing in one branch only.
fn process_zeroed_in_one_branch(state: &mut AccountView, zero: bool) -> Result<(), ProgramError> {
	if zero {
		state.try_borrow_mut()?.fill(0);
	}
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_zeroed_in_one_arm(
	state: &mut AccountView,
	zero: Option<u8>,
) -> Result<(), ProgramError> {
	match zero {
		Some(_) => state.try_borrow_mut()?.fill(0),
		None => {}
	}
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_zeroed_on_short_circuit(
	state: &mut AccountView,
	zero: bool,
) -> Result<(), ProgramError> {
	let _ = zero && {
		state.try_borrow_mut()?.fill(0);
		true
	};
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

// P12: a write after the zeroing.
fn process_rewritten_after_zeroing(state: &mut AccountView) -> Result<(), ProgramError> {
	state.try_borrow_mut()?.fill(0);
	state.try_borrow_mut()?[0] = 7;
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn process_buffer_rewritten_after_zeroing(state: &mut AccountView) -> Result<(), ProgramError> {
	let mut data = state.try_borrow_mut()?;
	data.fill(0);
	data[0] = 7;
	drop(data);
	state.close()?;
	//~^ ERROR: account close should be preceded by
	Ok(())
}

fn main() {}

// compile-fail
