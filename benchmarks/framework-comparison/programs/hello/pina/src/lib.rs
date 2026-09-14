//! Pina half of the hello comparison: one instruction, one signer check, one
//! static log line.
#![no_std]

use pina::*;

declare_id!("DCF5KBmtQ9ryDC7mQezKLwuJHem6coVUCmKkw37M9J4A");

#[discriminator]
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

nostd_entrypoint!(process_instruction);

#[inline(always)]
pub fn process_instruction(
	program_id: &Address,
	accounts: &mut [AccountView],
	data: &[u8],
) -> ProgramResult {
	let instruction: HelloInstruction = parse_instruction(program_id, &ID, data)?;

	match instruction {
		HelloInstruction::Hello => HelloAccounts::try_from((program_id, accounts))?.process(data),
	}
}
