#![allow(dead_code)]
// normalize-stderr-test: "\n$" -> ""

struct AccountView;

impl AccountView {
	fn assert_empty(&self) -> Result<(), ()> {
		Ok(())
	}
}

struct State;

struct CreateProgramAccount<'a> {
	account: &'a AccountView,
}

impl CreateProgramAccount<'_> {
	fn invoke<T>(&self) -> Result<(), ()> {
		Ok(())
	}
}

fn process_instruction(insecure: &AccountView, secure: &AccountView) -> Result<(), ()> {
	let create = CreateProgramAccount { account: insecure };
	create.invoke::<State>()?;
	//~^ ERROR: `CreateProgramAccount` invoked without a preceding `assert_empty()` on the target account

	secure.assert_empty()?;
	CreateProgramAccount { account: secure }.invoke::<State>()
}

fn shadowed_builder(insecure: &AccountView, secure: &AccountView) -> Result<(), ()> {
	let create = CreateProgramAccount { account: insecure };
	{
		let create = CreateProgramAccount { account: secure };
		secure.assert_empty()?;
		create.invoke::<State>()?;
	}

	create.invoke::<State>()?;
	//~^ ERROR: `CreateProgramAccount` invoked without a preceding `assert_empty()` on the target account
	Ok(())
}

struct Accounts<'a> {
	account: &'a AccountView,
}

fn same_named_fields(first: Accounts<'_>, second: Accounts<'_>) -> Result<(), ()> {
	first.account.assert_empty()?;
	CreateProgramAccount {
		account: second.account,
	}
	.invoke::<State>()?;
	//~^ ERROR: `CreateProgramAccount` invoked without a preceding `assert_empty()` on the target account
	Ok(())
}

fn reassigned_builder(insecure: &AccountView, secure: &AccountView) -> Result<(), ()> {
	let mut create = CreateProgramAccount { account: insecure };
	insecure.assert_empty()?;
	create = CreateProgramAccount { account: secure };
	create.invoke::<State>()?;
	//~^ ERROR: `CreateProgramAccount` invoked without a preceding `assert_empty()` on the target account
	Ok(())
}

fn main() {}
