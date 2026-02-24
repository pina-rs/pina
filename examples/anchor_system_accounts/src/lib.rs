//! Anchor `system-accounts` parity example ported to pina.
//!
//! Verifies a signer authority plus a wallet account owned by the system
//! program.

#![allow(clippy::inline_always)]
#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[discriminator]
pub enum SystemAccountsInstruction {
	Initialize = 0,
}

#[instruction(discriminator = SystemAccountsInstruction, variant = Initialize)]
pub struct InitializeInstruction {}

#[derive(Accounts, Debug)]
pub struct InitializeAccounts<'a> {
	pub authority: &'a AccountView,
	pub wallet: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = InitializeInstruction::try_from_bytes(data)?;
		self.authority.assert_signer()?;
		self.wallet.assert_owner(&system::ID)?;
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
		let instruction: SystemAccountsInstruction = parse_instruction(program_id, &ID, data)?;
		match instruction {
			SystemAccountsInstruction::Initialize => {
				InitializeAccounts::try_from(accounts)?.process(data)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn validate_system_owner(owner: &Address) -> ProgramResult {
		if owner == &system::ID {
			Ok(())
		} else {
			Err(ProgramError::InvalidAccountOwner)
		}
	}

	#[test]
	fn parse_instruction_accepts_matching_program_id() {
		let data = [SystemAccountsInstruction::Initialize as u8];
		let result = parse_instruction::<SystemAccountsInstruction>(&ID, &ID, &data);
		assert!(matches!(result, Ok(SystemAccountsInstruction::Initialize)));
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [9u8; 32].into();
		let data = [SystemAccountsInstruction::Initialize as u8];
		let result = parse_instruction::<SystemAccountsInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}

	#[test]
	fn validate_system_owner_accepts_system_program() {
		assert!(validate_system_owner(&system::ID).is_ok());
	}

	#[test]
	fn validate_system_owner_rejects_non_system_program() {
		let wrong_owner: Address = [1u8; 32].into();
		assert!(matches!(
			validate_system_owner(&wrong_owner),
			Err(ProgramError::InvalidAccountOwner)
		));
	}
}
