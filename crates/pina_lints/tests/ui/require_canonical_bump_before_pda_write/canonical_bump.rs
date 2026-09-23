#![allow(dead_code)]

struct Account;

struct CreateProgramAccountWithBump<'a> {
	account: &'a Account,
}

impl CreateProgramAccountWithBump<'_> {
	fn invoke(&self) -> Result<(), ()> {
		let _ = self.account;
		Ok(())
	}
}

impl Account {
	fn assert_canonical_bump(&self, _seeds: &[&[u8]], _program: &[u8]) -> Result<u8, ()> {
		Ok(255)
	}

	fn assert_seeds_with_bump(&self, _seeds: &[&[u8]], _program: &[u8]) -> Result<(), ()> {
		Ok(())
	}
}

fn process(account: &Account, seeds: &[&[u8]], program: &[u8]) -> Result<(), ()> {
	account.assert_canonical_bump(seeds, program)?;
	account.assert_seeds_with_bump(seeds, program)
}

fn process_unchecked(account: &Account, seeds: &[&[u8]], program: &[u8]) -> Result<(), ()> {
	account.assert_seeds_with_bump(seeds, program)
	//~^ ERROR: explicit PDA bump used without first proving the canonical address
}

fn process_checked_builder(account: &Account) -> Result<(), ()> {
	CreateProgramAccountWithBump { account }.invoke()
}

struct State {
	bump: u8,
}

struct Args {
	bump: u8,
}

impl Account {
	fn as_account(&self) -> Result<&State, ()> {
		Ok(&State { bump: 255 })
	}

	fn assert_stored_bump(
		&self,
		_stored_bump: u8,
		_seeds: &[&[u8]],
		_program: &[u8],
	) -> Result<(), ()> {
		Ok(())
	}
}

struct Vesting;

impl Vesting {
	// The generated shape: the bump is captured in the same parse that
	// produced the other fields, then handed back. This must pass.
	fn assert_stored_bump(
		account: &Account,
		stored_bump: u8,
		seeds: &[&[u8]],
		program: &[u8],
	) -> Result<(), ()> {
		account.assert_stored_bump(stored_bump, seeds, program)
	}
}

fn process_stored_bump_from_parse(
	account: &Account,
	seeds: &[&[u8]],
	program: &[u8],
) -> Result<(), ()> {
	let bump = account.as_account()?.bump;
	Vesting::assert_stored_bump(account, bump, seeds, program)
}

fn process_stored_bump_from_alias(
	account: &Account,
	seeds: &[&[u8]],
	program: &[u8],
) -> Result<(), ()> {
	let (bump, _) = {
		let state = account.as_account()?;
		(state.bump, state.bump)
	};
	Vesting::assert_stored_bump(account, bump, seeds, program)
}

fn process_stored_bump_from_instruction_data(
	account: &Account,
	args: &Args,
	seeds: &[&[u8]],
	program: &[u8],
) -> Result<(), ()> {
	Vesting::assert_stored_bump(account, args.bump, seeds, program)
	//~^ ERROR: assert_stored_bump was given a bump that was not parsed from the account
}

fn process_stored_bump_raw_literal(
	account: &Account,
	seeds: &[&[u8]],
	program: &[u8],
) -> Result<(), ()> {
	Vesting::assert_stored_bump(account, 255, seeds, program)
	//~^ ERROR: assert_stored_bump was given a bump that was not parsed from the account
}

fn main() {}

// compile-fail
