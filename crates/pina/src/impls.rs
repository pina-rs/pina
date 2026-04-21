#![allow(unsafe_code)]

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
use crate::Ref;
use crate::RefMut;
use crate::log;
use crate::log_caller;

const SYSVAR_ID: Address = crate::address!("Sysvar1111111111111111111111111111111111111");

#[track_caller]
fn validate_signer(account: &AccountView) -> ProgramResult {
	if !account.is_signer() {
		log!(
			"address: {} is missing a required signature",
			account.address().as_ref()
		);
		log_caller();

		return Err(ProgramError::MissingRequiredSignature);
	}

	Ok(())
}

#[track_caller]
fn validate_writable(account: &AccountView) -> ProgramResult {
	if !account.is_writable() {
		log!(
			"address: {} has not been marked as writable",
			account.address().as_ref()
		);
		log_caller();

		return Err(ProgramError::InvalidAccountData);
	}

	Ok(())
}

#[track_caller]
fn validate_executable(account: &AccountView) -> ProgramResult {
	if !account.executable() {
		log!("address: {} is not executable", account.address().as_ref());
		log_caller();

		return Err(ProgramError::InvalidAccountData);
	}

	Ok(())
}

#[track_caller]
fn validate_data_len(account: &AccountView, len: usize) -> ProgramResult {
	if account.data_len() != len {
		log!(
			"address: {} has an incorrect length",
			account.address().as_ref()
		);
		log_caller();

		return Err(ProgramError::InvalidAccountData);
	}

	Ok(())
}

#[track_caller]
fn validate_empty(account: &AccountView) -> ProgramResult {
	if !account.is_data_empty() {
		log!("address: {} is not empty", account.address().as_ref());
		log_caller();

		return Err(ProgramError::AccountAlreadyInitialized);
	}

	Ok(())
}

#[track_caller]
fn validate_not_empty(account: &AccountView) -> ProgramResult {
	if account.is_data_empty() {
		log!("address: {} is empty", account.address().as_ref());
		log_caller();

		return Err(ProgramError::UninitializedAccount);
	}

	Ok(())
}

#[track_caller]
fn validate_program(account: &AccountView, program_id: &Address) -> ProgramResult {
	validate_address(account, program_id)?;
	validate_executable(account)
}

#[track_caller]
fn validate_type<T: HasDiscriminator>(
	account: &AccountView,
	program_id: &Address,
) -> ProgramResult {
	validate_owner(account, program_id)?;

	let data = account.try_borrow()?;

	if !T::matches_discriminator(&data) {
		log!(
			"address: {} has invalid discriminator",
			account.address().as_ref()
		);
		log_caller();

		return Err(ProgramError::InvalidAccountData);
	}

	if data.len() != size_of::<T>() {
		log!(
			"address: {} has invalid data length for the account type",
			account.address().as_ref()
		);
		log_caller();

		return Err(ProgramError::AccountDataTooSmall);
	}

	Ok(())
}

#[track_caller]
fn validate_sysvar(account: &AccountView, sysvar_id: &Address) -> ProgramResult {
	validate_owner(account, &SYSVAR_ID)?;
	validate_address(account, sysvar_id)
}

#[track_caller]
fn validate_owner(account: &AccountView, owner: &Address) -> ProgramResult {
	let account_owner = account.owner();

	if account_owner.ne(owner) {
		log!(
			"address: {} has invalid owner: {}, required: {}",
			account.address().as_ref(),
			account_owner.as_ref(),
			owner.as_ref()
		);
		log_caller();

		return Err(ProgramError::InvalidAccountOwner);
	}

	Ok(())
}

#[track_caller]
fn validate_owners(account: &AccountView, owners: &[Address]) -> ProgramResult {
	let account_owner = account.owner();

	if owners.contains(account_owner) {
		return Ok(());
	}

	log!(
		"address: {} has invalid owner: {}",
		account.address().as_ref(),
		account_owner.as_ref(),
	);
	log_caller();

	Err(ProgramError::InvalidAccountOwner)
}

#[track_caller]
fn validate_address(account: &AccountView, addr: &Address) -> ProgramResult {
	if account.address() == addr {
		return Ok(());
	}

	log!(
		"address: {} is invalid, expected: {}",
		account.address().as_ref(),
		addr.as_ref()
	);
	log_caller();

	Err(ProgramError::InvalidAccountData)
}

#[track_caller]
fn validate_addresses(account: &AccountView, addresses: &[Address]) -> ProgramResult {
	if addresses.contains(account.address()) {
		return Ok(());
	}

	log!("address: {} is invalid", account.address().as_ref());
	log_caller();

	Err(ProgramError::InvalidAccountData)
}

#[track_caller]
fn validate_seeds(account: &AccountView, seeds: &[&[u8]], program_id: &Address) -> ProgramResult {
	let Some((pda, _bump)) = crate::try_find_program_address(seeds, program_id) else {
		log!(
			"could not find program address from seeds with program id: {}",
			program_id.as_ref()
		);
		log_caller();

		return Err(ProgramError::InvalidSeeds);
	};

	if account.address() == &pda {
		return Ok(());
	}

	log!(
		"address: {} is invalid, expected pda: {}",
		account.address().as_ref(),
		pda.as_ref()
	);
	log_caller();

	Err(ProgramError::InvalidSeeds)
}

#[track_caller]
fn validate_seeds_with_bump(
	account: &AccountView,
	seeds: &[&[u8]],
	program_id: &Address,
) -> ProgramResult {
	let pda = match crate::create_program_address(seeds, program_id) {
		Ok(pda) => pda,
		Err(_error) => {
			log!(
				"could not create pda for address: {}, with provided seeds and bump",
				account.address().as_ref(),
			);
			log_caller();

			return Err(ProgramError::InvalidSeeds);
		}
	};

	if &pda != account.address() {
		log!(
			"address: {} is invalid, expected pda: {}",
			account.address().as_ref(),
			pda.as_ref()
		);
		log_caller();

		return Err(ProgramError::InvalidSeeds);
	}

	Ok(())
}

#[track_caller]
fn validate_canonical_bump(
	account: &AccountView,
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

	if pda.eq(account.address()) {
		return Ok(bump);
	}

	log!(
		"address: {} is invalid, expected pda: {}",
		account.address().as_ref(),
		pda.as_ref()
	);
	log_caller();

	Err(ProgramError::InvalidSeeds)
}

#[cfg(feature = "token")]
#[track_caller]
fn validate_associated_token_address(
	account: &AccountView,
	wallet: &Address,
	mint: &Address,
	token_program: &Address,
) -> ProgramResult {
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

	if ata_address.eq(account.address()) {
		return Ok(());
	}

	log!(
		"address: {} is invalid, expected associated token address: {}",
		account.address().as_ref(),
		ata_address.as_ref()
	);
	log_caller();

	Err(ProgramError::InvalidSeeds)
}

macro_rules! impl_account_info_validation {
	($type:ty) => {
		impl<'a> AccountInfoValidation for $type {
			#[track_caller]
			fn assert_signer(self) -> Result<Self, ProgramError> {
				validate_signer(self)?;

				Ok(self)
			}

			#[track_caller]
			fn assert_writable(self) -> Result<Self, ProgramError> {
				validate_writable(self)?;

				Ok(self)
			}

			#[track_caller]
			fn assert_executable(self) -> Result<Self, ProgramError> {
				validate_executable(self)?;

				Ok(self)
			}

			#[track_caller]
			fn assert_data_len(self, len: usize) -> Result<Self, ProgramError> {
				validate_data_len(self, len)?;

				Ok(self)
			}

			#[track_caller]
			fn assert_empty(self) -> Result<Self, ProgramError> {
				validate_empty(self)?;

				Ok(self)
			}

			#[track_caller]
			fn assert_not_empty(self) -> Result<Self, ProgramError> {
				validate_not_empty(self)?;

				Ok(self)
			}

			#[track_caller]
			fn assert_type<T: HasDiscriminator>(
				self,
				program_id: &Address,
			) -> Result<Self, ProgramError> {
				validate_type::<T>(self, program_id)?;

				Ok(self)
			}

			#[track_caller]
			fn assert_program(self, program_id: &Address) -> Result<Self, ProgramError> {
				validate_program(self, program_id)?;

				Ok(self)
			}

			#[track_caller]
			fn assert_sysvar(self, sysvar_id: &Address) -> Result<Self, ProgramError> {
				validate_sysvar(self, sysvar_id)?;

				Ok(self)
			}

			#[track_caller]
			fn assert_address(self, address: &Address) -> Result<Self, ProgramError> {
				validate_address(self, address)?;

				Ok(self)
			}

			#[track_caller]
			fn assert_addresses(self, addresses: &[Address]) -> Result<Self, ProgramError> {
				validate_addresses(self, addresses)?;

				Ok(self)
			}

			#[track_caller]
			fn assert_owner(self, owner: &Address) -> Result<Self, ProgramError> {
				validate_owner(self, owner)?;

				Ok(self)
			}

			#[track_caller]
			fn assert_owners(self, owners: &[Address]) -> Result<Self, ProgramError> {
				validate_owners(self, owners)?;

				Ok(self)
			}

			#[track_caller]
			fn assert_seeds(
				self,
				seeds: &[&[u8]],
				program_id: &Address,
			) -> Result<Self, ProgramError> {
				validate_seeds(self, seeds, program_id)?;

				Ok(self)
			}

			#[track_caller]
			fn assert_seeds_with_bump(
				self,
				seeds: &[&[u8]],
				program_id: &Address,
			) -> Result<Self, ProgramError> {
				validate_seeds_with_bump(self, seeds, program_id)?;

				Ok(self)
			}

			#[track_caller]
			fn assert_canonical_bump(
				self,
				seeds: &[&[u8]],
				program_id: &Address,
			) -> Result<u8, ProgramError> {
				validate_canonical_bump(self, seeds, program_id)
			}

			#[cfg(feature = "token")]
			#[track_caller]
			fn assert_associated_token_address(
				self,
				wallet: &Address,
				mint: &Address,
				token_program: &Address,
			) -> Result<Self, ProgramError> {
				validate_associated_token_address(self, wallet, mint, token_program)?;

				Ok(self)
			}
		}
	};
}

impl_account_info_validation!(&'a AccountView);
impl_account_info_validation!(&'a mut AccountView);

impl AsAccount for AccountView {
	#[track_caller]
	fn as_account<T>(&self, program_id: &Address) -> Result<Ref<'_, T>, ProgramError>
	where
		T: AccountDeserialize + HasDiscriminator + Pod,
	{
		self.assert_owner(program_id)?;
		self.assert_data_len(size_of::<T>())?;

		Ref::try_map(self.try_borrow()?, |data| T::try_from_bytes(data))
			.map_err(|(_guard, error)| error)
	}

	#[track_caller]
	fn as_account_mut<T>(&mut self, program_id: &Address) -> Result<RefMut<'_, T>, ProgramError>
	where
		T: AccountDeserialize + HasDiscriminator + Pod,
	{
		self.assert_owner(program_id)?;
		self.assert_data_len(size_of::<T>())?;

		RefMut::try_map(self.try_borrow_mut()?, |data| T::try_from_bytes_mut(data))
			.map_err(|(_guard, error)| error)
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
				crate::assert(condition(self), ProgramError::InvalidAccountData, log)?;

				Ok(self)
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
				crate::assert(condition(self), ProgramError::InvalidAccountData, log)?;

				Ok(self)
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

		// SAFETY: `check_borrow()` verified the account is not already mutably
		// borrowed. `from_account_view_unchecked` performs a pointer cast from
		// the account data to the target type layout. The caller is responsible
		// for verifying ownership before trusting the result — the `_checked`
		// variants handle this automatically.
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

		// SAFETY: `check_borrow()` verified the account is not already mutably
		// borrowed. `from_account_view_unchecked` performs a pointer cast from
		// the account data to the target type layout. The caller is responsible
		// for verifying ownership before trusting the result — the `_checked`
		// variants handle this automatically.
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

		// SAFETY: `check_borrow()` verified the account is not already mutably
		// borrowed. `from_account_view_unchecked` performs a pointer cast from
		// the account data to the target type layout. The caller is responsible
		// for verifying ownership before trusting the result — the `_checked`
		// variants handle this automatically.
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

		// SAFETY: `check_borrow()` verified the account is not already mutably
		// borrowed. `from_account_view_unchecked` performs a pointer cast from
		// the account data to the target type layout. The caller is responsible
		// for verifying ownership before trusting the result — the `_checked`
		// variants handle this automatically. Additionally, the address is
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

impl LamportTransfer for AccountView {
	/// Send the specified lamports to the `recipient` account.
	/// The sender must be writable and owned by the executing program.
	#[inline(always)]
	#[track_caller]
	fn send(&mut self, lamports: u64, recipient: &mut AccountView) -> ProgramResult {
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
	fn collect(&self, lamports: u64, from: &AccountView) -> Result<(), ProgramError> {
		Transfer {
			from,
			to: self,
			lamports,
		}
		.invoke()
	}
}

impl CloseAccountWithRecipient for AccountView {
	#[track_caller]
	fn close_with_recipient(&mut self, recipient: &mut AccountView) -> ProgramResult {
		self.assert_writable()?;
		recipient.assert_writable()?;

		if self.address() == recipient.address() {
			log!("Could not close account: recipient must differ from account");
			log_caller();
			return Err(ProgramError::InvalidArgument);
		}

		let new_balance = checked_close_balance(self.lamports(), recipient.lamports())
			.inspect_err(|_| {
				log!("Could not close account: lamport overflow");
				log_caller();
			})?;
		recipient.set_lamports(new_balance);
		self.set_lamports(0);
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

	#[test]
	fn checked_send_balances_exact_balance() {
		let (sender, recipient) = checked_send_balances(5, 3, 5)
			.unwrap_or_else(|err| panic!("expected valid transfer: {err:?}"));
		assert_eq!(sender, 0);
		assert_eq!(recipient, 8);
	}

	#[test]
	fn checked_send_balances_zero_transfer() {
		let (sender, recipient) = checked_send_balances(10, 5, 0)
			.unwrap_or_else(|err| panic!("expected valid transfer: {err:?}"));
		assert_eq!(sender, 10);
		assert_eq!(recipient, 5);
	}

	#[test]
	fn checked_send_balances_max_values() {
		// Test with large values near u64::MAX boundaries.
		let result = checked_send_balances(u64::MAX, 0, u64::MAX);
		let (sender, recipient) =
			result.unwrap_or_else(|err| panic!("expected valid transfer: {err:?}"));
		assert_eq!(sender, 0);
		assert_eq!(recipient, u64::MAX);
	}

	#[test]
	fn checked_close_balance_zero_sender() {
		let result = checked_close_balance(0, 5)
			.unwrap_or_else(|err| panic!("expected valid close: {err:?}"));
		assert_eq!(result, 5);
	}

	#[test]
	fn checked_close_balance_both_zero() {
		let result = checked_close_balance(0, 0)
			.unwrap_or_else(|err| panic!("expected valid close: {err:?}"));
		assert_eq!(result, 0);
	}

	#[test]
	fn checked_close_balance_max_sender_zero_recipient() {
		let result = checked_close_balance(u64::MAX, 0)
			.unwrap_or_else(|err| panic!("expected valid close: {err:?}"));
		assert_eq!(result, u64::MAX);
	}
}
