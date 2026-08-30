// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

struct AccountView;
struct Guard;
struct Cpi;
struct Cache;
struct Scheduler;

impl AccountView {
	fn try_borrow_mut(&mut self) -> Result<Guard, ()> {
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

fn process_unrelated_borrow(cache: &mut Cache, cpi: &Cpi) -> Result<(), ()> {
	let _guard = cache.try_borrow_mut()?;
	cpi.invoke()
}

fn process_unrelated_invoke(account: &mut AccountView, scheduler: &Scheduler) -> Result<(), ()> {
	let _guard = account.try_borrow_mut()?;
	scheduler.invoke()
}

fn main() {}
