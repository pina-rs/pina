//! Anchor `declare-id` parity example ported to pina.
//!
//! Anchor's test validates that a program-id mismatch is rejected. In pina,
//! that check occurs in `parse_instruction`.

#![allow(clippy::inline_always)]
#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

// Same ID value used by anchor's declare-id test program.
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[discriminator]
pub enum DeclareIdInstruction {
	Initialize = 0,
}

#[instruction(discriminator = DeclareIdInstruction, variant = Initialize)]
pub struct InitializeInstruction {}

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		_accounts: &[AccountView],
		data: &[u8],
	) -> ProgramResult {
		let _ = parse_instruction::<DeclareIdInstruction>(program_id, &ID, data)?;
		let _ = InitializeInstruction::try_from_bytes(data)?;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_instruction_accepts_matching_program_id() {
		let data = [DeclareIdInstruction::Initialize as u8];
		let instruction = parse_instruction::<DeclareIdInstruction>(&ID, &ID, &data);
		assert!(matches!(instruction, Ok(DeclareIdInstruction::Initialize)));
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [7u8; 32].into();
		let data = [DeclareIdInstruction::Initialize as u8];
		let result = parse_instruction::<DeclareIdInstruction>(&wrong_program_id, &ID, &data);

		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}
}
