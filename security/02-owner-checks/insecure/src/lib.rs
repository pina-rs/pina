//! INSECURE: Missing owner check before token deserialization.
//!
//! This program deserializes a token account without verifying that it is
//! owned by the SPL Token program.

#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("FTgx5MVAztkPs2zYy8w36e5mXN7eceSdxAbJjcyhujk4");

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

		// BUG: No owner check before deserializing as a token account!
		// An attacker can create a fake account with arbitrary token data
		// owned by any program.
		let token = self.token_account.as_token_account()?;
		let balance = token.amount();

		let amount: u64 = args.amount.into();
		if balance < amount {
			return Err(ProgramError::InsufficientFunds);
		}

		log!("Deposit accepted (owner unverified)");

		Ok(())
	}
}
