//! INSECURE: Missing owner check before Token-2022 byte parsing.
//!
//! This program parses token-account bytes without verifying that the runtime
//! account is owned by Token-2022.

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

		// BUG: This parser validates only the byte layout. It has no AccountView
		// and therefore cannot validate the runtime account owner.
		let data = self.token_account.try_borrow()?;
		let token =
			token_2022::state::StateWithExtensions::<token_2022::state::TokenAccount>::from_bytes(
				&data,
			)?;
		let balance = token.base.amount();

		let amount = args.amount.get();

		if balance < amount {
			return Err(ProgramError::InsufficientFunds);
		}

		log!("Deposit accepted (owner unverified)");

		Ok(())
	}
}
