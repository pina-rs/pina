#![allow(dead_code)]

struct Account;
struct Guard;
struct Cpi;

impl Account {
	fn try_borrow_mut(&mut self) -> Result<Guard, ()> {
		Ok(Guard)
	}
}

impl Cpi {
	fn invoke(&self) -> Result<(), ()> {
		Ok(())
	}
}

fn process(account: &mut Account, cpi: &Cpi) -> Result<(), ()> {
	let guard = account.try_borrow_mut()?;
	drop(guard);
	cpi.invoke()
}

fn process_borrowed(account: &mut Account, cpi: &Cpi) -> Result<(), ()> {
	let _guard = account.try_borrow_mut()?;
	cpi.invoke()
	//~^ ERROR: CPI invoked while a mutable account-data borrow is still alive
}

fn main() {}
