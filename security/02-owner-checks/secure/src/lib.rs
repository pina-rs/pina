//! SECURE: Owner check enforced before token deserialization.
//!
//! This program verifies account ownership before deserializing token data.

#![no_std]

#[cfg(all(not(any(target_os = "solana", target_arch = "bpf")), not(test)))]
extern crate std;

use pina::*;

declare_id!("2UfG9UattL4UwPRzKEEj4F1mjoLqoFRbZbPt3dVBHFR2");

const SPL_PROGRAM_IDS: [Address; 2] = [token::ID, token_2022::ID];

#[discriminator]
pub enum PoolInstruction {
	Deposit = 0,
}

#[instruction(discriminator = PoolInstruction, variant = Deposit)]
pub struct DepositInstruction {
	pub amount: PodU64,
}

#[derive(Accounts, Debug)]
pub struct DepositAccounts<'a> {
	pub depositor: &'a AccountView,
	pub token_account: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for DepositAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = DepositInstruction::try_from_bytes(data)?;

		self.depositor.assert_signer()?;

		// SECURE: Verify ownership before deserialization.
		self.token_account.assert_owners(&SPL_PROGRAM_IDS)?;
		let token = self.token_account.as_token_account()?;
		let balance = token.amount();

		let amount: u64 = args.amount.into();
		if balance < amount {
			return Err(ProgramError::InsufficientFunds);
		}

		log!("Deposit accepted (owner verified)");

		Ok(())
	}
}
