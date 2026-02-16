//! INSECURE: Missing program address check before CPI.
//!
//! This program performs a CPI transfer without verifying the system program
//! account's address.

#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("U4eKmMiqFKesVfdvuAigKD9LkaW3ADm6PByJ9aAixzC");

#[discriminator]
pub enum PayInstruction {
	Pay = 0,
}

#[instruction(discriminator = PayInstruction, variant = Pay)]
pub struct PayInstructionData {
	pub amount: PodU64,
}

#[derive(Accounts, Debug)]
pub struct PayAccounts<'a> {
	pub payer: &'a AccountView,
	pub recipient: &'a AccountView,
	pub system_program: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for PayAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = PayInstructionData::try_from_bytes(data)?;

		self.payer.assert_signer()?.assert_writable()?;
		self.recipient.assert_writable()?;

		// BUG: No address check on system_program!
		// An attacker can pass a malicious program that steals the payer's
		// lamports instead of performing a legitimate transfer.

		system::instructions::Transfer {
			from: self.payer,
			to: self.recipient,
			lamports: args.amount.into(),
		}
		.invoke()?;

		Ok(())
	}
}
