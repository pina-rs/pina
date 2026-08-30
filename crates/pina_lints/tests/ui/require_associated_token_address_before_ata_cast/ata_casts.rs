// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

struct AtaView;

#[derive(Clone, Copy)]
struct Wallet;

#[derive(Clone, Copy)]
struct Mint;

impl AtaView {
	fn assert_associated_token_address(
		&self,
		_wallet: Wallet,
		_mint: Mint,
		_token_program: (),
	) -> Result<(), ()> {
		Ok(())
	}

	fn as_associated_token_account(
		&self,
		_wallet: Wallet,
		_mint: Mint,
		_token_program: (),
	) -> Result<AtaView, ()> {
		Ok(AtaView)
	}
}

fn process_deposit(
	vault: &AtaView,
	wallet: Wallet,
	mint: Mint,
	token_program: (),
) -> Result<(), ()> {
	vault.assert_associated_token_address(wallet, mint, token_program)?;
	let view = vault.as_associated_token_account(wallet, mint, token_program)?;
	let _ = view;
	Ok(())
}

fn process_unchecked(
	vault: &AtaView,
	wallet: Wallet,
	mint: Mint,
	token_program: (),
) -> Result<(), ()> {
	let view = vault.as_associated_token_account(wallet, mint, token_program)?;
	//~^ ERROR: ATA casts should be preceded by
	let _ = view;
	Ok(())
}

fn process_wrong_receiver(
	vault: &AtaView,
	other: &AtaView,
	wallet: Wallet,
	mint: Mint,
	token_program: (),
) -> Result<(), ()> {
	other.assert_associated_token_address(wallet, mint, token_program)?;
	let view = vault.as_associated_token_account(wallet, mint, token_program)?;
	//~^ ERROR: ATA casts should be preceded by
	let _ = view;
	Ok(())
}

fn process_order(vault: &AtaView, wallet: Wallet, mint: Mint, token_program: ()) -> Result<(), ()> {
	let view = vault.as_associated_token_account(wallet, mint, token_program)?;
	//~^ ERROR: ATA casts should be preceded by
	vault.assert_associated_token_address(wallet, mint, token_program)?;
	let _ = view;
	Ok(())
}

fn main() {}
