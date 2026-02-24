//! Anchor `realloc` parity example ported to pina.
//!
//! This adaptation focuses on the key safety checks from Anchor's realloc
//! tests: maximum permitted growth and duplicate realloc target detection.

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

const MAX_PERMITTED_DATA_INCREASE: usize = 10_240;

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReallocError {
	AccountReallocExceedsLimit = 3016,
	AccountDuplicateReallocs = 3017,
}

#[discriminator]
pub enum ReallocInstruction {
	Realloc = 0,
	Realloc2 = 1,
}

#[instruction(discriminator = ReallocInstruction, variant = Realloc)]
pub struct ReallocIx {
	pub len: PodU16,
}

#[instruction(discriminator = ReallocInstruction, variant = Realloc2)]
pub struct Realloc2Ix {
	pub len: PodU16,
}

#[derive(Accounts, Debug)]
pub struct ReallocAccounts<'a> {
	pub authority: &'a AccountView,
	pub sample: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct Realloc2Accounts<'a> {
	pub authority: &'a AccountView,
	pub sample1: &'a AccountView,
	pub sample2: &'a AccountView,
	pub system_program: &'a AccountView,
}

fn validate_realloc_delta(current_len: usize, new_len: usize) -> ProgramResult {
	if new_len > current_len {
		let delta = new_len - current_len;
		if delta > MAX_PERMITTED_DATA_INCREASE {
			return Err(ReallocError::AccountReallocExceedsLimit.into());
		}
	}

	Ok(())
}

fn validate_distinct_realloc_targets(account1: &Address, account2: &Address) -> ProgramResult {
	if account1 == account2 {
		return Err(ReallocError::AccountDuplicateReallocs.into());
	}

	Ok(())
}

impl<'a> ProcessAccountInfos<'a> for ReallocAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = ReallocIx::try_from_bytes(data)?;
		let target_len = usize::from(u16::from(args.len));

		self.authority.assert_signer()?;
		self.sample.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;

		validate_realloc_delta(self.sample.data_len(), target_len)?;
		self.sample.resize(target_len)
	}
}

impl<'a> ProcessAccountInfos<'a> for Realloc2Accounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = Realloc2Ix::try_from_bytes(data)?;
		let base_len = usize::from(u16::from(args.len));
		let second_target_len = base_len
			.checked_add(10)
			.ok_or(ProgramError::ArithmeticOverflow)?;

		self.authority.assert_signer()?;
		self.sample1.assert_writable()?;
		self.sample2.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;

		validate_distinct_realloc_targets(self.sample1.address(), self.sample2.address())?;
		validate_realloc_delta(self.sample1.data_len(), base_len)?;
		validate_realloc_delta(self.sample2.data_len(), second_target_len)?;

		self.sample1.resize(base_len)?;
		self.sample2.resize(second_target_len)
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
		let instruction: ReallocInstruction = parse_instruction(program_id, &ID, data)?;
		match instruction {
			ReallocInstruction::Realloc => ReallocAccounts::try_from(accounts)?.process(data),
			ReallocInstruction::Realloc2 => Realloc2Accounts::try_from(accounts)?.process(data),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [5u8; 32].into();
		let data = [ReallocInstruction::Realloc as u8];
		let result = parse_instruction::<ReallocInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}

	#[test]
	fn realloc_instruction_roundtrip() {
		let ix = ReallocIx::builder().len(PodU16::from_primitive(5)).build();
		let bytes = ix.to_bytes();
		let parsed = ReallocIx::try_from_bytes(bytes).unwrap_or_else(|e| panic!("decode: {e:?}"));
		assert_eq!(u16::from(parsed.len), 5);
	}

	#[test]
	fn validate_realloc_delta_allows_small_growth() {
		assert!(validate_realloc_delta(100, 200).is_ok());
	}

	#[test]
	fn validate_realloc_delta_rejects_growth_beyond_limit() {
		let result = validate_realloc_delta(100, 100 + MAX_PERMITTED_DATA_INCREASE + 1);
		assert!(matches!(
			result,
			Err(ProgramError::Custom(code)) if code == ReallocError::AccountReallocExceedsLimit as u32
		));
	}

	#[test]
	fn validate_distinct_realloc_targets_rejects_duplicates() {
		let same: Address = [2u8; 32].into();
		let result = validate_distinct_realloc_targets(&same, &same);
		assert!(matches!(
			result,
			Err(ProgramError::Custom(code)) if code == ReallocError::AccountDuplicateReallocs as u32
		));
	}
}
