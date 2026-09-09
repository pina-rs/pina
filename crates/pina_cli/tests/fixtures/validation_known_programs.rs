declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator]
pub enum KnownProgramInstruction {
	ValidatePrograms = 1,
}

#[instruction(discriminator = KnownProgramInstruction, variant = ValidatePrograms)]
pub struct ValidateProgramsInstruction {}

#[derive(Accounts, Debug)]
pub struct ValidateProgramsAccounts<'a> {
	#[pina(validate(program = system::ID))]
	pub system_program: &'a AccountView,
	#[pina(validate(program = token::ID))]
	pub token_program: &'a AccountView,
	#[pina(validate(program = token_2022::ID))]
	pub token_2022_program: &'a AccountView,
	#[pina(validate(program = associated_token_account::ID))]
	pub associated_token_program: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for ValidateProgramsAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = ValidateProgramsInstruction::try_from_bytes(data)?;
		Ok(())
	}
}

pub mod entrypoint {
	use super::*;

	pub fn process_instruction(
		program_id: &Address,
		accounts: &[AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: KnownProgramInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			KnownProgramInstruction::ValidatePrograms => {
				ValidateProgramsAccounts::try_from((program_id, accounts))?.process(data)
			}
		}
	}
}
