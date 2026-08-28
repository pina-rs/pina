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

fn choose_transfer(_transfer: TransferChecked) -> TransferChecked {
	TransferChecked
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

fn process_unrelated_cpi_before_transfer(
	source: &Account,
	mint: &Account,
	vault: &Account,
	owner: &Account,
) -> Result<(), ()> {
	let transfer = TransferChecked::new(source, mint, vault, owner, 10, 0);
	let _before = vault.amount();
	CreateAccount::new(source, mint, vault, owner, 10).invoke()?;
	transfer.invoke()
	//~^ ERROR: transfer into `vault` is not accounted from its observed balance delta
}

fn process_unrelated_cpi_before_reload(
	source: &Account,
	mint: &Account,
	vault: &Account,
	owner: &Account,
) -> Result<(), ()> {
	let transfer = TransferChecked::new(source, mint, vault, owner, 10, 0);
	let _before = vault.amount();
	transfer.invoke()?;
	//~^ ERROR: transfer into `vault` is not accounted from its observed balance delta
	CreateAccount::new(source, mint, vault, owner, 10).invoke()?;
	let _after = vault.amount();
	Ok(())
}

fn process_wrapped_transfer(
	source: &Account,
	mint: &Account,
	vault: &Account,
	owner: &Account,
) -> Result<(), ()> {
	let _before = vault.amount();
	let selected = choose_transfer(TransferChecked::new(source, mint, vault, owner, 10, 0));
	//~^ ERROR: transfer into `vault` is not accounted from its observed balance delta
	selected.invoke()?;
	let _after = vault.amount();
	Ok(())
}

fn main() {}
