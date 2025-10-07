#![no_std]

use pina::*;

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use pina::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		_program_id: &Pubkey,
		accounts: &[AccountInfo],
		instruction_data: &[u8],
	) -> ProgramResult {
		// Validate program ID
		// if _program_id != &crate::ID {
		Err(ProgramError::IncorrectProgramId)
		// }

		// let (discriminator, data) = instruction_data
		// 	.split_first()
		// 	.ok_or(ProgramError::InvalidInstructionData)?;

		// match Instruction::try_from(discriminator)? {
		// 	Instruction::MakeOffer => {
		// 		log!("Instruction: MakeOffer");
		// 		MakeOffer::try_from((accounts, data))?.handler()
		// 	}
		// 	Instruction::TakeOffer => {
		// 		log!("Instruction: TakeOffer");
		// 		TakeOffer::try_from((accounts, data))?.handler()
		// 	}
		// }
	}
}

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscrowError {
	OfferKeyMismatch = 0,
	TokenAccountMismatch = 1,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq, Eq)]
#[bytemuck(crate = "pina::bytemuck")]
pub struct Offer {
	pub maker: Pubkey,
	pub mint_a: Pubkey,
	pub mint_b: Pubkey,
	pub amount: [u8; 8],
	pub receiver: [u8; 8],
	pub seed: [u8; 8],
	pub bump: u8,
}
