// normalize-stderr-test: "\n$" -> ""
// aux-build: solana_account_view.rs

#![allow(dead_code)]

extern crate solana_account_view;

use solana_account_view::AccountView;
use solana_account_view::RefMut as Guard;

struct Cpi;
struct Cache;
struct UnrelatedGuard;
struct Value;
struct Scheduler;
struct State;

impl Cache {
	fn try_borrow_mut(&mut self) -> Result<UnrelatedGuard, ()> {
		Ok(UnrelatedGuard)
	}
}

impl Cpi {
	fn invoke(&self) -> Result<(), ()> {
		Ok(())
	}
}

impl Scheduler {
	fn invoke(&self) -> Result<(), ()> {
		Ok(())
	}
}

impl State {
	fn load_pda_mut(_account: &mut AccountView) -> Result<Guard, ()> {
		Ok(Guard)
	}
}

fn process(account: &mut AccountView, cpi: &Cpi) -> Result<(), ()> {
	let guard = account.try_borrow_mut()?;
	drop(guard);
	cpi.invoke()
}

fn process_borrowed(account: &mut AccountView, cpi: &Cpi) -> Result<(), ()> {
	let _guard = account.try_borrow_mut()?;
	cpi.invoke()
	//~^ ERROR: CPI invoked while a mutable account-data borrow is still alive
}

fn process_return_payload(account: &mut AccountView, cpi: &Cpi) -> Result<(), ()> {
	let _guard = account.try_borrow_mut()?;
	return cpi.invoke();
	//~^ ERROR: CPI invoked while a mutable account-data borrow is still alive
}

fn process_break_payload(account: &mut AccountView, cpi: &Cpi) -> Result<(), ()> {
	let _guard = account.try_borrow_mut()?;
	loop {
		break cpi.invoke();
		//~^ ERROR: CPI invoked while a mutable account-data borrow is still alive
	}
}

fn process_generated_pda_borrow(account: &mut AccountView, cpi: &Cpi) -> Result<(), ()> {
	let _guard = State::load_pda_mut(account)?;
	cpi.invoke()
	//~^ ERROR: CPI invoked while a mutable account-data borrow is still alive
}

fn process_block_scoped(account: &mut AccountView, cpi: &Cpi) -> Result<(), ()> {
	{
		let _guard = account.try_borrow_mut()?;
	}
	cpi.invoke()
}

fn process_unrelated_borrow(cache: &mut Cache, cpi: &Cpi) -> Result<(), ()> {
	let _guard = cache.try_borrow_mut()?;
	cpi.invoke()
}

fn process_unrelated_invoke(account: &mut AccountView, scheduler: &Scheduler) -> Result<(), ()> {
	let _guard = account.try_borrow_mut()?;
	scheduler.invoke()
}

fn load_guard(account: &mut AccountView) -> Result<Guard, ()> {
	account.try_borrow_mut()
}

fn consume_guard(_guard: Guard) -> Value {
	Value
}

fn process_consumed_borrow(account: &mut AccountView, cpi: &Cpi) -> Result<(), ()> {
	let value = consume_guard(account.try_borrow_mut()?);
	let _ = value;
	cpi.invoke()
}

fn process_function_pointer_borrow(account: &mut AccountView, cpi: &Cpi) -> Result<(), ()> {
	let loader: fn(&mut AccountView) -> Result<Guard, ()> = load_guard;
	let _guard = loader(account)?;
	cpi.invoke()
	//~^ ERROR: CPI invoked while a mutable account-data borrow is still alive
}

fn process_destructured_borrow(account: &mut AccountView, cpi: &Cpi) -> Result<(), ()> {
	let (_guard, value) = (account.try_borrow_mut()?, 7);
	let _ = value;
	cpi.invoke()
	//~^ ERROR: CPI invoked while a mutable account-data borrow is still alive
}

fn process_cpi_in_closure(account: &mut AccountView, cpi: &Cpi) -> Result<(), ()> {
	let _guard = account.try_borrow_mut()?;
	let invoke = || cpi.invoke();
	//~^ ERROR: CPI invoked while a mutable account-data borrow is still alive
	invoke()
}

fn process_borrow_and_cpi_in_closure(account: &mut AccountView, cpi: &Cpi) -> Result<(), ()> {
	let mut invoke = || {
		let _guard = account.try_borrow_mut()?;
		cpi.invoke()
		//~^ ERROR: CPI invoked while a mutable account-data borrow is still alive
	};
	invoke()
}

fn process_closure_after_dropping_borrow(account: &mut AccountView, cpi: &Cpi) -> Result<(), ()> {
	let guard = account.try_borrow_mut()?;
	let invoke = || cpi.invoke();
	drop(guard);
	invoke()
}

fn process_closure_after_acquiring_borrow(account: &mut AccountView, cpi: &Cpi) -> Result<(), ()> {
	let invoke = || cpi.invoke();
	//~^ ERROR: CPI invoked while a mutable account-data borrow is still alive
	let _guard = account.try_borrow_mut()?;
	invoke()
}

fn process_block_wrapped_closure(account: &mut AccountView, cpi: &Cpi) -> Result<(), ()> {
	let invoke = || cpi.invoke();
	//~^ ERROR: CPI invoked while a mutable account-data borrow is still alive
	let _guard = account.try_borrow_mut()?;
	({ invoke })()
}

fn process_reassigned_closure(account: &mut AccountView, cpi: &Cpi) -> Result<(), ()> {
	let invoke = || cpi.invoke();
	//~^ ERROR: CPI invoked while a mutable account-data borrow is still alive
	let mut selected = invoke;
	selected()?;
	selected = invoke;
	let _guard = account.try_borrow_mut()?;
	selected()
}

fn process_opaque_reassigned_closure(account: &mut AccountView, cpi: &Cpi) -> Result<(), ()> {
	let invoke = || cpi.invoke();
	//~^ ERROR: CPI invoked while a mutable account-data borrow is still alive
	let mut selected = invoke;
	selected()?;
	selected = core::convert::identity(invoke);
	let _guard = account.try_borrow_mut()?;
	selected()
}

fn traverse_binary_and_assignment(account: &mut AccountView) -> Result<(), ()> {
	let _guard = account.try_borrow_mut()?;
	let mut value = 1;
	value += 1;
	let _combined = value + 1;
	Ok(())
}

fn process_cpi_in_match_guard(account: &mut AccountView, cpi: &Cpi) -> Result<(), ()> {
	let _guard = account.try_borrow_mut()?;
	match () {
		_ if cpi.invoke().is_ok() => Ok(()),
		//~^ ERROR: CPI invoked while a mutable account-data borrow is still alive
		_ => Ok(()),
	}
}

mod shadowed {
	pub fn drop<T>(value: T) {
		core::mem::forget(value);
	}
}

fn process_shadowed_drop(account: &mut AccountView, cpi: &Cpi) -> Result<(), ()> {
	let guard = account.try_borrow_mut()?;
	shadowed::drop(guard);
	cpi.invoke()
	//~^ ERROR: CPI invoked while a mutable account-data borrow is still alive
}

fn main() {}

// compile-fail
