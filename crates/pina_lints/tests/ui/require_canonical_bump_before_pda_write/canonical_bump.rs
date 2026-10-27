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
	// The real generated helper validates the account's stored field at
	// runtime, so this lint does not need to infer argument provenance.
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
}

fn process_stored_bump_raw_literal(
	account: &Account,
	seeds: &[&[u8]],
	program: &[u8],
) -> Result<(), ()> {
	Vesting::assert_stored_bump(account, 255, seeds, program)
}

struct Ctx<'a> {
	account: &'a Account,
}

/// The account expression itself is dotted, so the field read
/// `ctx.account.as_account()?.bump` must still resolve to it: the base is
/// compared as the complete account identity, not split on the first dot.
fn process_stored_bump_dotted_account(
	ctx: &Ctx<'_>,
	seeds: &[&[u8]],
	program: &[u8],
) -> Result<(), ()> {
	let bump = ctx.account.as_account()?.bump;
	Vesting::assert_stored_bump(ctx.account, bump, seeds, program)
}

/// A rest pattern consumes an element at its own position, so `bump` binds
/// the second element — instruction data. Pairing by subpattern index alone
/// would misattribute the first element to it and approve an
/// attacker-chosen bump.
fn process_stored_bump_rest_pattern(
	account: &Account,
	args: &Args,
	seeds: &[&[u8]],
	program: &[u8],
) -> Result<(), ()> {
	let (.., bump) = (account.as_account()?.bump, args.bump);
	Vesting::assert_stored_bump(account, bump, seeds, program)
}

/// The alias records the binding-time value; the assignment replaces it with
/// instruction data before the call, so the stale alias must not bless it.
#[allow(unused_assignments)]
fn process_stored_bump_reassigned(
	account: &Account,
	args: &Args,
	seeds: &[&[u8]],
	program: &[u8],
) -> Result<(), ()> {
	let mut bump = account.as_account()?.bump;
	bump = args.bump;
	Vesting::assert_stored_bump(account, bump, seeds, program)
}

fn main() {}

// compile-fail
