#![allow(dead_code)]

struct Account;
struct Mint;

fn choose_mint(_loaded: Mint) -> Mint {
	Mint
}

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

fn process_unrelated_policy(
	account: &Account,
	other_mint: &Mint,
	program: &[u8],
) -> Result<(), ()> {
	other_mint.assert_no_extensions()?;
	account.as_token_mint_for_program(program).map(|_| ())
	//~^ ERROR: Token-2022-capable mint loaded without an explicit extension policy
}

fn process_wrapped_load(account: &Account, program: &[u8]) -> Result<(), ()> {
	let selected = choose_mint(account.as_token_mint_for_program(program)?);
	//~^ ERROR: Token-2022-capable mint loaded without an explicit extension policy
	selected.assert_no_extensions()
}

fn main() {}
