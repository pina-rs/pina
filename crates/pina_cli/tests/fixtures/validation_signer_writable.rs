declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator]
pub enum ValidationInstruction {
	Validate = 1,
}

#[instruction(discriminator = ValidationInstruction, variant = Validate)]
pub struct ValidateInstruction {}

#[derive(Accounts, Debug)]
pub struct ValidateAccounts<'a> {
	pub authority: &'a AccountView,
	pub payer: &'a AccountView,
	pub recipient: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for ValidateAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = ValidateInstruction::try_from_bytes(data)?;
		self.authority.assert_signer()?;
		self.payer.assert_signer()?.assert_writable()?;
		self.recipient.assert_writable()?;
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
		let instruction: ValidationInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			ValidationInstruction::Validate => ValidateAccounts::try_from(accounts)?.process(data),
		}
	}
}
