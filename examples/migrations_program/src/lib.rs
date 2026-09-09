#![no_std]

use pina::*;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator]
pub enum MigrationInstruction {
	Update = 0,
}

#[discriminator]
pub enum MigrationAccount {
	State = 1,
}

#[account(discriminator = MigrationAccount::State, migrations)]
pub struct State {
	pub authority: Address,
	pub value: u64,
	pub enabled: bool,
}

#[instruction(discriminator = MigrationInstruction::Update, migrations)]
pub struct UpdateInstruction {
	pub value: u64,
	pub memo: u16,
}

#[derive(Accounts)]
pub struct UpdateAccounts<'a> {
	#[pina(validate(signer))]
	pub authority: &'a AccountView,
	pub referrer: Option<&'a AccountView>,
}

impl<'a> ProcessAccountInfos<'a> for UpdateAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		UpdateInstruction::with_current_instruction_data(data, |current| {
			let instruction = UpdateInstruction::try_from_bytes(current)?;
			let _ = (
				self.authority,
				self.referrer,
				instruction.value.get(),
				instruction.memo.get(),
			);
			Ok(())
		})
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
		let instruction: MigrationInstruction = parse_instruction(program_id, &ID, data)?;
		match instruction {
			MigrationInstruction::Update => {
				UpdateAccounts::try_from((program_id, accounts))?.process(data)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn generated_instruction_migration_accepts_version_zero_exactly() {
		let mut old = [0_u8; 10];
		old[0] = MigrationInstruction::Update as u8;
		old[1] = 0;
		old[2..].copy_from_slice(&42_u64.to_le_bytes());
		let mut workspace = [0xaa; 12];

		let current = normalize_instruction_data::<UpdateInstruction>(&old, &mut workspace)
			.unwrap_or_else(|error| panic!("normalize instruction: {error:?}"));
		assert!(current.was_migrated());
		assert_eq!(current.as_bytes()[0..2], [0, 1]);
		assert_eq!(&current.as_bytes()[2..10], &42_u64.to_le_bytes());
		assert_eq!(&current.as_bytes()[10..12], &[0, 0]);
	}

	#[test]
	fn generated_instruction_migration_rejects_trailing_bytes() {
		let mut old = [0_u8; 11];
		old[0] = MigrationInstruction::Update as u8;
		old[1] = 0;
		let mut workspace = [0xaa; 12];

		assert_eq!(
			normalize_instruction_data::<UpdateInstruction>(&old, &mut workspace),
			Err(ProgramError::InvalidInstructionData),
		);
	}

	#[test]
	fn generated_account_migration_preserves_old_fields_and_zeros_new_fields() {
		let authority = Address::new_from_array([9; 32]);
		let mut old = [0_u8; 42];
		old[0] = MigrationAccount::State as u8;
		old[1] = 0;
		old[2..34].copy_from_slice(authority.as_ref());
		old[34..42].copy_from_slice(&42_u64.to_le_bytes());

		let plan = State::plan_migration(&old)
			.unwrap_or_else(|error| panic!("plan account migration: {error:?}"));
		assert_eq!(plan.target_size(), 43);
		let mut destination = [0xaa; 43];
		destination[..old.len()].copy_from_slice(&old);
		State::apply_migration(plan.into_payload(), &mut destination);
		State::validate_migration_destination(&destination)
			.unwrap_or_else(|error| panic!("validate destination: {error:?}"));
		State::write_current_migration_version(&mut destination)
			.unwrap_or_else(|error| panic!("write version: {error:?}"));

		assert_eq!(destination[0..2], [1, 1]);
		assert_eq!(&destination[2..34], authority.as_ref());
		assert_eq!(&destination[34..42], &42_u64.to_le_bytes());
		assert_eq!(destination[42], 0);
	}
}
