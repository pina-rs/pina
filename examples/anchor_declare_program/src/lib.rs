//! Anchor `declare-program` parity example ported to pina.
//!
//! This adaptation focuses on the core behavior that maps to pina directly:
//! validating that an "external program" account matches an expected program
//! ID before executing cross-program logic.

#![allow(clippy::inline_always)]
#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("Dec1areProgram11111111111111111111111111111");

pub mod external {
	use pina::*;

	declare_id!("Externa111111111111111111111111111111111111");
}

#[discriminator]
pub enum DeclareProgramInstruction {
	ValidateExternalProgram = 0,
}

#[instruction(discriminator = DeclareProgramInstruction, variant = ValidateExternalProgram)]
pub struct ValidateExternalProgramInstruction {}

#[derive(Accounts, Debug)]
pub struct ValidateExternalProgramAccounts<'a> {
	pub authority: &'a AccountView,
	pub external_program: &'a AccountView,
}

#[inline]
fn assert_external_program_id(external_program_id: &Address) -> ProgramResult {
	if external_program_id != &external::ID {
		return Err(ProgramError::IncorrectProgramId);
	}

	Ok(())
}

impl<'a> ProcessAccountInfos<'a> for ValidateExternalProgramAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = ValidateExternalProgramInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		assert_external_program_id(self.external_program.address())?;
		self.external_program
			.assert_address(&external::ID)?
			.assert_executable()?;

		Ok(())
	}
}

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		accounts: &[AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: DeclareProgramInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			DeclareProgramInstruction::ValidateExternalProgram => {
				ValidateExternalProgramAccounts::try_from(accounts)?.process(data)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_instruction_accepts_matching_program_id() {
		let data = [DeclareProgramInstruction::ValidateExternalProgram as u8];
		let instruction = parse_instruction::<DeclareProgramInstruction>(&ID, &ID, &data);
		assert!(matches!(
			instruction,
			Ok(DeclareProgramInstruction::ValidateExternalProgram)
		));
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [9u8; 32].into();
		let data = [DeclareProgramInstruction::ValidateExternalProgram as u8];
		let result = parse_instruction::<DeclareProgramInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}

	#[test]
	fn external_program_id_check_rejects_wrong_address() {
		let wrong: Address = [4u8; 32].into();
		let result = assert_external_program_id(&wrong);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}

	#[test]
	fn external_program_id_check_accepts_expected_address() {
		let result = assert_external_program_id(&external::ID);
		assert!(result.is_ok());
	}
}
