#![allow(dead_code)]

struct AccountView;

impl AccountView {
	fn assert_writable(&self) -> Result<(), ()> {
		Ok(())
	}

	fn resize(&mut self, _new_len: usize) -> Result<(), ()> {
		Ok(())
	}
}

struct ParsedAccounts<'a> {
	account: &'a mut AccountView,
}

impl ParsedAccounts<'_> {
	fn process(self) -> Result<(), ()> {
		self.account.assert_writable()?;
		self.account.resize(1)
	}
}

fn process_instruction(accounts: &mut [AccountView]) -> Result<(), ()> {
	let account = &mut accounts[0];
	account.resize(1)?;
	//~^ ERROR: account resize should be preceded by `assert_writable()` on the same account

	account.assert_writable()?;
	account.resize(2)
}

fn main() {}
