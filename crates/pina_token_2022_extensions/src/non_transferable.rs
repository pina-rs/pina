use pinocchio::account_info::AccountInfo;
use pinocchio::cpi::invoke_signed;
use pinocchio::instruction::AccountMeta;
use pinocchio::instruction::Instruction;
use pinocchio::instruction::Signer;
use pinocchio::program_error::ProgramError;
use pinocchio::ProgramResult;

use super::get_extension_from_bytes;

/// State of the non-transferable for mint
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NonTransferable;

impl NonTransferable {
	/// The length of the `NonTransferable` account data.
	pub const LEN: usize = size_of::<NonTransferable>();

	/// Return a `NonTransferable` from the given account info.
	///
	/// This method performs owner and length validation on `AccountInfo`, safe
	/// borrowing the account data.
	#[inline(always)]
	pub fn from_account_info_unchecked(
		account_info: &AccountInfo,
	) -> Result<&NonTransferable, ProgramError> {
		if !account_info.is_owned_by(&crate::ID) {
			return Err(ProgramError::InvalidAccountOwner);
		}

		get_extension_from_bytes(unsafe { account_info.borrow_data_unchecked() })
			.ok_or(ProgramError::InvalidAccountData)
	}
}

impl super::Extension for NonTransferable {
	const BASE_STATE: super::BaseState = super::BaseState::Mint;
	const LEN: usize = Self::LEN;
	const TYPE: super::ExtensionType = super::ExtensionType::NonTransferable;
}

/// State of the non-transferable for token account
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NonTransferableAccount;

impl super::Extension for NonTransferableAccount {
	const BASE_STATE: super::BaseState = super::BaseState::TokenAccount;
	const LEN: usize = Self::LEN;
	const TYPE: super::ExtensionType = super::ExtensionType::NonTransferableAccount;
}

impl NonTransferableAccount {
	/// The length of the `NonTransferableAccount` account data.
	pub const LEN: usize = size_of::<NonTransferableAccount>();

	/// Return a `NonTransferableAccount` from the given account info.
	///
	/// This method performs owner and length validation on `AccountInfo`, safe
	/// borrowing the account data.
	#[inline(always)]
	pub fn from_account_info_unchecked(
		account_info: &AccountInfo,
	) -> Result<&NonTransferableAccount, ProgramError> {
		if !account_info.is_owned_by(&crate::ID) {
			return Err(ProgramError::InvalidAccountOwner);
		}

		get_extension_from_bytes(unsafe { account_info.borrow_data_unchecked() })
			.ok_or(ProgramError::InvalidAccountData)
	}
}

// Instructions
pub struct InitializeNonTransferableMint<'a> {
	/// The mint to initialize the non-transferable
	pub mint: &'a AccountInfo,
}

impl InitializeNonTransferableMint<'_> {
	#[inline(always)]
	pub fn invoke(&self) -> ProgramResult {
		self.invoke_signed(&[])
	}

	#[inline(always)]
	pub fn invoke_signed(&self, signers: &[Signer]) -> ProgramResult {
		let account_metas = [AccountMeta::writable(self.mint.key())];

		// Instruction data Layout:
		//[0] u8: instruction discriminator

		let instruction = Instruction {
			program_id: &crate::ID,
			accounts: &account_metas,
			data: &[32],
		};

		invoke_signed(&instruction, &[self.mint], signers)?;

		Ok(())
	}
}
