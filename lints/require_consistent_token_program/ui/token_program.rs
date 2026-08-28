#![allow(dead_code)]

struct Account;

mod token {
	pub static ID: [u8; 1] = [1];
}

mod token_2022 {
	pub static ID: [u8; 1] = [2];
}

impl Account {
	fn as_token_mint_for_program(&self, _program: &[u8]) -> Result<(), ()> {
		Ok(())
	}

	fn assert_associated_token_address(
		&self,
		_owner: &[u8],
		_mint: &[u8],
		_program: &[u8],
	) -> Result<(), ()> {
		Ok(())
	}
}

fn process(
	mint: &Account,
	vault: &Account,
	owner: &[u8],
	program: &[u8],
	other_program: &[u8],
) -> Result<(), ()> {
	mint.as_token_mint_for_program(program)?;
	vault.assert_associated_token_address(owner, owner, other_program)
	//~^ ERROR: token operation uses `other_program` after the instruction established `program`
}

fn process_mixed_constants(mint: &Account, vault: &Account, owner: &[u8]) -> Result<(), ()> {
	mint.as_token_mint_for_program(&token::ID)?;
	vault.assert_associated_token_address(owner, owner, &token_2022::ID)
	//~^ ERROR: token operation uses `token_2022::ID` after the instruction established `token::ID`
}

fn main() {}
