use bytemuck::Pod;
use solana_program::account_info::AccountInfo;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

use crate::assert;
use crate::msg;
use crate::AccountDeserialize;
use crate::AccountInfoValidation;
use crate::AccountValidation;
use crate::AsAccount;
#[cfg(feature = "spl")]
use crate::AsSplAccount;
use crate::CloseAccount;
use crate::Discriminator;
use crate::LamportTransfer;

impl AccountInfoValidation for AccountInfo<'_> {
	#[track_caller]
	fn assert_signer(&self) -> Result<&Self, ProgramError> {
		if !self.is_signer {
			let caller = std::panic::Location::caller();
			msg!("address: {} is missing a required signature", self.key);
			msg!("{}", caller);

			return Err(ProgramError::MissingRequiredSignature);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_writable(&self) -> Result<&Self, ProgramError> {
		if !self.is_writable {
			let caller = std::panic::Location::caller();
			msg!("address: {} has not been marked as writable", self.key);
			msg!("{}", caller);

			return Err(ProgramError::MissingRequiredSignature);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_executable(&self) -> Result<&Self, ProgramError> {
		if !self.executable {
			let caller = std::panic::Location::caller();
			msg!("address: {} is not executable", self.key);
			msg!("{}", caller);

			return Err(ProgramError::InvalidAccountData);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_empty(&self) -> Result<&Self, ProgramError> {
		if !self.data_is_empty() {
			let caller = std::panic::Location::caller();
			msg!("address: {} is not empty", self.key);
			msg!("{}", caller);

			return Err(ProgramError::AccountAlreadyInitialized);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_not_empty(&self) -> Result<&Self, ProgramError> {
		if self.data_is_empty() {
			let caller = std::panic::Location::caller();
			msg!("address: {} is empty", self.key);
			msg!("{}", caller);

			return Err(ProgramError::InvalidAccountData);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_program(&self, program_id: &Pubkey) -> Result<&Self, ProgramError> {
		self.assert_address(program_id)?.assert_executable()
	}

	fn assert_type<T: Discriminator>(&self, program_id: &Pubkey) -> Result<&Self, ProgramError> {
		self.assert_owner(program_id)?;
		let data = self.try_borrow_data()?;
		let data_len = 8 + std::mem::size_of::<T>();

		if data[0].ne(&T::discriminator()) {
			let caller = std::panic::Location::caller();
			msg!("address: {} has invalid discriminator", self.key);
			msg!("{}", caller);

			return Err(ProgramError::InvalidAccountData);
		}

		if data.len() != data_len {
			let caller = std::panic::Location::caller();
			msg!(
				"address: {} has invalid data length for the account type",
				self.key
			);
			msg!("{}", caller);

			return Err(ProgramError::AccountDataTooSmall);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_sysvar(&self, sysvar_id: &Pubkey) -> Result<&Self, ProgramError> {
		self.assert_owner(&solana_program::sysvar::ID)?
			.assert_address(sysvar_id)
	}

	#[track_caller]
	fn assert_owner(&self, owner: &Pubkey) -> Result<&Self, ProgramError> {
		if self.owner.ne(owner) {
			let caller = std::panic::Location::caller();
			msg!(
				"address: {} has invalid owner: {}, required: {}",
				self.key,
				self.owner,
				owner
			);
			msg!("{}", caller);

			return Err(ProgramError::InvalidAccountOwner);
		}

		Ok(self)
	}

	#[track_caller]
	#[cfg(feature = "spl")]
	fn assert_spl_owner(&self) -> Result<&Self, ProgramError> {
		if spl_token_2022::check_spl_token_program_account(&self.owner).is_err() {
			let caller = std::panic::Location::caller();
			msg!(
				"address: {} is not owned by a supported spl token program: {}",
				self.key,
				self.owner
			);
			msg!("{}", caller);

			return Err(ProgramError::IncorrectProgramId);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_address(&self, address: &Pubkey) -> Result<&Self, ProgramError> {
		if self.key.ne(&address) {
			let caller = std::panic::Location::caller();
			msg!("address: {} is invalid, expected: {}", self.key, address);
			msg!("{}", caller);

			return Err(ProgramError::InvalidAccountData);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_seeds(&self, seeds: &[&[u8]], program_id: &Pubkey) -> Result<&Self, ProgramError> {
		let pda = Pubkey::find_program_address(seeds, program_id).0;

		if pda.ne(self.key) {
			let caller = std::panic::Location::caller();
			msg!("address: {} is invalid, expected pda: {}", self.key, pda);
			msg!("{}", caller);

			return Err(ProgramError::InvalidSeeds);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_seeds_with_bump(
		&self,
		seeds: &[&[u8]],
		program_id: &Pubkey,
	) -> Result<&Self, ProgramError> {
		let pda = match Pubkey::create_program_address(seeds, program_id) {
			Ok(pda) => pda,
			Err(error) => {
				let caller = std::panic::Location::caller();
				msg!(
					"could not create pda for address: {}, with provided seeds",
					self.key
				);
				msg!("{}", caller);

				return Err(error.into());
			}
		};

		if pda.ne(self.key) {
			let caller = std::panic::Location::caller();
			msg!("address: {} is invalid, expected pda: {}", self.key, pda);
			msg!("{}", caller);

			return Err(ProgramError::InvalidSeeds);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_canonical_bump(
		&self,
		seeds: &[&[u8]],
		program_id: &Pubkey,
	) -> Result<u8, ProgramError> {
		let (pda, bump) = Pubkey::find_program_address(seeds, program_id);

		if pda.ne(self.key) {
			let caller = std::panic::Location::caller();
			msg!("address: {} is invalid, expected pda: {}", self.key, pda);
			msg!("{}", caller);

			return Err(ProgramError::InvalidSeeds);
		}

		Ok(bump)
	}

	#[cfg(feature = "spl")]
	#[track_caller]
	fn assert_associated_token_address(
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
			let caller = std::panic::Location::caller();
			msg!(
				"address: {} is invalid, expected associated token address: {}",
				self.key,
				address
			);
			msg!("{}", caller);
			return Err(ProgramError::InvalidSeeds);
		}

		Ok(self)
	}
}

impl AsAccount for AccountInfo<'_> {
	#[track_caller]
	fn as_account<T>(&self, program_id: &Pubkey) -> Result<&T, ProgramError>
	where
		T: AccountDeserialize + Discriminator + Pod,
	{
		self.assert_owner(program_id)?;

		unsafe {
			T::try_from_bytes(std::slice::from_raw_parts(
				self.try_borrow_data()?.as_ptr(),
				8 + std::mem::size_of::<T>(),
			))
		}
	}

	#[track_caller]
	fn as_account_mut<T>(&self, program_id: &Pubkey) -> Result<&mut T, ProgramError>
	where
		T: AccountDeserialize + Discriminator + Pod,
	{
		self.assert_owner(program_id)?;

		unsafe {
			T::try_from_bytes_mut(std::slice::from_raw_parts_mut(
				self.try_borrow_mut_data()?.as_mut_ptr(),
				8 + std::mem::size_of::<T>(),
			))
		}
	}
}

#[cfg(feature = "spl")]
impl AccountValidation for spl_token_2022::pod::PodMint {
	#[track_caller]
	fn assert<F>(&self, condition: F) -> Result<&Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if !condition(self) {
			let caller = std::panic::Location::caller();
			msg!("Mint account data is invalid: {}", caller);
			return Err(ProgramError::InvalidAccountData);
		}
		Ok(self)
	}

	#[track_caller]
	fn assert_err<F, E>(&self, condition: F, err: E) -> Result<&Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
		E: Into<ProgramError> + std::error::Error,
	{
		if !condition(self) {
			let caller = std::panic::Location::caller();
			msg!("Mint account data validation error: {}", err);
			msg!("{}", caller);
			return Err(err.into());
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match assert(condition(self), ProgramError::InvalidAccountData, msg) {
			Err(err) => Err(err.into()),
			Ok(()) => Ok(self),
		}
	}

	#[track_caller]
	fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if !condition(self) {
			let caller = std::panic::Location::caller();
			msg!("Mint account data is invalid: {}", caller);

			return Err(ProgramError::InvalidAccountData);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_mut_err<F, E>(&mut self, condition: F, err: E) -> Result<&mut Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
		E: Into<ProgramError> + std::error::Error,
	{
		if !condition(self) {
			let caller = std::panic::Location::caller();
			msg!("Mint account data validation error: {}", err);
			msg!("{}", caller);

			return Err(err.into());
		}
		Ok(self)
	}

	#[track_caller]
	fn assert_mut_msg<F>(&mut self, condition: F, msg: &str) -> Result<&mut Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match assert(condition(self), ProgramError::InvalidAccountData, msg) {
			Err(err) => Err(err.into()),
			Ok(()) => Ok(self),
		}
	}
}

#[cfg(feature = "spl")]
impl AccountValidation for spl_token_2022::pod::PodAccount {
	#[track_caller]
	fn assert<F>(&self, condition: F) -> Result<&Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if !condition(self) {
			let caller = std::panic::Location::caller();
			msg!("Token account data is invalid: {}", caller);
			return Err(ProgramError::InvalidAccountData);
		}
		Ok(self)
	}

	#[track_caller]
	fn assert_err<F, E>(&self, condition: F, err: E) -> Result<&Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
		E: Into<ProgramError> + std::error::Error,
	{
		if !condition(self) {
			let caller = std::panic::Location::caller();
			msg!("Token account data validation error: {}", err);
			msg!("{}", caller);
			return Err(err.into());
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match assert(condition(self), ProgramError::InvalidAccountData, msg) {
			Err(err) => Err(err.into()),
			Ok(()) => Ok(self),
		}
	}

	#[track_caller]
	fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if !condition(self) {
			let caller = std::panic::Location::caller();
			msg!("Token account data is invalid: {}", caller);

			return Err(ProgramError::InvalidAccountData);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_mut_err<F, E>(&mut self, condition: F, err: E) -> Result<&mut Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
		E: Into<ProgramError> + std::error::Error,
	{
		if !condition(self) {
			let caller = std::panic::Location::caller();
			msg!("Token account data validation error: {}", err);
			msg!("{}", caller);

			return Err(err.into());
		}
		Ok(self)
	}

	#[track_caller]
	fn assert_mut_msg<F>(&mut self, condition: F, msg: &str) -> Result<&mut Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match assert(condition(self), ProgramError::InvalidAccountData, msg) {
			Err(err) => Err(err.into()),
			Ok(()) => Ok(self),
		}
	}
}

#[cfg(feature = "spl")]
impl AsSplAccount for AccountInfo<'_> {
	#[track_caller]
	fn as_token_mint_state<'info>(
		&self,
	) -> Result<
		spl_token_2022::extension::PodStateWithExtensions<'info, spl_token_2022::pod::PodMint>,
		ProgramError,
	> {
		self.assert_spl_owner()?;

		unsafe {
			let data = self.try_borrow_data()?;
			let state = spl_token_2022::extension::PodStateWithExtensions::<
				spl_token_2022::pod::PodMint,
			>::unpack(std::slice::from_raw_parts(data.as_ptr(), data.len()))?;

			Ok(state)
		}
	}

	#[track_caller]
	fn as_token_mint(&self) -> Result<spl_token_2022::pod::PodMint, ProgramError> {
		let state = self.as_token_mint_state()?;

		Ok(*state.base)
	}

	#[track_caller]
	fn as_token_account_state<'info>(
		&self,
	) -> Result<
		spl_token_2022::extension::PodStateWithExtensions<'info, spl_token_2022::pod::PodAccount>,
		ProgramError,
	> {
		self.assert_spl_owner()?;

		unsafe {
			let data = self.try_borrow_data()?;
			let state = spl_token_2022::extension::PodStateWithExtensions::<
				spl_token_2022::pod::PodAccount,
			>::unpack(std::slice::from_raw_parts(data.as_ptr(), data.len()))?;

			Ok(state)
		}
	}

	#[track_caller]
	fn as_token_account(&self) -> Result<spl_token_2022::pod::PodAccount, ProgramError> {
		let state = self.as_token_account_state()?;

		Ok(*state.base)
	}

	#[track_caller]
	fn as_associated_token_account_state<'info>(
		&self,
		owner: &Pubkey,
		mint: &Pubkey,
	) -> Result<
		spl_token_2022::extension::PodStateWithExtensions<'info, spl_token_2022::pod::PodAccount>,
		ProgramError,
	> {
		self.assert_address(
			&spl_associated_token_account::get_associated_token_address_with_program_id(
				owner,
				mint,
				&self.owner,
			),
		)?
		.as_token_account_state()
	}

	#[track_caller]
	fn as_associated_token_account(
		&self,
		owner: &Pubkey,
		mint: &Pubkey,
	) -> Result<spl_token_2022::pod::PodAccount, ProgramError> {
		let state = self.as_associated_token_account_state(owner, mint)?;

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
