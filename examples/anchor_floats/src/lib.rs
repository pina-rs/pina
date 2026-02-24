//! Anchor `floats` parity example ported to pina.
//!
//! Demonstrates float fields in account data plus an authority-gated update.

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

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatError {
	AuthorityMismatch = 0,
}

#[discriminator]
pub enum FloatInstruction {
	Create = 0,
	Update = 1,
}

#[discriminator]
pub enum FloatAccount {
	FloatDataAccount = 1,
}

#[account(discriminator = FloatAccount)]
pub struct FloatDataAccount {
	pub data_f64: PodU64,
	pub data_f32: PodU32,
	pub authority: Address,
}

#[instruction(discriminator = FloatInstruction, variant = Create)]
pub struct CreateInstruction {
	pub data_f32: PodU32,
	pub data_f64: PodU64,
}

#[instruction(discriminator = FloatInstruction, variant = Update)]
pub struct UpdateInstruction {
	pub data_f32: PodU32,
	pub data_f64: PodU64,
}

#[derive(Accounts, Debug)]
pub struct CreateAccounts<'a> {
	pub account: &'a AccountView,
	pub authority: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct UpdateAccounts<'a> {
	pub account: &'a AccountView,
	pub authority: &'a AccountView,
}

fn apply_create(account: &mut FloatDataAccount, authority: &Address, data_f32: f32, data_f64: f64) {
	account.data_f32 = PodU32::from_primitive(data_f32.to_bits());
	account.data_f64 = PodU64::from_primitive(data_f64.to_bits());
	account.authority = *authority;
}

fn apply_update(
	account: &mut FloatDataAccount,
	authority: &Address,
	data_f32: f32,
	data_f64: f64,
) -> ProgramResult {
	if account.authority != *authority {
		return Err(FloatError::AuthorityMismatch.into());
	}

	account.data_f32 = PodU32::from_primitive(data_f32.to_bits());
	account.data_f64 = PodU64::from_primitive(data_f64.to_bits());
	Ok(())
}

impl<'a> ProcessAccountInfos<'a> for CreateAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = CreateInstruction::try_from_bytes(data)?;
		let data_f32 = f32::from_bits(u32::from(args.data_f32));
		let data_f64 = f64::from_bits(u64::from(args.data_f64));

		self.authority.assert_signer()?;
		self.account.assert_empty()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;

		create_account(
			self.authority,
			self.account,
			size_of::<FloatDataAccount>(),
			&ID,
		)?;

		let account = self.account.as_account_mut::<FloatDataAccount>(&ID)?;
		apply_create(account, self.authority.address(), data_f32, data_f64);

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for UpdateAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = UpdateInstruction::try_from_bytes(data)?;
		let data_f32 = f32::from_bits(u32::from(args.data_f32));
		let data_f64 = f64::from_bits(u64::from(args.data_f64));

		self.authority.assert_signer()?;
		self.account
			.assert_writable()?
			.assert_type::<FloatDataAccount>(&ID)?;

		let account = self.account.as_account_mut::<FloatDataAccount>(&ID)?;
		apply_update(account, self.authority.address(), data_f32, data_f64)
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
		let instruction: FloatInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			FloatInstruction::Create => CreateAccounts::try_from(accounts)?.process(data),
			FloatInstruction::Update => UpdateAccounts::try_from(accounts)?.process(data),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn create_instruction_roundtrip() {
		let instruction = CreateInstruction::builder()
			.data_f32(PodU32::from_primitive(1.0f32.to_bits()))
			.data_f64(PodU64::from_primitive(2.0f64.to_bits()))
			.build();
		let bytes = instruction.to_bytes();
		let decoded =
			CreateInstruction::try_from_bytes(bytes).unwrap_or_else(|e| panic!("decode: {e:?}"));

		assert_eq!(f32::from_bits(u32::from(decoded.data_f32)), 1.0);
		assert_eq!(f64::from_bits(u64::from(decoded.data_f64)), 2.0);
	}

	#[test]
	fn update_instruction_roundtrip() {
		let instruction = UpdateInstruction::builder()
			.data_f32(PodU32::from_primitive(3.0f32.to_bits()))
			.data_f64(PodU64::from_primitive(4.0f64.to_bits()))
			.build();
		let bytes = instruction.to_bytes();
		let decoded =
			UpdateInstruction::try_from_bytes(bytes).unwrap_or_else(|e| panic!("decode: {e:?}"));

		assert_eq!(f32::from_bits(u32::from(decoded.data_f32)), 3.0);
		assert_eq!(f64::from_bits(u64::from(decoded.data_f64)), 4.0);
	}

	#[test]
	fn apply_update_rejects_authority_mismatch() {
		let authority: Address = [1u8; 32].into();
		let wrong_authority: Address = [2u8; 32].into();
		let mut account = FloatDataAccount::builder()
			.data_f32(PodU32::from_primitive(1.0f32.to_bits()))
			.data_f64(PodU64::from_primitive(2.0f64.to_bits()))
			.authority(authority)
			.build();

		let result = apply_update(&mut account, &wrong_authority, 3.0, 4.0);
		assert!(matches!(
			result,
			Err(ProgramError::Custom(code)) if code == FloatError::AuthorityMismatch as u32
		));
	}

	#[test]
	fn apply_update_updates_values() {
		let authority: Address = [1u8; 32].into();
		let mut account = FloatDataAccount::builder()
			.data_f32(PodU32::from_primitive(1.0f32.to_bits()))
			.data_f64(PodU64::from_primitive(2.0f64.to_bits()))
			.authority(authority)
			.build();

		let result = apply_update(&mut account, &authority, 3.0, 4.0);
		assert!(result.is_ok());
		assert_eq!(f32::from_bits(u32::from(account.data_f32)), 3.0);
		assert_eq!(f64::from_bits(u64::from(account.data_f64)), 4.0);
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [5u8; 32].into();
		let data = [FloatInstruction::Create as u8];
		let result = parse_instruction::<FloatInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}
}
