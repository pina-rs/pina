#![allow(unsafe_code)]

use core::slice::from_raw_parts;
use core::slice::from_raw_parts_mut;

use pinocchio::ProgramResult;
use pinocchio_system::instructions::Transfer;

use crate::AccountDeserialize;
use crate::AccountInfo;
use crate::AccountInfoValidation;
#[cfg(feature = "token")]
use crate::AccountValidation;
use crate::AsAccount;
#[cfg(feature = "token")]
use crate::AsTokenAccount;
use crate::CloseAccountWithRecipient;
use crate::HasDiscriminator;
use crate::LamportTransfer;
use crate::Pod;
use crate::ProgramError;
use crate::Pubkey;
#[cfg(feature = "token")]
use crate::assert;
use crate::create_program_address;
use crate::log;
use crate::log_caller;
use crate::pubkey;
use crate::try_find_program_address;

const SYSVAR_ID: Pubkey = pubkey!("Sysvar1111111111111111111111111111111111111");

impl AccountInfoValidation for AccountInfo {
	#[track_caller]
	fn assert_signer(&self) -> Result<&Self, ProgramError> {
		if !self.is_signer() {
			log!("address: {} is missing a required signature", self.key());
			log_caller();

			return Err(ProgramError::MissingRequiredSignature);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_writable(&self) -> Result<&Self, ProgramError> {
		if !self.is_writable() {
			log!("address: {} has not been marked as writable", self.key());
			log_caller();

			// TODO: use a more specific error like `InvalidAccountData` or a
			// custom `NotWritable` variant — `MissingRequiredSignature` is
			// misleading for a writability check.
			return Err(ProgramError::MissingRequiredSignature);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_executable(&self) -> Result<&Self, ProgramError> {
		if !self.executable() {
			log!("address: {} is not executable", self.key());
			log_caller();

			return Err(ProgramError::InvalidAccountData);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_data_len(&self, len: usize) -> Result<&Self, ProgramError> {
		if self.data_len() != len {
			log!("address: {} has an incorrect length", self.key());
			log_caller();

			return Err(ProgramError::InvalidAccountData);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_empty(&self) -> Result<&Self, ProgramError> {
		if !self.data_is_empty() {
			log!("address: {} is not empty", self.key());
			log_caller();

			return Err(ProgramError::AccountAlreadyInitialized);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_not_empty(&self) -> Result<&Self, ProgramError> {
		if self.data_is_empty() {
			log!("address: {} is empty", self.key());
			log_caller();

			return Err(ProgramError::UninitializedAccount);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_program(&self, program_id: &Pubkey) -> Result<&Self, ProgramError> {
		self.assert_address(program_id)?.assert_executable()
	}

	fn assert_type<T: HasDiscriminator>(&self, program_id: &Pubkey) -> Result<&Self, ProgramError> {
		self.assert_owner(program_id)?;
		let data = self.try_borrow_data()?;

		if !T::matches_discriminator(&data) {
			log!("address: {} has invalid discriminator", self.key());
			log_caller();

			return Err(ProgramError::InvalidAccountData);
		}

		if data.len() != size_of::<T>() {
			log!(
				"address: {} has invalid data length for the account type",
				self.key()
			);
			log_caller();

			return Err(ProgramError::AccountDataTooSmall);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_sysvar(&self, sysvar_id: &Pubkey) -> Result<&Self, ProgramError> {
		self.assert_owner(&SYSVAR_ID)?.assert_address(sysvar_id)
	}

	#[track_caller]
	fn assert_owner(&self, owner: &Pubkey) -> Result<&Self, ProgramError> {
		if self.owner().ne(owner) {
			log!(
				"address: {} has invalid owner: {}, required: {}",
				self.key(),
				self.owner(),
				owner
			);
			log_caller();

			return Err(ProgramError::InvalidAccountOwner);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_owners(&self, owners: &[Pubkey]) -> Result<&Self, ProgramError> {
		if owners.contains(self.owner()) {
			return Ok(self);
		}

		log!(
			"address: {} has invalid owner: {}",
			self.key(),
			self.owner(),
		);
		log_caller();

		Err(ProgramError::InvalidAccountOwner)
	}

	#[track_caller]
	fn assert_address(&self, address: &Pubkey) -> Result<&Self, ProgramError> {
		if self.key() == address {
			return Ok(self);
		}

		log!("address: {} is invalid, expected: {}", self.key(), address);
		log_caller();

		Err(ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_addresses(&self, addresses: &[Pubkey]) -> Result<&Self, ProgramError> {
		if addresses.contains(self.key()) {
			return Ok(self);
		}

		log!("address: {} is invalid", self.key());
		log_caller();

		Err(ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_seeds(&self, seeds: &[&[u8]], program_id: &Pubkey) -> Result<&Self, ProgramError> {
		let Some((pda, _bump)) = try_find_program_address(seeds, program_id) else {
			log!(
				"could not find program address from seeds: {} with program id: {}",
				seeds,
				program_id
			);
			log_caller();
			return Err(ProgramError::InvalidSeeds);
		};

		if self.key() == &pda {
			return Ok(self);
		}

		log!("address: {} is invalid, expected pda: {}", self.key(), &pda);
		log_caller();

		Err(ProgramError::InvalidSeeds)
	}

	#[track_caller]
	fn assert_seeds_with_bump(
		&self,
		seeds: &[&[u8]],
		program_id: &Pubkey,
	) -> Result<&Self, ProgramError> {
		let pda = match create_program_address(seeds, program_id) {
			Ok(pda) => pda,
			Err(error) => {
				log!(
					"could not create pda for address: {}, with provided seeds and bump",
					self.key(),
				);
				log_caller();

				return Err(error);
			}
		};

		if &pda != self.key() {
			log!("address: {} is invalid, expected pda: {}", self.key(), &pda);
			log_caller();

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
		let Some((pda, bump)) = try_find_program_address(seeds, program_id) else {
			log!(
				"could not find program address from seeds: {} with program id: {}",
				seeds,
				program_id
			);
			log_caller();
			return Err(ProgramError::InvalidSeeds);
		};

		if pda.eq(self.key()) {
			return Ok(bump);
		}

		log!("address: {} is invalid, expected pda: {}", self.key(), &pda);
		log_caller();

		Err(ProgramError::InvalidSeeds)
	}

	#[cfg(feature = "token")]
	#[track_caller]
	fn assert_associated_token_address(
		&self,
		wallet: &Pubkey,
		mint: &Pubkey,
		token_program: &Pubkey,
	) -> Result<&Self, ProgramError> {
		let Some((address, _bump)) =
			crate::try_get_associated_token_address(wallet, mint, token_program)
		else {
			log!(
				"could not find associated token (p-token) address for wallet: {}, mint: {}",
				wallet,
				mint,
			);
			log_caller();

			return Err(ProgramError::InvalidSeeds);
		};

		if address.eq(self.key()) {
			return Ok(self);
		}

		log!(
			"address: {} is invalid, expected associated token address: {}",
			self.key(),
			&address
		);
		log_caller();

		Err(ProgramError::InvalidSeeds)
	}
}

impl AsAccount for AccountInfo {
	#[track_caller]
	fn as_account<T>(&self, program_id: &Pubkey) -> Result<&T, ProgramError>
	where
		T: AccountDeserialize + HasDiscriminator + Pod,
	{
		self.assert_owner(program_id)?;

		// SAFETY: `try_borrow_data` returns a reference whose lifetime is tied to
		// `self`. We create a raw-parts slice of exactly `size_of::<T>()` bytes
		// from the same pointer. `T::try_from_bytes` then validates the
		// discriminator and performs a bytemuck cast — no uninitialized memory is
		// read.
		unsafe {
			T::try_from_bytes(from_raw_parts(
				self.try_borrow_data()?.as_ptr(),
				size_of::<T>(),
			))
		}
	}

	#[track_caller]
	fn as_account_mut<T>(&self, program_id: &Pubkey) -> Result<&mut T, ProgramError>
	where
		T: AccountDeserialize + HasDiscriminator + Pod,
	{
		self.assert_owner(program_id)?;

		// SAFETY: Same reasoning as `as_account` above, but with a mutable
		// borrow. The Solana runtime guarantees exclusive access when
		// `try_borrow_mut_data` succeeds.
		unsafe {
			T::try_from_bytes_mut(from_raw_parts_mut(
				self.try_borrow_mut_data()?.as_mut_ptr(),
				size_of::<T>(),
			))
		}
	}
}

#[cfg(feature = "token")]
impl AccountValidation for crate::token_2022::state::Mint {
	#[track_caller]
	fn assert<F>(&self, condition: F) -> Result<&Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if condition(self) {
			return Ok(self);
		}

		log!("Mint account data is invalid");
		log_caller();

		Err(ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_msg<F>(&self, condition: F, log: &str) -> Result<&Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match assert(condition(self), ProgramError::InvalidAccountData, log) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}

	#[track_caller]
	fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if condition(self) {
			return Ok(self);
		}

		log!("Mint account data is invalid");
		log_caller();

		Err(ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_mut_msg<F>(&mut self, condition: F, log: &str) -> Result<&mut Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match assert(condition(self), ProgramError::InvalidAccountData, log) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}
}

#[cfg(feature = "token")]
impl AccountValidation for crate::token::state::TokenAccount {
	#[track_caller]
	fn assert<F>(&self, condition: F) -> Result<&Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if !condition(self) {
			log!("Token account data is invalid");
			log_caller();
			return Err(ProgramError::InvalidAccountData);
		}
		Ok(self)
	}

	#[track_caller]
	fn assert_msg<F>(&self, condition: F, log: &str) -> Result<&Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match assert(condition(self), ProgramError::InvalidAccountData, log) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}

	#[track_caller]
	fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if !condition(self) {
			log!("Token account data is invalid");
			log_caller();

			return Err(ProgramError::InvalidAccountData);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_mut_msg<F>(&mut self, condition: F, log: &str) -> Result<&mut Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match assert(condition(self), ProgramError::InvalidAccountData, log) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}
}

#[cfg(feature = "token")]
impl AsTokenAccount for AccountInfo {
	#[track_caller]
	fn as_token_mint(&self) -> Result<&crate::token::state::Mint, ProgramError> {
		self.can_borrow_data()?;

		// SECURITY: relies on pinocchio's internal layout validation inside
		// `from_account_info_unchecked`. Callers should verify ownership before
		// trusting the result.
		unsafe { crate::token::state::Mint::from_account_info_unchecked(self) }
	}

	fn as_token_account(&self) -> Result<&crate::token::state::TokenAccount, ProgramError> {
		self.can_borrow_data()?;

		// SECURITY: see `as_token_mint` above.
		unsafe { crate::token::state::TokenAccount::from_account_info_unchecked(self) }
	}

	fn as_token_2022_mint(&self) -> Result<&crate::token_2022::state::Mint, ProgramError> {
		self.can_borrow_data()?;

		// SECURITY: see `as_token_mint` above.
		unsafe { crate::token_2022::state::Mint::from_account_info_unchecked(self) }
	}

	fn as_token_2022_account(
		&self,
	) -> Result<&crate::token_2022::state::TokenAccount, ProgramError> {
		self.can_borrow_data()?;

		// SECURITY: see `as_token_mint` above.
		unsafe { crate::token_2022::state::TokenAccount::from_account_info_unchecked(self) }
	}

	fn as_associated_token_account(
		&self,
		owner: &Pubkey,
		mint: &Pubkey,
		token_program: &Pubkey,
	) -> Result<&crate::token::state::TokenAccount, ProgramError> {
		self.can_borrow_data()?;

		// SECURITY: see `as_token_mint` above. Additionally, the address is
		// verified against the derived ATA address before the unchecked cast.
		unsafe {
			crate::token::state::TokenAccount::from_account_info_unchecked(
				self.assert_associated_token_address(owner, mint, token_program)?,
			)
		}
	}
}

impl<'a> LamportTransfer<'a> for AccountInfo {
	/// Send the specified lamports to the `recipient` account.
	/// The sender must be a mutable signer for this to be possible.
	#[inline(always)]
	#[track_caller]
	fn send(&'a self, lamports: u64, recipient: &'a AccountInfo) -> ProgramResult {
		let mut self_lamports = match self.try_borrow_mut_lamports() {
			Ok(v) => v,
			Err(e) => {
				log!("Could not mutably borrow owned lamports");
				log_caller();
				return Err(e);
			}
		};

		let mut recipient_lamports = match recipient.try_borrow_mut_lamports() {
			Ok(v) => v,
			Err(e) => {
				log!("Could not mutably borrow recipient lamports");
				log_caller();
				return Err(e);
			}
		};

		// SAFETY:
		// The solana runtime will check that no extra lamports are created so this
		// should be safe even if attempting to add more lamports than owned.
		*self_lamports = self_lamports
			.checked_sub(lamports)
			.ok_or(ProgramError::InsufficientFunds)?;
		*recipient_lamports = recipient_lamports
			.checked_add(lamports)
			.ok_or(ProgramError::ArithmeticOverflow)?;

		Ok(())
	}

	/// The `from` account must be mutable and a signer for this to be
	/// possible.
	#[inline(always)]
	fn collect(&'a self, lamports: u64, from: &'a AccountInfo) -> Result<(), ProgramError> {
		Transfer {
			from,
			to: self,
			lamports,
		}
		.invoke()
	}
}

impl<'a> CloseAccountWithRecipient<'a> for AccountInfo {
	fn close_with_recipient(&'a self, recipient: &'a AccountInfo) -> ProgramResult {
		// SECURITY: unchecked addition — overflow is impossible in practice
		// because the total supply of lamports fits in a u64, but an overflow
		// here would be caught by the runtime's lamport balance check at the
		// end of the transaction.
		*recipient.try_borrow_mut_lamports()? += *self.try_borrow_lamports()?;
		self.resize(0)?;
		self.close()
	}
}
