//! A hand-written accounts parser that declares its slots but no limit can
//! consume any number of accounts, so `ENTRYPOINT_ACCOUNT_CAPACITY` must fall
//! back to the transaction maximum instead of trusting the declared bound.

use pina::*;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator(entrypoint)]
pub enum Instruction {
	Collect = 0,
	Pair = 1,
}

/// Declares one slot, then reads every account after it.
pub struct CollectAccounts<'a> {
	pub first: &'a AccountView,
	pub rest: &'a [AccountView],
}

impl<'a> ParseAccounts<'a> for CollectAccounts<'a> {
	const ACCOUNT_BOUND: usize = 1;

	fn parse_accounts(cursor: &mut AccountsCursor<'a>) -> Result<Self, ProgramError> {
		let first = cursor.next()?;
		let rest = cursor.take_remaining();

		Ok(Self { first, rest })
	}
}

impl<'a> TryFromAccountInfos<'a> for CollectAccounts<'a> {
	fn try_from_account_infos(
		program_id: &Address,
		accounts: &'a mut [AccountView],
	) -> Result<Self, ProgramError> {
		Self::parse_accounts(&mut AccountsCursor::new(*program_id, accounts))
	}
}

impl<'a> TryFrom<(&'a Address, &'a mut [AccountView])> for CollectAccounts<'a> {
	type Error = ProgramError;

	fn try_from(
		(program_id, accounts): (&'a Address, &'a mut [AccountView]),
	) -> Result<Self, Self::Error> {
		Self::try_from_account_infos(program_id, accounts)
	}
}

impl<'a> ProcessAccountInfos<'a> for CollectAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		let _ = (self.first, self.rest);
		Ok(())
	}
}

#[derive(Accounts)]
pub struct PairAccounts<'a> {
	pub first: &'a AccountView,
	pub second: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for PairAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		Ok(())
	}
}

fn main() {
	assert_eq!(
		<CollectAccounts<'static> as ParseAccounts<'static>>::ACCOUNT_LIMIT,
		<CollectAccounts<'static> as ParseAccounts<'static>>::UNBOUNDED,
	);
	// The declared count still folds into `MAX_INSTRUCTION_ACCOUNTS`.
	assert_eq!(Instruction::MAX_INSTRUCTION_ACCOUNTS, 2);
	assert_eq!(
		Instruction::ENTRYPOINT_ACCOUNT_CAPACITY,
		pinocchio::MAX_TX_ACCOUNTS,
	);
}
