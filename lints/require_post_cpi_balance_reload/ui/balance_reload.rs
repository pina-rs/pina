#![allow(dead_code)]

struct Account;
struct CreateAccount;
struct TransferChecked;

impl Account {
	fn amount(&self) -> u64 {
		0
	}
}

impl TransferChecked {
	fn new(_: &Account, _: &Account, _: &Account, _: &Account, _: u64, _: u8) -> Self {
		Self
	}

	fn invoke(&self) -> Result<(), ()> {
		Ok(())
	}
}

impl CreateAccount {
	fn new(_: &Account, _: &Account, _: &Account, _: &Account, _: u64) -> Self {
		Self
	}

	fn invoke(&self) -> Result<(), ()> {
		Ok(())
	}
}

fn process_non_transfer_constructor(
	first: &Account,
	second: &Account,
	vault: &Account,
	owner: &Account,
) -> Result<(), ()> {
	CreateAccount::new(first, second, vault, owner, 10).invoke()
}

fn process(source: &Account, mint: &Account, vault: &Account, owner: &Account) -> Result<(), ()> {
	let _before = vault.amount();
	TransferChecked::new(source, mint, vault, owner, 10, 0).invoke()?;
	let _after = vault.amount();
	Ok(())
}

fn process_without_reload(
	source: &Account,
	mint: &Account,
	vault: &Account,
	owner: &Account,
) -> Result<(), ()> {
	TransferChecked::new(source, mint, vault, owner, 10, 0).invoke()
	//~^ ERROR: transfer into `vault` is not accounted from its observed balance delta
}

fn main() {}
