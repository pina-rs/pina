//! SECURE: Program address verified before CPI.
//!
//! This program verifies the system program's address before performing
//! a CPI transfer.

#![no_std]

#[cfg(all(not(any(target_os = "solana", target_arch = "bpf")), not(test)))]
extern crate std;

use pina::*;

declare_id!("AgMjyZeNNAp1MXUfLheCbwZzRW15vpcaG19485byeyNm");

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

		// SECURE: Verify the system program address before CPI.
		self.system_program.assert_address(&system::ID)?;

		system::instructions::Transfer {
			from: self.payer,
			to: self.recipient,
			lamports: args.amount.into(),
		}
		.invoke()?;

		Ok(())
	}
}
