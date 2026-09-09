// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

struct AccountView;
struct Guard;
struct Cpi;
struct Cache;
struct Scheduler;
struct State;

impl AccountView {
	fn try_borrow_mut(&mut self) -> Result<Guard, ()> {
		Ok(Guard)
	}

	fn read(&self) -> usize {
		0
	}
}

trait AsAccount {
	fn as_account_mut(&mut self) -> Result<Guard, ()>;
}

impl AsAccount for AccountView {
	fn as_account_mut(&mut self) -> Result<Guard, ()> {
		Ok(Guard)
	}
}

impl Cache {
	fn try_borrow_mut(&mut self) -> Result<Guard, ()> {
		Ok(Guard)
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

fn helper_expression_shapes(account: &mut AccountView, cpi: &Cpi, flag: bool) -> Result<(), ()> {
	let value = account.read();
	let guard = match account.try_borrow_mut() {
		Ok(guard) => guard,
		Err(()) => return Err(()),
	};
	drop(guard);
	let guard = account.as_account_mut()?;
	drop(guard);
	let mut count = value;
	count = count + usize::from(flag);
	count += 1;
	let _indexed = [count][0];
	if flag {
		let _ = cpi;
	} else {
		let _ = account;
	}
	cpi.invoke()
}

fn main() {}
