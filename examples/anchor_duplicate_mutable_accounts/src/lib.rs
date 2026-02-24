//! Anchor `duplicate-mutable-accounts` parity example ported to pina.
//!
//! Anchor enforces duplicate mutable account constraints in the account parser.
//! In pina this check should be explicit in program logic.

#![allow(clippy::inline_always)]
#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("4D6rvpR7TSPwmFottLGa5gpzMcJ76kN8bimQHV9rogjH");

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateMutableError {
	ConstraintDuplicateMutableAccount = 2040,
}

#[discriminator]
pub enum DuplicateMutableInstruction {
	FailsDuplicateMutable = 0,
	AllowsDuplicateMutable = 1,
	AllowsDuplicateReadonly = 2,
}

#[instruction(discriminator = DuplicateMutableInstruction, variant = FailsDuplicateMutable)]
pub struct FailsDuplicateMutableInstruction {}

#[instruction(discriminator = DuplicateMutableInstruction, variant = AllowsDuplicateMutable)]
pub struct AllowsDuplicateMutableInstruction {}

#[instruction(discriminator = DuplicateMutableInstruction, variant = AllowsDuplicateReadonly)]
pub struct AllowsDuplicateReadonlyInstruction {}

#[derive(Accounts, Debug)]
pub struct DuplicateMutableAccounts<'a> {
	pub account1: &'a AccountView,
	pub account2: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct DuplicateReadonlyAccounts<'a> {
	pub account1: &'a AccountView,
	pub account2: &'a AccountView,
}

fn ensure_distinct(account1: &Address, account2: &Address) -> ProgramResult {
	if account1 == account2 {
		return Err(DuplicateMutableError::ConstraintDuplicateMutableAccount.into());
	}

	Ok(())
}

impl<'a> ProcessAccountInfos<'a> for DuplicateMutableAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = FailsDuplicateMutableInstruction::try_from_bytes(data)?;
		self.account1.assert_writable()?;
		self.account2.assert_writable()?;
		ensure_distinct(self.account1.address(), self.account2.address())
	}
}

impl<'a> ProcessAccountInfos<'a> for DuplicateReadonlyAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = AllowsDuplicateReadonlyInstruction::try_from_bytes(data)?;
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
		let instruction: DuplicateMutableInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			DuplicateMutableInstruction::FailsDuplicateMutable => {
				DuplicateMutableAccounts::try_from(accounts)?.process(data)
			}
			DuplicateMutableInstruction::AllowsDuplicateMutable => {
				let _ = AllowsDuplicateMutableInstruction::try_from_bytes(data)?;
				Ok(())
			}
			DuplicateMutableInstruction::AllowsDuplicateReadonly => {
				DuplicateReadonlyAccounts::try_from(accounts)?.process(data)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn duplicate_mutable_check_rejects_same_account() {
		let a: Address = [3u8; 32].into();
		let result = ensure_distinct(&a, &a);
		assert!(matches!(
			result,
			Err(ProgramError::Custom(code))
				if code == DuplicateMutableError::ConstraintDuplicateMutableAccount as u32
		));
	}

	#[test]
	fn duplicate_mutable_check_accepts_distinct_accounts() {
		let a: Address = [1u8; 32].into();
		let b: Address = [2u8; 32].into();
		assert!(ensure_distinct(&a, &b).is_ok());
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [7u8; 32].into();
		let data = [DuplicateMutableInstruction::FailsDuplicateMutable as u8];
		let result =
			parse_instruction::<DuplicateMutableInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}
}
