declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator]
pub enum DocsInstruction {
	Create = 1,
}

#[discriminator]
pub enum DocsAccount {
	DocumentState = 1,
}

/// Account state used by the docs fixture.
#[account(discriminator = DocsAccount)]
pub struct DocumentState {
	/// Program authority for this document.
	pub authority: Address,
	/// Monotonic revision number.
	pub revision: PodU64,
}

/// Creates a new document account.
#[instruction(discriminator = DocsInstruction, variant = Create)]
pub struct CreateDocumentInstruction {
	/// PDA bump for document derivation.
	pub bump: u8,
}

/// Accounts required to create a document.
#[derive(Accounts, Debug)]
pub struct CreateDocumentAccounts<'a> {
	/// Payer funding the account creation.
	pub payer: &'a AccountView,
	/// Document account to initialize.
	pub document: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for CreateDocumentAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = CreateDocumentInstruction::try_from_bytes(data)?;
		self.payer.assert_signer()?;
		self.document.assert_writable()?;
		Ok(())
	}
}

#[error]
pub enum DocsError {
	/// The document already exists.
	AlreadyExists = 9000,
}

pub mod entrypoint {
	use super::*;

	pub fn process_instruction(
		program_id: &Address,
		accounts: &[AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: DocsInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			DocsInstruction::Create => CreateDocumentAccounts::try_from(accounts)?.process(data),
		}
	}
}
