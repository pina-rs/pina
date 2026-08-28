#![allow(dead_code)]

struct Account;
struct Mint;

impl Account {
	fn as_token_mint_for_program(&self, _program: &[u8]) -> Result<Mint, ()> {
		Ok(Mint)
	}
}

impl Mint {
	fn assert_no_extensions(&self) -> Result<(), ()> {
		Ok(())
	}
}

fn process(account: &Account, program: &[u8]) -> Result<(), ()> {
	let mint = account.as_token_mint_for_program(program)?;
	mint.assert_no_extensions()
}

fn process_without_policy(account: &Account, program: &[u8]) -> Result<(), ()> {
	account.as_token_mint_for_program(program).map(|_| ())
	//~^ ERROR: Token-2022-capable mint loaded without an explicit extension policy
}

fn process_two_mints_with_one_policy(
	first: &Account,
	second: &Account,
	program: &[u8],
) -> Result<(), ()> {
	let mint = first.as_token_mint_for_program(program)?;
	mint.assert_no_extensions()?;
	second.as_token_mint_for_program(program).map(|_| ())
	//~^ ERROR: Token-2022-capable mint loaded without an explicit extension policy
}

fn main() {}
