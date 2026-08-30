// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

struct MintView;
struct AccountView;

const SPL_PROGRAM_IDS: [(); 1] = [()];

impl MintView {
	fn assert_owners(&self, _programs: &[()]) -> Result<(), ()> {
		Ok(())
	}

	fn as_token_mint(&self) -> Result<MintView, ()> {
		Ok(MintView)
	}

	fn as_token_mint_for_program(&self, _program: &()) -> Result<MintView, ()> {
		Ok(MintView)
	}
}

impl AccountView {
	fn assert_owner(&self, _program: &()) -> Result<(), ()> {
		Ok(())
	}

	fn as_token_account(&self) -> Result<AccountView, ()> {
		Ok(AccountView)
	}

	fn as_token_account_for_program(&self, _program: &()) -> Result<AccountView, ()> {
		Ok(AccountView)
	}
}

fn process_mint(mint: &MintView) -> Result<(), ()> {
	mint.assert_owners(&SPL_PROGRAM_IDS)?;
	mint.as_token_mint()?;
	Ok(())
}

fn process_account(account: &AccountView, other: &AccountView) -> Result<(), ()> {
	other.assert_owner(&SPL_PROGRAM_IDS[0])?;
	account.as_token_account()?;
	//~^ ERROR: calls to `as_token_*()` methods should be preceded by
	Ok(())
}

fn process_order(account: &AccountView) -> Result<(), ()> {
	let cast = account.as_token_account()?;
	account.assert_owner(&SPL_PROGRAM_IDS[0])?;
	// The check after the cast does not satisfy the requirement.
	let _ = cast;
	Ok(())
}

fn process_checked_loaders(mint: &MintView, account: &AccountView, program: ()) -> Result<(), ()> {
	// Loader variants that validate the program are exempt.
	let _ = mint.as_token_mint_for_program(&program)?;
	let _ = account.as_token_account_for_program(&program)?;
	Ok(())
}

fn main() {}
