use bytemuck::Pod;
use solana_program::account_info::AccountInfo;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

use crate::AccountDeserialize;
use crate::AccountInfoValidation;
use crate::AsAccount;
#[cfg(feature = "spl")]
use crate::AsSplAccount;
use crate::CloseAccount;
use crate::Discriminator;
use crate::LamportTransfer;

impl AccountInfoValidation for AccountInfo<'_> {
	fn is_signer(&self) -> Result<&Self, ProgramError> {
		if !self.is_signer {
			crate::msg!("address: {} is missing a required signature", self.key);

			return Err(ProgramError::MissingRequiredSignature);
		}

		Ok(self)
	}

	fn is_writable(&self) -> Result<&Self, ProgramError> {
		if !self.is_writable {
			crate::msg!("address: {} has not been marked as writable", self.key);

			return Err(ProgramError::MissingRequiredSignature);
		}

		Ok(self)
	}

	fn is_executable(&self) -> Result<&Self, ProgramError> {
		if !self.executable {
			crate::msg!("address: {} is not executable", self.key);

			return Err(ProgramError::InvalidAccountData);
		}

		Ok(self)
	}

	fn is_empty(&self) -> Result<&Self, ProgramError> {
		if !self.data_is_empty() {
			crate::msg!("address: {} is not empty", self.key);

			return Err(ProgramError::AccountAlreadyInitialized);
		}

		Ok(self)
	}

	fn is_program(&self, program_id: &Pubkey) -> Result<&Self, ProgramError> {
		self.has_address(program_id)?.is_executable()
	}

	fn is_type<T: Discriminator>(&self, program_id: &Pubkey) -> Result<&Self, ProgramError> {
		self.has_owner(program_id)?;
		let data = self.try_borrow_data()?;
		let data_len = 8 + std::mem::size_of::<T>();

		if data[0].ne(&T::discriminator()) {
			crate::msg!("address: {} has invalid discriminator", self.key);

			return Err(ProgramError::InvalidAccountData);
		}

		if data.len() != data_len {
			crate::msg!(
				"address: {} has invalid data length for the account type",
				self.key
			);

			return Err(ProgramError::AccountDataTooSmall);
		}

		Ok(self)
	}

	fn is_sysvar(&self, sysvar_id: &Pubkey) -> Result<&Self, ProgramError> {
		self.has_owner(&solana_program::sysvar::ID)?
			.has_address(sysvar_id)
	}

	fn has_owner(&self, owner: &Pubkey) -> Result<&Self, ProgramError> {
		if self.owner.ne(owner) {
			crate::msg!(
				"address: {} has invalid owner: {}, required: {}",
				self.key,
				self.owner,
				owner
			);

			return Err(ProgramError::InvalidAccountOwner);
		}

		Ok(self)
	}

	fn has_address(&self, address: &Pubkey) -> Result<&Self, ProgramError> {
		if self.key.ne(&address) {
			crate::msg!("address: {} is invalid, expected: {}", self.key, address);

			return Err(ProgramError::InvalidAccountData);
		}

		Ok(self)
	}

	fn has_seeds(&self, seeds: &[&[u8]], program_id: &Pubkey) -> Result<&Self, ProgramError> {
		let pda = Pubkey::find_program_address(seeds, program_id).0;

		if pda.ne(self.key) {
			crate::msg!("address: {} is invalid, expected pda: {}", self.key, pda);

			return Err(ProgramError::InvalidSeeds);
		}

		Ok(self)
	}

	fn has_seeds_with_bump(
		&self,
		seeds: &[&[u8]],
		program_id: &Pubkey,
	) -> Result<&Self, ProgramError> {
		let pda = match Pubkey::create_program_address(seeds, program_id) {
			Ok(pda) => pda,
			Err(error) => {
				crate::msg!(
					"could not create pda for address: {}, with provided seeds",
					self.key
				);

				return Err(error.into());
			}
		};

		if pda.ne(self.key) {
			crate::msg!("address: {} is invalid, expected pda: {}", self.key, pda);

			return Err(ProgramError::InvalidSeeds);
		}

		Ok(self)
	}

	fn find_canonical_bump(
		&self,
		seeds: &[&[u8]],
		program_id: &Pubkey,
	) -> Result<u8, ProgramError> {
		let (pda, bump) = Pubkey::find_program_address(seeds, program_id);

		if pda.ne(self.key) {
			crate::msg!("address: {} is invalid, expected pda: {}", self.key, pda);

			return Err(ProgramError::InvalidSeeds);
		}

		Ok(bump)
	}

	#[cfg(feature = "spl")]
	fn is_associated_token_address(
		&self,
		wallet: &Pubkey,
		mint: &Pubkey,
	) -> Result<&Self, ProgramError> {
		let address = spl_associated_token_account::get_associated_token_address_with_program_id(
			wallet,
			mint,
			&spl_token_2022::ID,
		);

		if address.ne(self.key) {
			crate::msg!(
				"address: {} is invalid, expected associated token address: {}",
				self.key,
				address
			);

			return Err(ProgramError::InvalidSeeds);
		}

		Ok(self)
	}
}

impl AsAccount for AccountInfo<'_> {
	fn as_account<T>(&self, program_id: &Pubkey) -> Result<&T, ProgramError>
	where
		T: AccountDeserialize + Discriminator + Pod,
	{
		unsafe {
			self.has_owner(program_id)?;
			T::try_from_bytes(std::slice::from_raw_parts(
				self.try_borrow_data()?.as_ptr(),
				8 + std::mem::size_of::<T>(),
			))
		}
	}

	fn as_account_mut<T>(&self, program_id: &Pubkey) -> Result<&mut T, ProgramError>
	where
		T: AccountDeserialize + Discriminator + Pod,
	{
		unsafe {
			self.has_owner(program_id)?;
			T::try_from_bytes_mut(std::slice::from_raw_parts_mut(
				self.try_borrow_mut_data()?.as_mut_ptr(),
				8 + std::mem::size_of::<T>(),
			))
		}
	}
}

#[cfg(feature = "spl")]
impl AsSplAccount for AccountInfo<'_> {
	fn as_token_mint_state<'info>(
		&self,
	) -> Result<
		spl_token_2022::extension::PodStateWithExtensions<'info, spl_token_2022::pod::PodMint>,
		ProgramError,
	> {
		unsafe {
			let data = self.try_borrow_data()?;
			let state = spl_token_2022::extension::PodStateWithExtensions::<
				spl_token_2022::pod::PodMint,
			>::unpack(std::slice::from_raw_parts(data.as_ptr(), data.len()))?;

			Ok(state)
		}
	}

	fn as_token_mint(&self) -> Result<spl_token_2022::pod::PodMint, ProgramError> {
		let state = self.as_token_mint_state()?;

		Ok(*state.base)
	}

	fn as_token_account_state<'info>(
		&self,
	) -> Result<
		spl_token_2022::extension::PodStateWithExtensions<'info, spl_token_2022::pod::PodAccount>,
		ProgramError,
	> {
		unsafe {
			let data = self.try_borrow_data()?;
			let state = spl_token_2022::extension::PodStateWithExtensions::<
				spl_token_2022::pod::PodAccount,
			>::unpack(std::slice::from_raw_parts(data.as_ptr(), data.len()))?;

			Ok(state)
		}
	}

	fn as_token_account(&self) -> Result<spl_token_2022::pod::PodAccount, ProgramError> {
		let state = self.as_token_account_state()?;

		Ok(*state.base)
	}
}

impl<'a, 'info> LamportTransfer<'a, 'info> for AccountInfo<'info> {
	#[inline(always)]
	fn send(&'a self, lamports: u64, to: &'a AccountInfo<'info>) {
		**self.lamports.borrow_mut() -= lamports;
		**to.lamports.borrow_mut() += lamports;
	}

	#[inline(always)]
	fn collect(&'a self, lamports: u64, from: &'a AccountInfo<'info>) -> Result<(), ProgramError> {
		solana_program::program::invoke(
			&solana_program::system_instruction::transfer(from.key, self.key, lamports),
			&[from.clone(), self.clone()],
		)
	}
}

impl<'a, 'info> CloseAccount<'a, 'info> for AccountInfo<'info> {
	fn close(&'a self, to: &'a AccountInfo<'info>) -> Result<(), ProgramError> {
		// Realloc data to zero.
		self.realloc(0, true)?;

		// Return rent lamports.
		self.send(self.lamports(), to);

		Ok(())
	}
}
