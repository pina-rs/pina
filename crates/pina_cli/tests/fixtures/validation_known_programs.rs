declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator]
pub enum KnownProgramInstruction {
	ValidatePrograms = 1,
}

#[instruction(discriminator = KnownProgramInstruction, variant = ValidatePrograms)]
pub struct ValidateProgramsInstruction {}

#[derive(Accounts, Debug)]
pub struct ValidateProgramsAccounts<'a> {
	pub system_program: &'a AccountView,
	pub token_program: &'a AccountView,
	pub token_2022_program: &'a AccountView,
	pub associated_token_program: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for ValidateProgramsAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = ValidateProgramsInstruction::try_from_bytes(data)?;
		self.system_program.assert_address(&system::ID)?;
		self.token_program.assert_address(&token::ID)?;
		self.token_2022_program.assert_address(&token_2022::ID)?;
		self
			.associated_token_program
			.assert_address(&associated_token_account::ID)?;
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
				ValidateProgramsAccounts::try_from(accounts)?.process(data)
			}
		}
	}
}
