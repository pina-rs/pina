// check-warn
// aux-build: solana_account_view.rs
// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code, unused_variables)]

extern crate solana_account_view;

use solana_account_view::AccountView;
use solana_account_view::Ref as Guard;
use solana_account_view::Value;

struct UnrelatedGuard;
struct Unrelated;
struct MintView;
struct State;

impl Unrelated {
	fn as_account(&self) -> Result<UnrelatedGuard, ()> {
		Ok(UnrelatedGuard)
	}
}

trait AsTokenAccount {
	fn as_token_mint_for_program(&self, program: &u8) -> Result<Guard, ()>;
	fn as_associated_token_account(&self, owner: &u8) -> Result<Guard, ()>;
}

impl AsTokenAccount for MintView {
	fn as_token_mint_for_program(&self, _program: &u8) -> Result<Guard, ()> {
		Ok(Guard)
	}

	fn as_associated_token_account(&self, _owner: &u8) -> Result<Guard, ()> {
		Ok(Guard)
	}
}

impl State {
	fn load_pda(_account: &AccountView) -> Result<Guard, ()> {
		Ok(Guard)
	}
}

macro_rules! bind_guard {
	($account:expr) => {
		let guard = $account.try_borrow()?;
	};
}

const UNRELATED_CONSTANT: u8 = 7;

mod shadowed {
	pub fn drop<T>(_: T) {}
}

struct Loader;

impl Loader {
	fn load(&self) -> fn(&AccountView) -> Result<Guard, ()> {
		State::load_pda
	}
}

fn dropped_without_reading(account: &mut AccountView) -> Result<(), ()> {
	let guard = account.try_borrow_mut()?;
	//~^ ERROR: account borrow guard `guard` is never read
	drop(guard);
	Ok(())
}

fn underscore_prefix_is_still_unused(mint: &MintView) -> Result<(), ()> {
	let _guard = mint.as_token_mint_for_program(&0)?;
	//~^ ERROR: account borrow guard `_guard` is never read
	Ok(())
}

fn never_read(mint: &MintView) -> Result<(), ()> {
	let guard = mint.as_associated_token_account(&0)?;
	//~^ ERROR: account borrow guard `guard` is never read
	Ok(())
}

fn block_scoped_without_reading(account: &AccountView) -> Result<(), ()> {
	{
		let guard = State::load_pda(account)?;
		//~^ ERROR: account borrow guard `guard` is never read
	}
	Ok(())
}

fn block_tail_initializer(account: &AccountView) -> Result<(), ()> {
	let guard = { account.try_borrow()? };
	//~^ ERROR: account borrow guard `guard` is never read
	Ok(())
}

fn macro_expansion_gets_no_suggestion(account: &AccountView) -> Result<(), ()> {
	bind_guard!(account);
	//~^ ERROR: account borrow guard `guard` is never read
	Ok(())
}

fn drop_inside_closure_gets_no_suggestion(account: &AccountView) -> Result<(), ()> {
	let guard = account.try_borrow()?;
	//~^ ERROR: account borrow guard `guard` is never read
	let read = || drop(guard);
	read();
	Ok(())
}

fn read_guard(account: &AccountView) -> Result<u8, ()> {
	let guard = account.try_borrow()?;
	Ok(guard.value())
}

fn read_then_drop(account: &AccountView) -> Result<u8, ()> {
	let guard = account.try_borrow()?;
	let value = guard.value();
	drop(guard);
	Ok(value)
}

fn drop_of_read_value(account: &AccountView) -> Result<(), ()> {
	let guard = account.try_borrow()?;
	drop(guard.value());
	Ok(())
}

fn drop_of_constant_is_never_a_guard() {
	drop(UNRELATED_CONSTANT);
}

fn shadowed_drop_is_a_use(account: &AccountView) -> Result<(), ()> {
	let guard = account.try_borrow()?;
	shadowed::drop(guard);
	Ok(())
}

fn guard_returned_through_function_pointer_is_detected(
	loader: &Loader,
	account: &AccountView,
) -> Result<(), ()> {
	let guard = loader.load()(account)?;
	//~^ ERROR: account borrow guard `guard` is never read
	Ok(())
}

fn consuming_guard_in_call_does_not_taint_result(account: &AccountView) -> Result<(), ()> {
	fn consume(_guard: Guard) -> Value {
		Value
	}

	let value = consume(account.try_borrow()?);
	drop(value);
	Ok(())
}

fn consuming_guard_in_method_does_not_taint_result(account: &AccountView) -> Result<(), ()> {
	let value = account.try_borrow()?.into_value();
	drop(value);
	Ok(())
}

fn read_in_nested_block(account: &AccountView) -> Result<u8, ()> {
	let guard = account.try_borrow()?;
	let value = { guard.value() };
	Ok(value)
}

fn read_in_closure(account: &AccountView) -> Result<u8, ()> {
	let guard = account.try_borrow()?;
	let read = || guard.value();
	Ok(read())
}

fn discarded_immediately(mint: &MintView) -> Result<(), ()> {
	mint.as_token_mint_for_program(&0)?;
	Ok(())
}

fn unrelated_as_account_is_ignored(unrelated: &Unrelated) -> Result<(), ()> {
	let guard = unrelated.as_account()?;
	Ok(())
}
