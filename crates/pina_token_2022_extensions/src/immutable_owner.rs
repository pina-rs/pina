use pinocchio::ProgramResult;
use pinocchio::account_info::AccountInfo;
use pinocchio::cpi::invoke_signed;
use pinocchio::instruction::AccountMeta;
use pinocchio::instruction::Instruction;
use pinocchio::instruction::Signer;
use pinocchio::program_error::ProgramError;

use super::get_extension_from_bytes;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImmutableOwner;

impl super::Extension for ImmutableOwner {
	const BASE_STATE: super::BaseState = super::BaseState::TokenAccount;
	const LEN: usize = Self::LEN;
	const TYPE: super::ExtensionType = super::ExtensionType::ImmutableOwner;
}

impl ImmutableOwner {
	/// The length of the `ImmutableOwner` account data.
	pub const LEN: usize = size_of::<ImmutableOwner>();

	/// Return a `ImmutableOwner` from the given account info.
	///
	/// This method performs owner and length validation on `AccountInfo`, safe
	/// borrowing the account data.
	#[inline(always)]
	pub fn from_account_info_unchecked(
		account_info: &AccountInfo,
	) -> Result<&ImmutableOwner, ProgramError> {
		if !account_info.is_owned_by(&crate::ID) {
			return Err(ProgramError::InvalidAccountOwner);
		}

		get_extension_from_bytes(unsafe { account_info.borrow_data_unchecked() })
			.ok_or(ProgramError::InvalidAccountData)
	}
}

// Instructions
pub struct InitializeImmutableOwner<'a> {
	/// The mint to initialize the non-transferable
	pub mint: &'a AccountInfo,
}

impl InitializeImmutableOwner<'_> {
	#[inline(always)]
	pub fn invoke(&self) -> ProgramResult {
		self.invoke_signed(&[])
	}

	#[inline(always)]
	pub fn invoke_signed(&self, signers: &[Signer]) -> ProgramResult {
		let account_metas = [AccountMeta::writable(self.mint.key())];

		// Instruction data Layout:
		// - [0]: instruction discriminator
		let instruction = Instruction {
			program_id: &crate::ID,
			accounts: &account_metas,
			data: &[22],
		};

		invoke_signed(&instruction, &[self.mint], signers)?;

		Ok(())
	}
}
