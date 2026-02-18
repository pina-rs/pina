declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator]
pub enum PdaInstruction {
	Initialize = 1,
}

#[instruction(discriminator = PdaInstruction, variant = Initialize)]
pub struct InitializeInstruction {
	pub bump: u8,
}

#[derive(Accounts, Debug)]
pub struct InitializeAccounts<'a> {
	pub authority: &'a AccountView,
	pub counter: &'a AccountView,
}

const COUNTER_SEED: &[u8] = b"counter";

#[macro_export]
macro_rules! counter_seeds {
	($authority:expr) => {
		&[COUNTER_SEED, $authority]
	};
	($authority:expr, $bump:expr) => {
		&[COUNTER_SEED, $authority, &[$bump]]
	};
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = InitializeInstruction::try_from_bytes(data)?;
		let seeds_with_bump = counter_seeds!(self.authority.address().as_ref(), args.bump);
		self
			.counter
			.assert_writable()?
			.assert_seeds_with_bump(seeds_with_bump, &ID)?;
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
		let instruction: PdaInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			PdaInstruction::Initialize => InitializeAccounts::try_from(accounts)?.process(data),
		}
	}
}
