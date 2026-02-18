declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator]
pub enum SingleInstructionDiscriminator {
	DoThing = 1,
}

#[instruction(discriminator = SingleInstructionDiscriminator, variant = DoThing)]
pub struct DoThingInstruction {
	pub amount: PodU64,
	pub enabled: PodBool,
}

#[derive(Accounts, Debug)]
pub struct DoThingAccounts<'a> {
	pub authority: &'a AccountView,
	pub target: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for DoThingAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = DoThingInstruction::try_from_bytes(data)?;
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
		let instruction: SingleInstructionDiscriminator = parse_instruction(program_id, &ID, data)?;

		match instruction {
			SingleInstructionDiscriminator::DoThing => {
				DoThingAccounts::try_from(accounts)?.process(data)
			}
		}
	}
}
