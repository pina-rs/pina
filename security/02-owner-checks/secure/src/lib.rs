//! SECURE: Owner check enforced before token deserialization.
//!
//! This program verifies account ownership before deserializing token data.

#![no_std]

#[cfg(all(not(any(target_os = "solana", target_arch = "bpf")), not(test)))]
extern crate std;

use pina::*;

declare_id!("2UfG9UattL4UwPRzKEEj4F1mjoLqoFRbZbPt3dVBHFR2");

#[discriminator]
pub enum PoolInstruction {
	Deposit = 0,
}

#[instruction(discriminator = PoolInstruction, variant = Deposit)]
pub struct DepositInstruction {
	pub amount: u64,
}

#[derive(Accounts, Debug)]
pub struct DepositAccounts<'a> {
	pub depositor: &'a AccountView,
	pub token_account: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for DepositAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = DepositInstruction::try_from_bytes(data)?;

		self.depositor.assert_signer()?;

		// SECURE: The multi-program loader accepts only SPL Token or Token-2022
		// and delegates both ownership and layout validation to that program's
		// concrete upstream state type.
		let token = self
			.token_account
			.as_token_account_for_program(self.token_account.owner())?;
		let balance = token.amount();

		let amount = args.amount.get();

		if balance < amount {
			return Err(ProgramError::InsufficientFunds);
		}

		log!("Deposit accepted (owner verified)");

		Ok(())
	}
}
