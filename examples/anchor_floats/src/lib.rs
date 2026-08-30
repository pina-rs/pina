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
	pub data_f64: u64,
	pub data_f32: u32,
	pub authority: Address,
}

#[instruction(discriminator = FloatInstruction::Create)]
pub struct CreateInstruction {
	pub data_f32: u32,
	pub data_f64: u64,
}

#[instruction(discriminator = FloatInstruction::Update)]
pub struct UpdateInstruction {
	pub data_f32: u32,
	pub data_f64: u64,
}

#[derive(Accounts, Debug)]
pub struct CreateAccounts<'a> {
	pub account: &'a mut AccountView,
	pub authority: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct UpdateAccounts<'a> {
	pub account: &'a mut AccountView,
	pub authority: &'a AccountView,
}

fn apply_create(
	account: &mut FloatDataAccountZc,
	authority: &Address,
	data_f32: f32,
	data_f64: f64,
) {
	account.data_f32.set(data_f32.to_bits());
	account.data_f64.set(data_f64.to_bits());
	account.authority = *authority;
}

fn apply_update(
	account: &mut FloatDataAccountZc,
	authority: &Address,
	data_f32: f32,
	data_f64: f64,
) -> ProgramResult {
	if account.authority != *authority {
		return Err(FloatError::AuthorityMismatch.into());
	}

	account.data_f32.set(data_f32.to_bits());
	account.data_f64.set(data_f64.to_bits());

	Ok(())
}

impl<'a> ProcessAccountInfos<'a> for CreateAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = CreateInstruction::try_from_bytes(data)?;
		let data_f32 = f32::from_bits(args.data_f32.get());
		let data_f64 = f64::from_bits(args.data_f64.get());

		self.authority.assert_signer()?;
		self.account.assert_empty()?;
		self.system_program.assert_address(&system::ID)?;

		CreateAccount {
			from: self.authority,
			to: self.account,
			space: FloatDataAccount::SIZE as u64,
			owner: &ID,
		}
		.invoke()?;

		// A freshly created account holds zeroed storage; write the typed
		// discriminator before taking the validated view.
		{
			let mut storage = self.account.try_borrow_mut()?;
			FloatDataAccount::write_discriminator(&mut storage);
		}

		let mut account = self.account.as_account_mut::<FloatDataAccount>(&ID)?;
		apply_create(&mut account, self.authority.address(), data_f32, data_f64);

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for UpdateAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = UpdateInstruction::try_from_bytes(data)?;
		let data_f32 = f32::from_bits(args.data_f32.get());
		let data_f64 = f64::from_bits(args.data_f64.get());

		self.authority.assert_signer()?;
		self.account.assert_type::<FloatDataAccount>(&ID)?;

		let mut account = self.account.as_account_mut::<FloatDataAccount>(&ID)?;
		apply_update(&mut account, self.authority.address(), data_f32, data_f64)
	}
}

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		accounts: &mut [AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: FloatInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			FloatInstruction::Create => {
				CreateAccounts::try_from((program_id, accounts))?.process(data)
			}
			FloatInstruction::Update => {
				UpdateAccounts::try_from((program_id, accounts))?.process(data)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn create_instruction_roundtrip() {
		let mut bytes = [0u8; CreateInstruction::SIZE];
		let instruction = CreateInstruction::initialize(&mut bytes)
			.unwrap_or_else(|error| panic!("initialize: {error:?}"));
		instruction.data_f32.set(1.0f32.to_bits());
		instruction.data_f64.set(2.0f64.to_bits());
		let decoded =
			CreateInstruction::try_from_bytes(&bytes).unwrap_or_else(|e| panic!("decode: {e:?}"));

		assert_eq!(f32::from_bits(decoded.data_f32.get()), 1.0);
		assert_eq!(f64::from_bits(decoded.data_f64.get()), 2.0);
	}

	#[test]
	fn update_instruction_roundtrip() {
		let mut bytes = [0u8; UpdateInstruction::SIZE];
		let instruction = UpdateInstruction::initialize(&mut bytes)
			.unwrap_or_else(|error| panic!("initialize: {error:?}"));
		instruction.data_f32.set(3.0f32.to_bits());
		instruction.data_f64.set(4.0f64.to_bits());
		let decoded =
			UpdateInstruction::try_from_bytes(&bytes).unwrap_or_else(|e| panic!("decode: {e:?}"));

		assert_eq!(f32::from_bits(decoded.data_f32.get()), 3.0);
		assert_eq!(f64::from_bits(decoded.data_f64.get()), 4.0);
	}

	#[test]
	fn apply_update_rejects_authority_mismatch() {
		let authority: Address = [1u8; 32].into();
		let wrong_authority: Address = [2u8; 32].into();
		let mut bytes = [0u8; FloatDataAccount::SIZE];
		let account = FloatDataAccount::initialize(&mut bytes)
			.unwrap_or_else(|error| panic!("initialize: {error:?}"));
		account.data_f32.set(1.0f32.to_bits());
		account.data_f64.set(2.0f64.to_bits());
		account.authority = authority;

		let result = apply_update(account, &wrong_authority, 3.0, 4.0);
		assert!(matches!(
			result,
			Err(ProgramError::Custom(code)) if code == FloatError::AuthorityMismatch as u32
		));
	}

	#[test]
	fn apply_update_updates_values() {
		let authority: Address = [1u8; 32].into();
		let mut bytes = [0u8; FloatDataAccount::SIZE];
		let account = FloatDataAccount::initialize(&mut bytes)
			.unwrap_or_else(|error| panic!("initialize: {error:?}"));
		account.data_f32.set(1.0f32.to_bits());
		account.data_f64.set(2.0f64.to_bits());
		account.authority = authority;

		let result = apply_update(account, &authority, 3.0, 4.0);
		assert!(result.is_ok());
		assert_eq!(f32::from_bits(account.data_f32.get()), 3.0);
		assert_eq!(f64::from_bits(account.data_f64.get()), 4.0);
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [5u8; 32].into();
		let data = [FloatInstruction::Create as u8];
		let result = parse_instruction::<FloatInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}
}
