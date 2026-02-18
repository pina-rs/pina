declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator]
pub enum MultiInstruction {
	Create = 0,
	Update = 1,
}

#[instruction(discriminator = MultiInstruction, variant = Create)]
pub struct CreateInstruction {
	pub bump: u8,
}

#[instruction(discriminator = MultiInstruction, variant = Update)]
pub struct UpdateInstruction {
	pub value: PodU64,
}

#[derive(Accounts, Debug)]
pub struct CreateAccounts<'a> {
	pub authority: &'a AccountView,
	pub state: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct UpdateAccounts<'a> {
	pub authority: &'a AccountView,
	pub state: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for CreateAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = CreateInstruction::try_from_bytes(data)?;
		self.authority.assert_signer()?;
		self.state.assert_writable()?;
		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for UpdateAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = UpdateInstruction::try_from_bytes(data)?;
		self.authority.assert_signer()?;
		self.state.assert_writable()?;
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
		let instruction: MultiInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			MultiInstruction::Create => CreateAccounts::try_from(accounts)?.process(data),
			MultiInstruction::Update => UpdateAccounts::try_from(accounts)?.process(data),
		}
	}
}
