use pinocchio::ProgramResult;
use pinocchio::account_info::AccountInfo;
use pinocchio::cpi::invoke_signed;
use pinocchio::instruction::AccountMeta;
use pinocchio::instruction::Instruction;
use pinocchio::instruction::Signer;
use pinocchio::program_error::ProgramError;
use pinocchio::pubkey::Pubkey;

use super::get_extension_from_bytes;
use crate::UNINIT_BYTE;
use crate::write_bytes;

/// State of the metadata pointer
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetadataPointer {
	/// Authority that can set the metadata address
	pub authority: Pubkey,
	/// Account address that holds the metadata
	pub metadata_address: Pubkey,
}

impl super::Extension for MetadataPointer {
	const BASE_STATE: super::BaseState = super::BaseState::Mint;
	const LEN: usize = Self::LEN;
	const TYPE: super::ExtensionType = super::ExtensionType::MetadataPointer;
}

impl MetadataPointer {
	/// The length of the `MetadataPointer` account data.
	pub const LEN: usize = size_of::<MetadataPointer>();

	/// Return a `MetadataPointer` from the given account info.
	///
	/// This method performs owner and length validation on `AccountInfo`, safe
	/// borrowing the account data.
	#[inline(always)]
	pub fn from_account_info_unchecked(
		account_info: &AccountInfo,
	) -> Result<&MetadataPointer, ProgramError> {
		if !account_info.is_owned_by(&crate::ID) {
			return Err(ProgramError::InvalidAccountOwner);
		}

		get_extension_from_bytes(unsafe { account_info.borrow_data_unchecked() })
			.ok_or(ProgramError::InvalidAccountData)
	}
}

// Instructions
pub struct Initialize<'a> {
	/// The mint that this metadata pointer is associated with
	pub mint: &'a AccountInfo,
	/// The public key for the account that can update the metadata address
	pub authority: Option<Pubkey>,
	/// The account address that holds the metadata
	pub metadata_address: Option<Pubkey>,
}

impl Initialize<'_> {
	#[inline(always)]
	pub fn invoke(&self) -> ProgramResult {
		self.invoke_signed(&[])
	}

	pub fn invoke_signed(&self, signers: &[Signer]) -> ProgramResult {
		// Instruction data layout:
		// - [0] u8: instruction discriminator
		// - [1] u8: extension instruction discriminator
		// - [2..34] u8: authority
		// - [34..66] u8: metadata_address
		let mut instruction_data = [UNINIT_BYTE; 66];
		// Set discriminator as u8 at offset [0] & Set extension discriminator as u8 at
		// offset [1]
		write_bytes(&mut instruction_data[0..2], &[39, 0]);
		// Set authority as u8 at offset [2..34]
		if let Some(authority) = self.authority {
			write_bytes(&mut instruction_data[2..34], &authority);
		} else {
			write_bytes(&mut instruction_data[2..34], &Pubkey::default());
		}
		// Set metadata_address as u8 at offset [34..66]
		if let Some(metadata_address) = self.metadata_address {
			write_bytes(&mut instruction_data[34..66], &metadata_address);
		} else {
			write_bytes(&mut instruction_data[34..66], &Pubkey::default());
		}

		let account_metas: [AccountMeta; 1] = [AccountMeta::writable(self.mint.key())];

		let instruction = Instruction {
			program_id: &crate::ID,
			accounts: &account_metas,
			data: unsafe { core::slice::from_raw_parts(instruction_data.as_ptr().cast(), 66) },
		};

		invoke_signed(&instruction, &[self.mint], signers)
	}
}

pub struct Update<'a> {
	/// The mint that this metadata pointer is associated with
	pub mint: &'a AccountInfo,
	/// The metadata pointer authority
	pub authority: &'a AccountInfo,
	/// The new account address that holds the metadata
	pub new_metadata_address: Option<Pubkey>,
}

impl Update<'_> {
	#[inline(always)]
	pub fn invoke(&self) -> ProgramResult {
		self.invoke_signed(&[])
	}

	pub fn invoke_signed(&self, signers: &[Signer]) -> ProgramResult {
		// Instruction data layout:
		// - [0] u8: instruction discriminator
		// - [1] u8: extension instruction discriminator
		// - [2..34] u8: metadata_address
		let mut instruction_data = [UNINIT_BYTE; 34];
		// Set discriminator as u8 at offset [0] & Set extension discriminator as u8 at
		// offset [1]
		write_bytes(&mut instruction_data[0..2], &[39, 1]);
		// Set metadata_address as u8 at offset [2..34]
		if let Some(new_metadata_address) = self.new_metadata_address {
			write_bytes(&mut instruction_data[2..34], &new_metadata_address);
		} else {
			write_bytes(&mut instruction_data[2..34], &Pubkey::default());
		}

		let account_metas: [AccountMeta; 2] = [
			AccountMeta::writable(self.mint.key()),
			AccountMeta::readonly_signer(self.authority.key()),
		];

		let instruction = Instruction {
			program_id: &crate::ID,
			accounts: &account_metas,
			data: unsafe { core::slice::from_raw_parts(instruction_data.as_ptr().cast(), 34) },
		};

		invoke_signed(&instruction, &[self.mint, self.authority], signers)
	}
}
