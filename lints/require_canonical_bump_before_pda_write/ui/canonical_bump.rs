#![allow(dead_code)]

struct Account;

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

fn main() {}
