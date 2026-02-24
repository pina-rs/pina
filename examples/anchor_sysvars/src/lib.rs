//! Anchor `sysvars` parity example ported to pina.
//!
//! Verifies that provided clock, rent, and stake-history accounts are the
//! expected sysvar accounts.

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

const CLOCK_SYSVAR_ID: Address = address!("SysvarC1ock11111111111111111111111111111111");
const RENT_SYSVAR_ID: Address = address!("SysvarRent111111111111111111111111111111111");
const STAKE_HISTORY_SYSVAR_ID: Address = address!("SysvarStakeHistory1111111111111111111111111");

#[discriminator]
pub enum SysvarsInstruction {
	Sysvars = 0,
}

#[instruction(discriminator = SysvarsInstruction, variant = Sysvars)]
pub struct SysvarsCheckInstruction {}

#[derive(Accounts, Debug)]
pub struct SysvarsAccounts<'a> {
	pub clock: &'a AccountView,
	pub rent: &'a AccountView,
	pub stake_history: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for SysvarsAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = SysvarsCheckInstruction::try_from_bytes(data)?;

		self.clock.assert_sysvar(&CLOCK_SYSVAR_ID)?;
		self.rent.assert_sysvar(&RENT_SYSVAR_ID)?;
		self.stake_history.assert_sysvar(&STAKE_HISTORY_SYSVAR_ID)?;

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
		let instruction: SysvarsInstruction = parse_instruction(program_id, &ID, data)?;
		match instruction {
			SysvarsInstruction::Sysvars => SysvarsAccounts::try_from(accounts)?.process(data),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_instruction_accepts_matching_program_id() {
		let data = [SysvarsInstruction::Sysvars as u8];
		let result = parse_instruction::<SysvarsInstruction>(&ID, &ID, &data);
		assert!(matches!(result, Ok(SysvarsInstruction::Sysvars)));
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [7u8; 32].into();
		let data = [SysvarsInstruction::Sysvars as u8];
		let result = parse_instruction::<SysvarsInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}

	#[test]
	fn sysvar_constants_are_distinct() {
		assert_ne!(CLOCK_SYSVAR_ID, RENT_SYSVAR_ID);
		assert_ne!(CLOCK_SYSVAR_ID, STAKE_HISTORY_SYSVAR_ID);
		assert_ne!(RENT_SYSVAR_ID, STAKE_HISTORY_SYSVAR_ID);
	}
}
