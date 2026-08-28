//! SPL Token re-exports and safe multi-program token views.

pub use pinocchio_token::*;

use crate::AccountInfoValidation;
use crate::AccountView;
use crate::Address;
use crate::ProgramError;
use crate::Ref;

fn validate_extensions_allowed<B>(
	state: &crate::token_2022::state::StateWithExtensions<B>,
	allowed: &[crate::token_2022::state::ExtensionType],
) -> Result<(), ProgramError>
where
	B: crate::token_2022::state::ExtensionBaseState,
{
	let mut extensions = [crate::token_2022::state::ExtensionType::Uninitialized;
		crate::token_2022::state::MAX_EXTENSIONS];
	let count = state.write_extension_types(&mut extensions)?;

	if extensions[..count]
		.iter()
		.all(|extension| allowed.contains(extension))
	{
		return Ok(());
	}

	Err(ProgramError::InvalidAccountData)
}

pub mod state {
	pub use pinocchio_token::state::*;

	pub type TokenAccount = Account;
}

/// A validated mint borrow for either supported SPL Token program.
///
/// The variant preserves the concrete upstream type that performed validation,
/// so Pina never needs to reinterpret Token-2022 storage as a legacy token
/// struct. Common mint fields remain available directly on this enum; callers
/// that need extensions can match [`Self::Token2022`].
pub enum TokenMintRef<'a> {
	/// A mint owned and validated by the original SPL Token program.
	Legacy(Ref<'a, state::Mint>),
	/// A mint owned and validated by Token-2022, including its extension layout.
	Token2022(
		Ref<'a, crate::token_2022::state::StateWithExtensions<crate::token_2022::state::Mint>>,
	),
}

impl<'a> TokenMintRef<'a> {
	/// Validate `account` using the selected token program.
	///
	/// `token_program` must equal the canonical SPL Token or Token-2022 program
	/// ID, and `account` must be owned by that same program. The resulting enum
	/// variant therefore reflects validated program ownership rather than an
	/// untrusted caller selection.
	///
	/// # Errors
	///
	/// Returns `IncorrectProgramId` for unsupported programs, or the validation
	/// error returned by the selected upstream token crate.
	pub fn from_account_view(
		account: &'a AccountView,
		token_program: &Address,
	) -> Result<Self, ProgramError> {
		if token_program == &ID {
			account.assert_owner(token_program)?;
			return state::Mint::from_account_view(account).map(Self::Legacy);
		}

		if token_program == &crate::token_2022::ID {
			account.assert_owner(token_program)?;
			return crate::token_2022::state::StateWithExtensions::<
				crate::token_2022::state::Mint,
			>::from_account_view(account)
			.map(Self::Token2022);
		}

		Err(ProgramError::IncorrectProgramId)
	}

	/// Return the program that validated this mint.
	pub const fn program_id(&self) -> &Address {
		match self {
			Self::Legacy(_) => &ID,
			Self::Token2022(_) => &crate::token_2022::ID,
		}
	}

	/// Require every Token-2022 extension on this mint to appear in `allowed`.
	///
	/// Legacy SPL Token mints have no extensions and always satisfy the policy.
	/// This check makes protocol support explicit instead of silently relying on
	/// legacy base fields when Token-2022 changes transfer or authority semantics.
	/// Returns the validated mint view so this assertion can be chained directly
	/// after [`crate::AsTokenAccount::as_token_mint_for_program`].
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidAccountData`] when a Token-2022 extension is
	/// not allow-listed, or propagates malformed extension-data errors.
	pub fn assert_extensions_allowed(
		self,
		allowed: &[crate::token_2022::state::ExtensionType],
	) -> Result<Self, ProgramError> {
		if let Self::Token2022(mint) = &self {
			validate_extensions_allowed(mint, allowed)?;
		}

		Ok(self)
	}

	/// Reject every Token-2022 mint extension.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidAccountData`] when any extension is present,
	/// or propagates malformed extension-data errors.
	pub fn assert_no_extensions(self) -> Result<Self, ProgramError> {
		self.assert_extensions_allowed(&[])
	}

	/// Borrow the legacy mint when this view belongs to SPL Token.
	pub fn legacy(&self) -> Option<&state::Mint> {
		match self {
			Self::Legacy(mint) => Some(mint),
			Self::Token2022(_) => None,
		}
	}

	/// Borrow the complete Token-2022 state when extensions are available.
	pub fn token_2022(
		&self,
	) -> Option<&crate::token_2022::state::StateWithExtensions<crate::token_2022::state::Mint>> {
		match self {
			Self::Legacy(_) => None,
			Self::Token2022(mint) => Some(mint),
		}
	}

	/// Return the optional mint authority.
	pub fn mint_authority(&self) -> Option<&Address> {
		match self {
			Self::Legacy(mint) => mint.mint_authority(),
			Self::Token2022(mint) => mint.base.mint_authority(),
		}
	}

	/// Return the total token supply.
	pub fn supply(&self) -> u64 {
		match self {
			Self::Legacy(mint) => mint.supply(),
			Self::Token2022(mint) => mint.base.supply(),
		}
	}

	/// Return the number of base-ten decimal places.
	pub fn decimals(&self) -> u8 {
		match self {
			Self::Legacy(mint) => mint.decimals(),
			Self::Token2022(mint) => mint.base.decimals(),
		}
	}

	/// Return whether the mint is initialized.
	pub fn is_initialized(&self) -> bool {
		match self {
			Self::Legacy(mint) => mint.is_initialized(),
			Self::Token2022(mint) => mint.base.is_initialized(),
		}
	}

	/// Return the optional freeze authority.
	pub fn freeze_authority(&self) -> Option<&Address> {
		match self {
			Self::Legacy(mint) => mint.freeze_authority(),
			Self::Token2022(mint) => mint.base.freeze_authority(),
		}
	}
}

/// A validated token-account borrow for either supported SPL Token program.
///
/// Common base-account fields are exposed directly. Match
/// [`Self::Token2022`] when an extension must be inspected.
pub enum TokenAccountRef<'a> {
	/// An account owned and validated by the original SPL Token program.
	Legacy(Ref<'a, state::TokenAccount>),
	/// An account owned and validated by Token-2022, including extensions.
	Token2022(
		Ref<
			'a,
			crate::token_2022::state::StateWithExtensions<crate::token_2022::state::TokenAccount>,
		>,
	),
}

impl<'a> TokenAccountRef<'a> {
	/// Validate `account` using the selected token program.
	///
	/// `token_program` must equal the canonical SPL Token or Token-2022 program
	/// ID, and `account` must be owned by that same program. The resulting enum
	/// variant therefore reflects validated program ownership rather than an
	/// untrusted caller selection.
	///
	/// # Errors
	///
	/// Returns `IncorrectProgramId` for unsupported programs, or the validation
	/// error returned by the selected upstream token crate.
	pub fn from_account_view(
		account: &'a AccountView,
		token_program: &Address,
	) -> Result<Self, ProgramError> {
		if token_program == &ID {
			account.assert_owner(token_program)?;
			return state::TokenAccount::from_account_view(account).map(Self::Legacy);
		}

		if token_program == &crate::token_2022::ID {
			account.assert_owner(token_program)?;
			return crate::token_2022::state::StateWithExtensions::<
				crate::token_2022::state::TokenAccount,
			>::from_account_view(account)
			.map(Self::Token2022);
		}

		Err(ProgramError::IncorrectProgramId)
	}

	/// Return the program that validated this token account.
	pub const fn program_id(&self) -> &Address {
		match self {
			Self::Legacy(_) => &ID,
			Self::Token2022(_) => &crate::token_2022::ID,
		}
	}

	/// Require every Token-2022 extension on this token account to appear in
	/// `allowed`.
	///
	/// Legacy SPL Token accounts have no extensions and always satisfy the
	/// policy. Account policies should be paired with a mint policy because the
	/// mint controls transfer-wide behavior such as fees and hooks.
	/// Returns the validated token-account view so this assertion can be chained
	/// directly after [`crate::AsTokenAccount::as_token_account_for_program`].
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidAccountData`] when a Token-2022 extension is
	/// not allow-listed, or propagates malformed extension-data errors.
	pub fn assert_extensions_allowed(
		self,
		allowed: &[crate::token_2022::state::ExtensionType],
	) -> Result<Self, ProgramError> {
		if let Self::Token2022(account) = &self {
			validate_extensions_allowed(account, allowed)?;
		}

		Ok(self)
	}

	/// Reject every Token-2022 token-account extension.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidAccountData`] when any extension is present,
	/// or propagates malformed extension-data errors.
	pub fn assert_no_extensions(self) -> Result<Self, ProgramError> {
		self.assert_extensions_allowed(&[])
	}

	/// Borrow the legacy token account when this view belongs to SPL Token.
	pub fn legacy(&self) -> Option<&state::TokenAccount> {
		match self {
			Self::Legacy(account) => Some(account),
			Self::Token2022(_) => None,
		}
	}

	/// Borrow the complete Token-2022 state when extensions are available.
	pub fn token_2022(
		&self,
	) -> Option<
		&crate::token_2022::state::StateWithExtensions<crate::token_2022::state::TokenAccount>,
	> {
		match self {
			Self::Legacy(_) => None,
			Self::Token2022(account) => Some(account),
		}
	}

	/// Return the mint associated with this account.
	pub fn mint(&self) -> &Address {
		match self {
			Self::Legacy(account) => account.mint(),
			Self::Token2022(account) => account.base.mint(),
		}
	}

	/// Return the wallet that owns this token account.
	pub fn owner(&self) -> &Address {
		match self {
			Self::Legacy(account) => account.owner(),
			Self::Token2022(account) => account.base.owner(),
		}
	}

	/// Return the token balance.
	pub fn amount(&self) -> u64 {
		match self {
			Self::Legacy(account) => account.amount(),
			Self::Token2022(account) => account.base.amount(),
		}
	}

	/// Return the optional transfer delegate.
	pub fn delegate(&self) -> Option<&Address> {
		match self {
			Self::Legacy(account) => account.delegate(),
			Self::Token2022(account) => account.base.delegate(),
		}
	}

	/// Return whether this account wraps native SOL.
	pub fn is_native(&self) -> bool {
		match self {
			Self::Legacy(account) => account.is_native(),
			Self::Token2022(account) => account.base.is_native(),
		}
	}

	/// Return the rent reserve for a native account.
	pub fn native_amount(&self) -> Option<u64> {
		match self {
			Self::Legacy(account) => account.native_amount(),
			Self::Token2022(account) => account.base.native_amount(),
		}
	}

	/// Return the amount currently delegated.
	pub fn delegated_amount(&self) -> u64 {
		match self {
			Self::Legacy(account) => account.delegated_amount(),
			Self::Token2022(account) => account.base.delegated_amount(),
		}
	}

	/// Return the optional close authority.
	pub fn close_authority(&self) -> Option<&Address> {
		match self {
			Self::Legacy(account) => account.close_authority(),
			Self::Token2022(account) => account.base.close_authority(),
		}
	}

	/// Return whether the account is initialized.
	pub fn is_initialized(&self) -> bool {
		match self {
			Self::Legacy(account) => account.is_initialized(),
			Self::Token2022(account) => account.base.is_initialized(),
		}
	}

	/// Return whether the account is frozen.
	pub fn is_frozen(&self) -> bool {
		match self {
			Self::Legacy(account) => account.is_frozen(),
			Self::Token2022(account) => account.base.is_frozen(),
		}
	}
}
