// aux-build: pinocchio_token.rs
// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

extern crate pinocchio_token;

struct Account;
struct CreateAccount;
struct TransferChecked;

mod token_2022 {
	pub(crate) mod instructions {
		pub(crate) use crate::TransferChecked;
	}
}

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

	fn invoke_with_program(&self, _program: &Account) -> Result<(), ()> {
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
	TransferChecked::new(source, mint, vault, owner, 10, 0).invoke_with_program(owner)?;
	let _after = vault.amount();
	Ok(())
}

fn process_legacy_without_reload(
	source: &Account,
	mint: &Account,
	vault: &Account,
	owner: &Account,
) -> Result<(), ()> {
	pinocchio_token::instructions::TransferChecked::new(source, mint, vault, owner, 10, 0).invoke()
}

fn process_static_token_2022_without_reload(
	source: &Account,
	mint: &Account,
	vault: &Account,
	owner: &Account,
) -> Result<(), ()> {
	token_2022::instructions::TransferChecked::new(source, mint, vault, owner, 10, 0).invoke()
	//~^ ERROR: transfer into `vault` is not accounted from its observed balance delta
}

fn process_without_reload(
	source: &Account,
	mint: &Account,
	vault: &Account,
	owner: &Account,
) -> Result<(), ()> {
	TransferChecked::new(source, mint, vault, owner, 10, 0).invoke_with_program(owner)
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
	transfer.invoke_with_program(owner)
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
	transfer.invoke_with_program(owner)?;
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
	selected.invoke()?;
	let _after = vault.amount();
	Ok(())
}

fn main() {}
