#![allow(unsafe_code)]

use core::slice::from_raw_parts;
use core::slice::from_raw_parts_mut;

use pinocchio::ProgramResult;
use pinocchio_system::instructions::Transfer;

use crate::AccountDeserialize;
use crate::AccountInfoValidation;
#[cfg(feature = "token")]
use crate::AccountValidation;
use crate::AccountView;
use crate::Address;
use crate::AsAccount;
#[cfg(feature = "token")]
use crate::AsTokenAccount;
use crate::CloseAccountWithRecipient;
use crate::HasDiscriminator;
use crate::LamportTransfer;
use crate::Pod;
use crate::ProgramError;
use crate::log;
use crate::log_caller;

const SYSVAR_ID: Address = crate::address!("Sysvar1111111111111111111111111111111111111");

impl AccountInfoValidation for AccountView {
	#[track_caller]
	fn assert_signer(&self) -> Result<&Self, ProgramError> {
		if !self.is_signer() {
			log!(
				"address: {} is missing a required signature",
				self.address().as_ref()
			);
			log_caller();

			return Err(ProgramError::MissingRequiredSignature);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_writable(&self) -> Result<&Self, ProgramError> {
		if !self.is_writable() {
			log!(
				"address: {} has not been marked as writable",
				self.address().as_ref()
			);
			log_caller();

			return Err(ProgramError::InvalidAccountData);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_executable(&self) -> Result<&Self, ProgramError> {
		if !self.executable() {
			log!("address: {} is not executable", self.address().as_ref());
			log_caller();

			return Err(ProgramError::InvalidAccountData);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_data_len(&self, len: usize) -> Result<&Self, ProgramError> {
		if self.data_len() != len {
			log!(
				"address: {} has an incorrect length",
				self.address().as_ref()
			);
			log_caller();

			return Err(ProgramError::InvalidAccountData);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_empty(&self) -> Result<&Self, ProgramError> {
		if !self.is_data_empty() {
			log!("address: {} is not empty", self.address().as_ref());
			log_caller();

			return Err(ProgramError::AccountAlreadyInitialized);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_not_empty(&self) -> Result<&Self, ProgramError> {
		if self.is_data_empty() {
			log!("address: {} is empty", self.address().as_ref());
			log_caller();

			return Err(ProgramError::UninitializedAccount);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_program(&self, program_id: &Address) -> Result<&Self, ProgramError> {
		self.assert_address(program_id)?.assert_executable()
	}

	fn assert_type<T: HasDiscriminator>(
		&self,
		program_id: &Address,
	) -> Result<&Self, ProgramError> {
		self.assert_owner(program_id)?;
		let data = self.try_borrow()?;

		if !T::matches_discriminator(&data) {
			log!(
				"address: {} has invalid discriminator",
				self.address().as_ref()
			);
			log_caller();

			return Err(ProgramError::InvalidAccountData);
		}

		if data.len() != size_of::<T>() {
			log!(
				"address: {} has invalid data length for the account type",
				self.address().as_ref()
			);
			log_caller();

			return Err(ProgramError::AccountDataTooSmall);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_sysvar(&self, sysvar_id: &Address) -> Result<&Self, ProgramError> {
		self.assert_owner(&SYSVAR_ID)?.assert_address(sysvar_id)
	}

	#[track_caller]
	fn assert_owner(&self, owner: &Address) -> Result<&Self, ProgramError> {
		// SAFETY: `owner()` is unsafe in pinocchio 0.10.x because it reads from
		// raw account memory. The Solana runtime guarantees this memory is valid
		// for the duration of the transaction.
		let account_owner = unsafe { self.owner() };
		if account_owner.ne(owner) {
			log!(
				"address: {} has invalid owner: {}, required: {}",
				self.address().as_ref(),
				account_owner.as_ref(),
				owner.as_ref()
			);
			log_caller();

			return Err(ProgramError::InvalidAccountOwner);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_owners(&self, owners: &[Address]) -> Result<&Self, ProgramError> {
		// SAFETY: see `assert_owner` above.
		let account_owner = unsafe { self.owner() };
		if owners.contains(account_owner) {
			return Ok(self);
		}

		log!(
			"address: {} has invalid owner: {}",
			self.address().as_ref(),
			account_owner.as_ref(),
		);
		log_caller();

		Err(ProgramError::InvalidAccountOwner)
	}

	#[track_caller]
	fn assert_address(&self, addr: &Address) -> Result<&Self, ProgramError> {
		if self.address() == addr {
			return Ok(self);
		}

		log!(
			"address: {} is invalid, expected: {}",
			self.address().as_ref(),
			addr.as_ref()
		);
		log_caller();

		Err(ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_addresses(&self, addresses: &[Address]) -> Result<&Self, ProgramError> {
		if addresses.contains(self.address()) {
			return Ok(self);
		}

		log!("address: {} is invalid", self.address().as_ref());
		log_caller();

		Err(ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_seeds(&self, seeds: &[&[u8]], program_id: &Address) -> Result<&Self, ProgramError> {
		let Some((pda, _bump)) = crate::try_find_program_address(seeds, program_id) else {
			log!(
				"could not find program address from seeds with program id: {}",
				program_id.as_ref()
			);
			log_caller();
			return Err(ProgramError::InvalidSeeds);
		};

		if self.address() == &pda {
			return Ok(self);
		}

		log!(
			"address: {} is invalid, expected pda: {}",
			self.address().as_ref(),
			pda.as_ref()
		);
		log_caller();

		Err(ProgramError::InvalidSeeds)
	}

	#[track_caller]
	fn assert_seeds_with_bump(
		&self,
		seeds: &[&[u8]],
		program_id: &Address,
	) -> Result<&Self, ProgramError> {
		let pda = match crate::create_program_address(seeds, program_id) {
			Ok(pda) => pda,
			Err(_error) => {
				log!(
					"could not create pda for address: {}, with provided seeds and bump",
					self.address().as_ref(),
				);
				log_caller();

				return Err(ProgramError::InvalidSeeds);
			}
		};

		if &pda != self.address() {
			log!(
				"address: {} is invalid, expected pda: {}",
				self.address().as_ref(),
				pda.as_ref()
			);
			log_caller();

			return Err(ProgramError::InvalidSeeds);
		}

		Ok(self)
	}

	#[track_caller]
	fn assert_canonical_bump(
		&self,
		seeds: &[&[u8]],
		program_id: &Address,
	) -> Result<u8, ProgramError> {
		let Some((pda, bump)) = crate::try_find_program_address(seeds, program_id) else {
			log!(
				"could not find program address from seeds with program id: {}",
				program_id.as_ref()
			);
			log_caller();
			return Err(ProgramError::InvalidSeeds);
		};

		if pda.eq(self.address()) {
			return Ok(bump);
		}

		log!(
			"address: {} is invalid, expected pda: {}",
			self.address().as_ref(),
			pda.as_ref()
		);
		log_caller();

		Err(ProgramError::InvalidSeeds)
	}

	#[cfg(feature = "token")]
	#[track_caller]
	fn assert_associated_token_address(
		&self,
		wallet: &Address,
		mint: &Address,
		token_program: &Address,
	) -> Result<&Self, ProgramError> {
		let Some((ata_address, _bump)) =
			crate::try_get_associated_token_address(wallet, mint, token_program)
		else {
			log!(
				"could not find associated token address for wallet: {}, mint: {}",
				wallet.as_ref(),
				mint.as_ref(),
			);
			log_caller();

			return Err(ProgramError::InvalidSeeds);
		};

		if ata_address.eq(self.address()) {
			return Ok(self);
		}

		log!(
			"address: {} is invalid, expected associated token address: {}",
			self.address().as_ref(),
			ata_address.as_ref()
		);
		log_caller();

		Err(ProgramError::InvalidSeeds)
	}
}

impl AsAccount for AccountView {
	#[track_caller]
	fn as_account<T>(&self, program_id: &Address) -> Result<&T, ProgramError>
	where
		T: AccountDeserialize + HasDiscriminator + Pod,
	{
		self.assert_owner(program_id)?;
		self.assert_data_len(size_of::<T>())?;

		// SAFETY: `try_borrow` returns a reference whose lifetime is tied to
		// `self`. We create a raw-parts slice of exactly `size_of::<T>()` bytes
		// from the same pointer. `T::try_from_bytes` then validates the
		// discriminator and performs a bytemuck cast — no uninitialized memory is
		// read.
		unsafe { T::try_from_bytes(from_raw_parts(self.try_borrow()?.as_ptr(), size_of::<T>())) }
	}

	#[track_caller]
	fn as_account_mut<T>(&self, program_id: &Address) -> Result<&mut T, ProgramError>
	where
		T: AccountDeserialize + HasDiscriminator + Pod,
	{
		self.assert_owner(program_id)?;
		self.assert_data_len(size_of::<T>())?;

		// SAFETY: Same reasoning as `as_account` above, but with a mutable
		// borrow. The Solana runtime guarantees exclusive access when
		// `try_borrow_mut` succeeds.
		unsafe {
			T::try_from_bytes_mut(from_raw_parts_mut(
				self.try_borrow_mut()?.as_mut_ptr(),
				size_of::<T>(),
			))
		}
	}
}

/// Implements `AccountValidation` for a token-related type. All four assertion
/// methods follow the same pattern: check the condition, log on failure, and
/// return the appropriate error.
#[cfg(feature = "token")]
macro_rules! impl_account_validation {
	($type:ty, $label:literal) => {
		impl AccountValidation for $type {
			#[track_caller]
			fn assert<F>(&self, condition: F) -> Result<&Self, ProgramError>
			where
				F: Fn(&Self) -> bool,
			{
				if !condition(self) {
					log!($label);
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
				match crate::assert(condition(self), ProgramError::InvalidAccountData, log) {
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
					log!($label);
					log_caller();
					return Err(ProgramError::InvalidAccountData);
				}
				Ok(self)
			}

			#[track_caller]
			fn assert_mut_msg<F>(
				&mut self,
				condition: F,
				log: &str,
			) -> Result<&mut Self, ProgramError>
			where
				F: Fn(&Self) -> bool,
			{
				match crate::assert(condition(self), ProgramError::InvalidAccountData, log) {
					Err(err) => Err(err),
					Ok(()) => Ok(self),
				}
			}
		}
	};
}

#[cfg(feature = "token")]
impl_account_validation!(crate::token::state::Mint, "Mint account data is invalid");
#[cfg(feature = "token")]
impl_account_validation!(
	crate::token_2022::state::Mint,
	"Mint account data is invalid"
);
#[cfg(feature = "token")]
impl_account_validation!(
	crate::token::state::TokenAccount,
	"Token account data is invalid"
);
#[cfg(feature = "token")]
impl_account_validation!(
	crate::token_2022::state::TokenAccount,
	"Token account data is invalid"
);

#[cfg(feature = "token")]
impl AsTokenAccount for AccountView {
	#[track_caller]
	fn as_token_mint(&self) -> Result<&crate::token::state::Mint, ProgramError> {
		self.check_borrow()?;

		// SECURITY: relies on pinocchio's internal layout validation inside
		// `from_account_view_unchecked`. Callers should verify ownership before
		// trusting the result.
		unsafe { crate::token::state::Mint::from_account_view_unchecked(self) }
	}

	#[track_caller]
	fn as_token_mint_checked(&self) -> Result<&crate::token::state::Mint, ProgramError> {
		self.as_token_mint_checked_with_owners(&[crate::token::ID])
	}

	#[track_caller]
	fn as_token_mint_checked_with_owners(
		&self,
		owners: &[Address],
	) -> Result<&crate::token::state::Mint, ProgramError> {
		self.assert_owners(owners)?;
		self.as_token_mint()
	}

	fn as_token_account(&self) -> Result<&crate::token::state::TokenAccount, ProgramError> {
		self.check_borrow()?;

		// SECURITY: see `as_token_mint` above.
		unsafe { crate::token::state::TokenAccount::from_account_view_unchecked(self) }
	}

	#[track_caller]
	fn as_token_account_checked(&self) -> Result<&crate::token::state::TokenAccount, ProgramError> {
		self.as_token_account_checked_with_owners(&[crate::token::ID])
	}

	#[track_caller]
	fn as_token_account_checked_with_owners(
		&self,
		owners: &[Address],
	) -> Result<&crate::token::state::TokenAccount, ProgramError> {
		self.assert_owners(owners)?;
		self.as_token_account()
	}

	fn as_token_2022_mint(&self) -> Result<&crate::token_2022::state::Mint, ProgramError> {
		self.check_borrow()?;

		// SECURITY: see `as_token_mint` above.
		unsafe { crate::token_2022::state::Mint::from_account_view_unchecked(self) }
	}

	#[track_caller]
	fn as_token_2022_mint_checked(&self) -> Result<&crate::token_2022::state::Mint, ProgramError> {
		self.as_token_2022_mint_checked_with_owners(&[crate::token_2022::ID])
	}

	#[track_caller]
	fn as_token_2022_mint_checked_with_owners(
		&self,
		owners: &[Address],
	) -> Result<&crate::token_2022::state::Mint, ProgramError> {
		self.assert_owners(owners)?;
		self.as_token_2022_mint()
	}

	fn as_token_2022_account(
		&self,
	) -> Result<&crate::token_2022::state::TokenAccount, ProgramError> {
		self.check_borrow()?;

		// SECURITY: see `as_token_mint` above.
		unsafe { crate::token_2022::state::TokenAccount::from_account_view_unchecked(self) }
	}

	#[track_caller]
	fn as_token_2022_account_checked(
		&self,
	) -> Result<&crate::token_2022::state::TokenAccount, ProgramError> {
		self.as_token_2022_account_checked_with_owners(&[crate::token_2022::ID])
	}

	#[track_caller]
	fn as_token_2022_account_checked_with_owners(
		&self,
		owners: &[Address],
	) -> Result<&crate::token_2022::state::TokenAccount, ProgramError> {
		self.assert_owners(owners)?;
		self.as_token_2022_account()
	}

	fn as_associated_token_account(
		&self,
		owner: &Address,
		mint: &Address,
		token_program: &Address,
	) -> Result<&crate::token::state::TokenAccount, ProgramError> {
		self.check_borrow()?;

		// SECURITY: see `as_token_mint` above. Additionally, the address is
		// verified against the derived ATA address before the unchecked cast.
		unsafe {
			crate::token::state::TokenAccount::from_account_view_unchecked(
				self.assert_associated_token_address(owner, mint, token_program)?,
			)
		}
	}

	#[track_caller]
	fn as_associated_token_account_checked(
		&self,
		owner: &Address,
		mint: &Address,
		token_program: &Address,
	) -> Result<&crate::token::state::TokenAccount, ProgramError> {
		self.assert_owner(token_program)?;
		self.as_associated_token_account(owner, mint, token_program)
	}
}

fn checked_send_balances(
	current: u64,
	recipient_balance: u64,
	lamports: u64,
) -> Result<(u64, u64), ProgramError> {
	let new_balance = current
		.checked_sub(lamports)
		.ok_or(ProgramError::InsufficientFunds)?;
	let new_recipient_balance = recipient_balance
		.checked_add(lamports)
		.ok_or(ProgramError::ArithmeticOverflow)?;

	Ok((new_balance, new_recipient_balance))
}

fn checked_close_balance(sender_balance: u64, recipient_balance: u64) -> Result<u64, ProgramError> {
	recipient_balance
		.checked_add(sender_balance)
		.ok_or(ProgramError::ArithmeticOverflow)
}

impl<'a> LamportTransfer<'a> for AccountView {
	/// Send the specified lamports to the `recipient` account.
	/// The sender must be a mutable signer for this to be possible.
	#[inline(always)]
	#[track_caller]
	fn send(&'a self, lamports: u64, recipient: &'a AccountView) -> ProgramResult {
		self.assert_writable()?;
		recipient.assert_writable()?;

		if self.address() == recipient.address() {
			log!("Could not send lamports: sender and recipient must differ");
			log_caller();
			return Err(ProgramError::InvalidArgument);
		}

		let current = self.lamports();
		let recipient_balance = recipient.lamports();
		let (new_balance, new_recipient_balance) =
			checked_send_balances(current, recipient_balance, lamports).map_err(|error| {
				match error {
					ProgramError::InsufficientFunds => {
						log!("Could not subtract lamports: insufficient funds");
					}
					ProgramError::ArithmeticOverflow => {
						log!("Could not add lamports: arithmetic overflow");
					}
					_ => {}
				}
				log_caller();
				error
			})?;

		self.set_lamports(new_balance);
		recipient.set_lamports(new_recipient_balance);

		Ok(())
	}

	/// The `from` account must be mutable and a signer for this to be
	/// possible.
	#[inline(always)]
	fn collect(&'a self, lamports: u64, from: &'a AccountView) -> Result<(), ProgramError> {
		Transfer {
			from,
			to: self,
			lamports,
		}
		.invoke()
	}
}

impl<'a> CloseAccountWithRecipient<'a> for AccountView {
	#[track_caller]
	fn close_with_recipient(&'a self, recipient: &'a AccountView) -> ProgramResult {
		self.assert_writable()?;
		recipient.assert_writable()?;

		if self.address() == recipient.address() {
			log!("Could not close account: recipient must differ from account");
			log_caller();
			return Err(ProgramError::InvalidArgument);
		}

		let new_balance =
			checked_close_balance(self.lamports(), recipient.lamports()).inspect_err(|_| {
				log!("Could not close account: lamport overflow");
				log_caller();
			})?;
		recipient.set_lamports(new_balance);
		self.set_lamports(0);
		self.resize(0)?;
		self.close()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn checked_send_balances_rejects_insufficient_funds() {
		let result = checked_send_balances(3, 10, 4);
		assert_eq!(result, Err(ProgramError::InsufficientFunds));
	}

	#[test]
	fn checked_send_balances_rejects_destination_overflow() {
		let result = checked_send_balances(10, u64::MAX, 1);
		assert_eq!(result, Err(ProgramError::ArithmeticOverflow));
	}

	#[test]
	fn checked_send_balances_conserves_lamports() {
		let (sender, recipient) = checked_send_balances(10, 4, 3)
			.unwrap_or_else(|err| panic!("expected valid transfer: {err:?}"));
		assert_eq!(sender + recipient, 14);
	}

	#[test]
	fn checked_close_balance_rejects_overflow() {
		let result = checked_close_balance(1, u64::MAX);
		assert_eq!(result, Err(ProgramError::ArithmeticOverflow));
	}

	#[test]
	fn checked_close_balance_moves_all_lamports() {
		let result = checked_close_balance(7, 2)
			.unwrap_or_else(|err| panic!("expected valid close balance: {err:?}"));
		assert_eq!(result, 9);
	}
}
