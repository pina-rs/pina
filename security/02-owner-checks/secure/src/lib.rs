//! SECURE: Owner check enforced while loading Token-2022 data.
//!
//! This program keeps ownership and byte-layout validation in one loader.

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

		// SECURE: The checked upstream account-view parser verifies Token-2022
		// ownership and layout while it creates the guard-backed view.
		let token = self.token_account.as_token_2022_account()?;
		let balance = token.base.amount();

		let amount = args.amount.get();

		if balance < amount {
			return Err(ProgramError::InsufficientFunds);
		}

		log!("Deposit accepted (owner verified)");

		Ok(())
	}
}
