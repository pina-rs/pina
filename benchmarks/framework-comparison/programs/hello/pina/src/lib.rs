//! Pina half of the hello comparison: one instruction, one signer check, one
//! static log line.
#![no_std]

use pina::*;

declare_id!("DCF5KBmtQ9ryDC7mQezKLwuJHem6coVUCmKkw37M9J4A");

#[discriminator(entrypoint)]
pub enum HelloInstruction {
	Hello = 0,
}

#[instruction(discriminator = HelloInstruction::Hello)]
pub struct HelloInstructionData {}

#[derive(Accounts, Debug)]
pub struct HelloAccounts<'a> {
	pub user: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for HelloAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = HelloInstructionData::try_from_bytes(data)?;
		self.user.assert_signer()?;
		log!("Hello, Solana!");
		Ok(())
	}
}

// The generated router reads the instruction first, then walks only the
// accounts the routed struct reads.
dispatch_entrypoint!(HelloInstruction);
