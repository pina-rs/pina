// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code, unused_variables)]

struct AccountView;
struct Guard;
struct Unrelated;
struct MintView;
struct State;

impl AccountView {
	fn try_borrow(&self) -> Result<Guard, ()> {
		Ok(Guard)
	}

	fn try_borrow_mut(&mut self) -> Result<Guard, ()> {
		Ok(Guard)
	}
}

impl Guard {
	fn value(&self) -> u8 {
		0
	}
}

impl Unrelated {
	fn as_account(&self) -> Result<Guard, ()> {
		Ok(Guard)
	}
}

trait AsTokenAccount {
	fn as_token_mint_for_program(&self, program: &u8) -> Result<Guard, ()>;
	fn as_associated_token_account_checked(&self, owner: &u8) -> Result<Guard, ()>;
}

impl AsTokenAccount for MintView {
	fn as_token_mint_for_program(&self, _program: &u8) -> Result<Guard, ()> {
		Ok(Guard)
	}

	fn as_associated_token_account_checked(&self, _owner: &u8) -> Result<Guard, ()> {
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
	let guard = mint.as_associated_token_account_checked(&0)?;
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

fn call_through_field_is_not_a_guard_construction(
	loader: &Loader,
	account: &AccountView,
) -> Result<(), ()> {
	let guard = loader.load()(account)?;
	drop(guard);
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
