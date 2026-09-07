//! CPI and account-allocation helpers used by on-chain instruction handlers.
//!
//! These builders wrap common system-program patterns (create, allocate,
//! assign, close) with consistent `ProgramError` behavior and PDA signing.
//! All APIs in this module are designed for on-chain determinism and return
//! `ProgramError` values for caller-side propagation with `?` instead of
//! panicking.
//!
//! Seed-based helpers require deterministic seed ordering and consistent
//! program IDs across derivation and verification.

use pinocchio::AccountView;
use pinocchio::Address;
#[cfg(feature = "account-resize")]
use pinocchio::Resize;
use pinocchio::cpi::Seed;
use pinocchio::cpi::Signer;
use pinocchio::error::ProgramError;
use pinocchio::instruction::InstructionAccount;
use pinocchio::instruction::InstructionView;
use pinocchio::sysvars::Sysvar;
use pinocchio::sysvars::rent::Rent;
use pinocchio_system::instructions::Allocate as SystemAllocate;
use pinocchio_system::instructions::Assign as SystemAssign;
use pinocchio_system::instructions::CreateAccount as SystemCreateAccount;
use pinocchio_system::instructions::Transfer as SystemTransfer;

use crate::AccountInfoValidation;
use crate::CloseAccountWithRecipient;
#[cfg(all(feature = "account-resize", feature = "compact"))]
use crate::CompactAccountInfoValidation;
#[cfg(feature = "account-resize")]
use crate::LamportTransfer;
use crate::MAX_SEEDS;
use crate::PinaAccount;
#[cfg(all(feature = "account-resize", feature = "compact"))]
use crate::PinaCompactAccount;
use crate::ProgramResult;

/// Creates a rent-exempt system account owned by another program.
///
/// Use this builder when both the funding account and new account are regular
/// transaction signers. Use [`CreateProgramAccount`] or
/// [`CreateProgramAccountWithBump`] when the new account is a PDA controlled by
/// the executing program. Rent is loaded through the runtime syscall, so no
/// rent-sysvar account belongs in this builder.
///
/// # Errors
///
/// `invoke` returns errors from rent sysvar access, minimum-balance
/// computation, or the system-program CPI. `invoke_signed` can additionally
/// authorize a PDA funding account or PDA destination with signer seeds.
///
/// # Examples
///
/// ```ignore
/// use pina::CreateAccount;
///
/// // Create a new account with 128 bytes of space owned by `program_id`:
/// CreateAccount {
/// 	from: payer,
/// 	to: new_account,
/// 	space: 128,
/// 	owner: &program_id,
/// }
/// .invoke()?;
/// ```
#[derive(Clone, Copy, Debug)]
#[must_use = "account creation has no effect until invoke or invoke_signed is called"]
pub struct CreateAccount<'account, 'address> {
	/// Funding account that pays the new account's rent-exempt balance.
	pub from: &'account AccountView,

	/// New account to fund, allocate, and assign.
	pub to: &'account AccountView,

	/// Number of account-data bytes to allocate.
	pub space: u64,

	/// Program that will own the new account.
	pub owner: &'address Address,
}

impl CreateAccount<'_, '_> {
	/// Creates the account using transaction-level signatures.
	#[inline(always)]
	pub fn invoke(&self) -> ProgramResult {
		self.invoke_signed(&[])
	}

	/// Creates the account with additional PDA signer seeds.
	///
	/// Use this variant when `from`, `to`, or both are PDAs controlled by the
	/// executing program.
	#[inline(always)]
	pub fn invoke_signed(&self, signers: &[Signer<'_, '_>]) -> ProgramResult {
		let space = usize::try_from(self.space).map_err(|_| ProgramError::InvalidArgument)?;
		self.invoke_signed_inner(signers, None, space)
	}

	#[cfg(test)]
	#[inline(always)]
	fn invoke_signed_with_rent(
		&self,
		signers: &[Signer<'_, '_>],
		rent: Rent,
		space: usize,
	) -> ProgramResult {
		self.invoke_signed_inner(signers, Some(rent), space)
	}

	#[inline(always)]
	fn invoke_signed_inner(
		&self,
		signers: &[Signer<'_, '_>],
		rent: Option<Rent>,
		space: usize,
	) -> ProgramResult {
		let rent = rent.map_or_else(Rent::get, Ok)?;

		SystemCreateAccount {
			from: self.from,
			to: self.to,
			lamports: rent.try_minimum_balance(space)?,
			space: self.space,
			owner: self.owner,
		}
		.invoke_signed(signers)
	}
}

/// Creates and initializes a PDA-backed account for a [`PinaAccount`] type.
///
/// This builder derives the canonical PDA for `seeds` + `owner`, allocates
/// account storage for `T`, initializes its discriminator, and assigns account
/// ownership to `owner`.
///
/// <!-- {=pinaPdaSeedContract|trim|linePrefix:"/// ":true} -->
/// Seed-based APIs require deterministic seed ordering.
///
/// Program IDs must stay consistent across derivation and verification.
///
/// When a bump is required, prefer canonical bump derivation.
///
/// Use explicit bumps when needed.<!-- {/pinaPdaSeedContract} -->
///
/// # Errors
///
/// Returns `InvalidSeeds` when no valid PDA can be derived, plus any errors
/// from allocation/assignment steps.
///
/// # Examples
///
/// ```ignore
/// // Create a PDA-backed escrow account:
/// let seeds: &[&[u8]] = &[b"escrow", authority.address().as_ref()];
/// let (address, bump) = CreateProgramAccount {
/// 	account: escrow_account,
/// 	payer,
/// 	owner: &program_id,
/// 	seeds,
/// }
/// .invoke::<EscrowState>()?;
/// ```
#[must_use = "account creation has no effect until invoke or invoke_signed is called"]
pub struct CreateProgramAccount<'account, 'address, 'seeds, 'seed> {
	/// PDA account to allocate and initialize.
	pub account: &'account mut AccountView,

	/// Funding account that pays any required rent-exempt balance.
	pub payer: &'account AccountView,

	/// Program that owns the PDA and derives it from `seeds`.
	pub owner: &'address Address,

	/// PDA seeds without the canonical bump.
	pub seeds: &'seeds [&'seed [u8]],
}

impl CreateProgramAccount<'_, '_, '_, '_> {
	/// Creates the account using the canonical PDA bump.
	#[inline(always)]
	pub fn invoke<T: PinaAccount>(&mut self) -> Result<(Address, u8), ProgramError> {
		self.invoke_signed::<T>(&[])
	}

	/// Creates the account using the canonical PDA bump and additional signers.
	///
	/// Additional signers are useful when `payer` is another PDA. The target
	/// account's signer is derived and supplied automatically.
	#[inline(always)]
	pub fn invoke_signed<T: PinaAccount>(
		&mut self,
		signers: &[Signer<'_, '_>],
	) -> Result<(Address, u8), ProgramError> {
		self.invoke_signed_inner::<T>(signers, None)
	}

	#[cfg(test)]
	#[inline(always)]
	fn invoke_signed_with_rent<T: PinaAccount>(
		&mut self,
		signers: &[Signer<'_, '_>],
		rent: Rent,
	) -> Result<(Address, u8), ProgramError> {
		self.invoke_signed_inner::<T>(signers, Some(rent))
	}

	#[inline(always)]
	fn invoke_signed_inner<T: PinaAccount>(
		&mut self,
		signers: &[Signer<'_, '_>],
		rent: Option<Rent>,
	) -> Result<(Address, u8), ProgramError> {
		let Some((address, bump)) = crate::try_find_program_address(self.seeds, self.owner) else {
			return Err(ProgramError::InvalidSeeds);
		};

		CreateProgramAccountWithBump {
			account: self.account,
			payer: self.payer,
			owner: self.owner,
			seeds: self.seeds,
			bump,
		}
		.invoke_signed_inner::<T>(signers, rent)?;

		Ok((address, bump))
	}
}

/// Creates a PDA-backed program account using a caller-provided `bump` and
/// initializes `T`'s discriminator.
///
/// Prefer [`CreateProgramAccount`] when you want canonical bump derivation.
/// Use this builder when the bump is instruction data and must be validated.
///
/// <!-- {=pinaPdaSeedContract|trim|linePrefix:"/// ":true} -->
/// Seed-based APIs require deterministic seed ordering.
///
/// Program IDs must stay consistent across derivation and verification.
///
/// When a bump is required, prefer canonical bump derivation.
///
/// Use explicit bumps when needed.<!-- {/pinaPdaSeedContract} -->
///
/// # Errors
///
/// Returns any error produced by [`AllocateAccountWithBump`], including
/// invalid seed layouts and system-program CPI failures.
///
/// # Examples
///
/// ```ignore
/// // Create a PDA-backed account when you already know the bump:
/// let seeds: &[&[u8]] = &[b"escrow", authority.address().as_ref()];
/// CreateProgramAccountWithBump {
/// 	account: escrow_account,
/// 	payer,
/// 	owner: &program_id,
/// 	seeds,
/// 	bump,
/// }
/// .invoke::<EscrowState>()?;
/// ```
#[must_use = "account creation has no effect until invoke or invoke_signed is called"]
pub struct CreateProgramAccountWithBump<'account, 'address, 'seeds, 'seed> {
	/// PDA account to allocate and initialize.
	pub account: &'account mut AccountView,

	/// Funding account that pays any required rent-exempt balance.
	pub payer: &'account AccountView,

	/// Program that owns the PDA and derives it from `seeds` and `bump`.
	pub owner: &'address Address,

	/// PDA seeds without the bump.
	pub seeds: &'seeds [&'seed [u8]],

	/// PDA bump to validate and append to `seeds`.
	pub bump: u8,
}

impl CreateProgramAccountWithBump<'_, '_, '_, '_> {
	/// Creates the account and writes `T`'s discriminator.
	#[inline(always)]
	pub fn invoke<T: PinaAccount>(&mut self) -> ProgramResult {
		self.invoke_signed::<T>(&[])
	}

	/// Creates the account with additional PDA signers and writes `T`'s
	/// discriminator.
	///
	/// The target account signer is derived and supplied automatically.
	#[inline(always)]
	pub fn invoke_signed<T: PinaAccount>(&mut self, signers: &[Signer<'_, '_>]) -> ProgramResult {
		self.invoke_signed_inner::<T>(signers, None)
	}

	#[cfg(test)]
	#[inline(always)]
	fn invoke_signed_with_rent<T: PinaAccount>(
		&mut self,
		signers: &[Signer<'_, '_>],
		rent: Rent,
	) -> ProgramResult {
		self.invoke_signed_inner::<T>(signers, Some(rent))
	}

	#[inline(always)]
	fn invoke_signed_inner<T: PinaAccount>(
		&mut self,
		signers: &[Signer<'_, '_>],
		rent: Option<Rent>,
	) -> ProgramResult {
		AllocateAccountWithBump {
			account: self.account,
			payer: self.payer,
			space: T::SIZE as u64,
			owner: self.owner,
			seeds: self.seeds,
			bump: self.bump,
		}
		.invoke_signed_inner(signers, rent)?;

		let mut data = self.account.try_borrow_mut()?;
		T::write_discriminator(&mut data);

		Ok(())
	}
}

/// Creates and initializes a variable-length PDA-backed account.
#[cfg(all(feature = "account-resize", feature = "compact"))]
#[must_use = "account creation has no effect until invoke or invoke_signed is called"]
pub struct CreateCompactProgramAccount<'account, 'address, 'seeds, 'seed> {
	/// PDA account to allocate and initialize.
	pub account: &'account mut AccountView,
	/// Funding account that pays the rent-exempt balance.
	pub payer: &'account AccountView,
	/// Program that owns and derives the PDA.
	pub owner: &'address Address,
	/// PDA seeds without the canonical bump.
	pub seeds: &'seeds [&'seed [u8]],
	/// Initial byte length, bounded by the compact schema.
	pub space: usize,
}

#[cfg(all(feature = "account-resize", feature = "compact"))]
impl CreateCompactProgramAccount<'_, '_, '_, '_> {
	/// Creates the compact account using its canonical PDA bump.
	pub fn invoke<T: PinaCompactAccount>(&mut self) -> Result<(Address, u8), ProgramError> {
		self.invoke_signed::<T>(&[])
	}

	/// Creates the compact account with additional payer signer seeds.
	pub fn invoke_signed<T: PinaCompactAccount>(
		&mut self,
		signers: &[Signer<'_, '_>],
	) -> Result<(Address, u8), ProgramError> {
		self.invoke_signed_inner::<T>(signers, None)
	}

	#[cfg(test)]
	fn invoke_signed_with_rent<T: PinaCompactAccount>(
		&mut self,
		signers: &[Signer<'_, '_>],
		rent: Rent,
	) -> Result<(Address, u8), ProgramError> {
		self.invoke_signed_inner::<T>(signers, Some(rent))
	}

	fn invoke_signed_inner<T: PinaCompactAccount>(
		&mut self,
		signers: &[Signer<'_, '_>],
		rent: Option<Rent>,
	) -> Result<(Address, u8), ProgramError> {
		let Some((address, bump)) = crate::try_find_program_address(self.seeds, self.owner) else {
			return Err(ProgramError::InvalidSeeds);
		};

		CreateCompactProgramAccountWithBump {
			account: self.account,
			payer: self.payer,
			owner: self.owner,
			seeds: self.seeds,
			bump,
			space: self.space,
		}
		.invoke_signed_inner::<T>(signers, rent)?;

		Ok((address, bump))
	}
}

/// Creates a variable-length PDA-backed account using an explicit bump.
#[cfg(all(feature = "account-resize", feature = "compact"))]
#[must_use = "account creation has no effect until invoke or invoke_signed is called"]
pub struct CreateCompactProgramAccountWithBump<'account, 'address, 'seeds, 'seed> {
	/// PDA account to allocate and initialize.
	pub account: &'account mut AccountView,
	/// Funding account that pays the rent-exempt balance.
	pub payer: &'account AccountView,
	/// Program that owns and derives the PDA.
	pub owner: &'address Address,
	/// PDA seeds without the bump.
	pub seeds: &'seeds [&'seed [u8]],
	/// PDA bump to validate and append to `seeds`.
	pub bump: u8,
	/// Initial byte length, bounded by the compact schema.
	pub space: usize,
}

#[cfg(all(feature = "account-resize", feature = "compact"))]
impl CreateCompactProgramAccountWithBump<'_, '_, '_, '_> {
	/// Creates and initializes the compact account.
	pub fn invoke<T: PinaCompactAccount>(&mut self) -> ProgramResult {
		self.invoke_signed::<T>(&[])
	}

	/// Creates and initializes the compact account with extra payer signers.
	pub fn invoke_signed<T: PinaCompactAccount>(
		&mut self,
		signers: &[Signer<'_, '_>],
	) -> ProgramResult {
		self.invoke_signed_inner::<T>(signers, None)
	}

	fn invoke_signed_inner<T: PinaCompactAccount>(
		&mut self,
		signers: &[Signer<'_, '_>],
		rent: Option<Rent>,
	) -> ProgramResult {
		T::validate_size(self.space)?;
		AllocateAccountWithBump {
			account: self.account,
			payer: self.payer,
			space: self.space as u64,
			owner: self.owner,
			seeds: self.seeds,
			bump: self.bump,
		}
		.invoke_signed_inner(signers, rent)?;

		let mut data = self.account.try_borrow_mut()?;
		T::initialize(&mut data)?;

		Ok(())
	}
}

/// Allocates space for a new program account, returning the derived `address`
/// and the canonical `bump`.
///
/// This is the lower-level allocator used by [`CreateProgramAccount`] for
/// cases where caller code wants manual discriminator/data initialization.
///
/// <!-- {=pinaPdaSeedContract|trim|linePrefix:"/// ":true} -->
/// Seed-based APIs require deterministic seed ordering.
///
/// Program IDs must stay consistent across derivation and verification.
///
/// When a bump is required, prefer canonical bump derivation.
///
/// Use explicit bumps when needed.<!-- {/pinaPdaSeedContract} -->
///
/// # Errors
///
/// Returns `InvalidSeeds` when no canonical PDA can be derived, plus any
/// allocation errors surfaced by [`AllocateAccountWithBump`].
///
/// # Examples
///
/// ```ignore
/// // Allocate raw space for manual initialization:
/// let seeds: &[&[u8]] = &[b"vault"];
/// let (address, bump) = AllocateAccount {
/// 	account: vault_account,
/// 	payer,
/// 	space: 64,
/// 	owner: &program_id,
/// 	seeds,
/// }
/// .invoke()?;
/// ```
#[derive(Clone, Copy, Debug)]
#[must_use = "account allocation has no effect until invoke or invoke_signed is called"]
pub struct AllocateAccount<'account, 'address, 'seeds, 'seed> {
	/// PDA account to allocate and assign.
	pub account: &'account AccountView,

	/// Funding account that pays any required rent-exempt balance.
	pub payer: &'account AccountView,

	/// Number of account-data bytes to allocate.
	pub space: u64,

	/// Program that owns the PDA and derives it from `seeds`.
	pub owner: &'address Address,

	/// PDA seeds without the canonical bump.
	pub seeds: &'seeds [&'seed [u8]],
}

impl AllocateAccount<'_, '_, '_, '_> {
	/// Allocates the account using the canonical PDA bump.
	#[inline(always)]
	pub fn invoke(&self) -> Result<(Address, u8), ProgramError> {
		self.invoke_signed(&[])
	}

	/// Allocates the account using the canonical PDA bump and additional
	/// signers.
	///
	/// Additional signers are useful when `payer` is another PDA. The target
	/// account's signer is derived and supplied automatically.
	#[inline(always)]
	pub fn invoke_signed(&self, signers: &[Signer<'_, '_>]) -> Result<(Address, u8), ProgramError> {
		self.invoke_signed_inner(signers, None)
	}

	#[cfg(test)]
	#[inline(always)]
	fn invoke_signed_with_rent(
		&self,
		signers: &[Signer<'_, '_>],
		rent: Rent,
	) -> Result<(Address, u8), ProgramError> {
		self.invoke_signed_inner(signers, Some(rent))
	}

	#[inline(always)]
	fn invoke_signed_inner(
		&self,
		signers: &[Signer<'_, '_>],
		rent: Option<Rent>,
	) -> Result<(Address, u8), ProgramError> {
		let Some((address, bump)) = crate::try_find_program_address(self.seeds, self.owner) else {
			return Err(ProgramError::InvalidSeeds);
		};

		AllocateAccountWithBump {
			account: self.account,
			payer: self.payer,
			space: self.space,
			owner: self.owner,
			seeds: self.seeds,
			bump,
		}
		.invoke_signed_inner(signers, rent)?;

		Ok((address, bump))
	}
}

/// Appends a single-byte bump seed to the provided seeds array, returning
/// a fixed-size `[Seed; MAX_SEEDS]` suitable for PDA signing.
///
/// # Errors
///
/// Returns `ProgramError::InvalidSeeds` if `seeds.len() >= MAX_SEEDS`.
///
/// <!-- {=pinaPdaSeedContract|trim|linePrefix:"/// ":true} -->
/// Seed-based APIs require deterministic seed ordering.
///
/// Program IDs must stay consistent across derivation and verification.
///
/// When a bump is required, prefer canonical bump derivation.
///
/// Use explicit bumps when needed.<!-- {/pinaPdaSeedContract} -->
///
/// # Examples
///
/// ```ignore
/// let escrow_seeds = EscrowState::seeds(&maker, seed).with_bump(bump);
/// let escrow_signer = escrow_seeds.to_signer();
/// let signers = [escrow_signer.as_signer()];
/// ```
///
/// For untyped seed slices, use this lower-level helper directly:
///
/// ```ignore
/// let seeds: &[&[u8]] = &[b"escrow", authority.address().as_ref()];
/// let bump_bytes = [bump];
/// let combined = combine_seeds_with_bump(seeds, &bump_bytes)?;
/// let signer = Signer::from(&combined[..=seeds.len()]);
/// ```
pub fn combine_seeds_with_bump<'a>(
	seeds: &[&'a [u8]],
	bump: &'a [u8; 1],
) -> Result<[Seed<'a>; MAX_SEEDS], ProgramError> {
	if seeds.len() >= MAX_SEEDS {
		return Err(ProgramError::InvalidSeeds);
	}

	// Create our backing storage on the stack, initialized with empty seeds.
	let mut storage: [Seed<'a>; MAX_SEEDS] = core::array::from_fn(|_| Seed::from(&[] as &[u8]));

	// 1. Copy the original seeds into our storage array.
	for (i, seed) in seeds.iter().enumerate() {
		storage[i] = Seed::from(*seed);
	}

	// 2. Add the single-byte bump slice to the end.
	let seeds_len = seeds.len();
	storage[seeds_len] = Seed::from(bump.as_slice());

	Ok(storage)
}

/// Stack-backed PDA signer seeds for Pinocchio CPI calls.
///
/// Pinocchio's [`Signer`] borrows a seed array. This type owns that fixed-size
/// array so generated PDA helpers can return one compact value, and callers can
/// pass [`PdaSigner::as_signer`] into `invoke_signed` APIs without manually
/// assembling temporary seed storage.
#[derive(Clone, Debug)]
#[must_use]
pub struct PdaSigner<'a, const SEEDS: usize> {
	seeds: [Seed<'a>; SEEDS],
}

impl<'a, const SEEDS: usize> PdaSigner<'a, SEEDS> {
	/// Build a PDA signer from byte-slice seeds.
	#[inline(always)]
	pub fn from_slices(seeds: [&'a [u8]; SEEDS]) -> Self {
		Self {
			seeds: seeds.map(Seed::from),
		}
	}

	/// Build a PDA signer from Pinocchio seed values.
	#[inline(always)]
	pub const fn from_seed_array(seeds: [Seed<'a>; SEEDS]) -> Self {
		Self { seeds }
	}

	/// Return the owned seed array.
	#[inline(always)]
	pub const fn as_seeds(&self) -> &[Seed<'a>; SEEDS] {
		&self.seeds
	}

	/// Borrow these seeds as a Pinocchio CPI signer.
	#[inline(always)]
	pub fn as_signer(&self) -> Signer<'a, '_> {
		Signer::from(&self.seeds)
	}
}

impl<'a, const SEEDS: usize> From<[Seed<'a>; SEEDS]> for PdaSigner<'a, SEEDS> {
	#[inline(always)]
	fn from(seeds: [Seed<'a>; SEEDS]) -> Self {
		Self::from_seed_array(seeds)
	}
}

impl<'a, const SEEDS: usize> From<[&'a [u8]; SEEDS]> for PdaSigner<'a, SEEDS> {
	#[inline(always)]
	fn from(seeds: [&'a [u8]; SEEDS]) -> Self {
		Self::from_slices(seeds)
	}
}

/// Allocates a PDA account with a caller-provided bump.
///
/// Two paths are taken depending on whether the target account already has
/// lamports:
///
/// - **Zero balance** -- a single `CreateAccount` CPI is issued.
/// - **Non-zero balance** -- a `Transfer` (to top up rent), `Allocate`, and
///   `Assign` are issued separately. This covers the case where the account was
///   pre-funded (e.g. by a previous failed transaction).
///
/// <!-- {=pinaPdaSeedContract|trim|linePrefix:"/// ":true} -->
/// Seed-based APIs require deterministic seed ordering.
///
/// Program IDs must stay consistent across derivation and verification.
///
/// When a bump is required, prefer canonical bump derivation.
///
/// Use explicit bumps when needed.<!-- {/pinaPdaSeedContract} -->
///
/// # Errors
///
/// Returns seed-validation errors, rent sysvar access errors, and any
/// system-program CPI failure from `CreateAccount`, `Transfer`, `Allocate`, or
/// `Assign`. `invoke_signed` returns `InvalidArgument` when 16 additional
/// signers would exceed the runtime's 16-signer limit after adding the target
/// PDA signer.
///
/// # Examples
///
/// ```ignore
/// let seeds: &[&[u8]] = &[b"vault"];
/// AllocateAccountWithBump {
/// 	account: vault_account,
/// 	payer,
/// 	space: 64,
/// 	owner: &program_id,
/// 	seeds,
/// 	bump,
/// }
/// .invoke()?;
/// ```
#[derive(Clone, Copy, Debug)]
#[must_use = "account allocation has no effect until invoke or invoke_signed is called"]
pub struct AllocateAccountWithBump<'account, 'address, 'seeds, 'seed> {
	/// PDA account to allocate and assign.
	pub account: &'account AccountView,

	/// Funding account that pays any required rent-exempt balance.
	pub payer: &'account AccountView,

	/// Number of account-data bytes to allocate.
	pub space: u64,

	/// Program that owns the PDA and derives it from `seeds` and `bump`.
	pub owner: &'address Address,

	/// PDA seeds without the bump.
	pub seeds: &'seeds [&'seed [u8]],

	/// PDA bump to validate and append to `seeds`.
	pub bump: u8,
}

impl AllocateAccountWithBump<'_, '_, '_, '_> {
	/// Allocates the account with its derived PDA signer.
	#[inline(always)]
	pub fn invoke(&self) -> ProgramResult {
		self.invoke_signed(&[])
	}

	/// Allocates the account with its derived PDA signer and additional signers.
	///
	/// The target account's signer is always included automatically. Pass only
	/// other signers required by the CPI, such as seeds for a PDA payer.
	#[inline(always)]
	pub fn invoke_signed(&self, signers: &[Signer<'_, '_>]) -> ProgramResult {
		self.invoke_signed_inner(signers, None)
	}

	#[cfg(test)]
	#[inline(always)]
	fn invoke_signed_with_rent(&self, signers: &[Signer<'_, '_>], rent: Rent) -> ProgramResult {
		self.invoke_signed_inner(signers, Some(rent))
	}

	#[inline(always)]
	fn invoke_signed_inner(&self, signers: &[Signer<'_, '_>], rent: Option<Rent>) -> ProgramResult {
		const MAX_CPI_SIGNERS: usize = 16;

		if signers.len() >= MAX_CPI_SIGNERS {
			return Err(ProgramError::InvalidArgument);
		}

		let bump_array = [self.bump];
		let combined_seeds = combine_seeds_with_bump(self.seeds, &bump_array)?;
		let mut derivation_seeds: [&[u8]; MAX_SEEDS] = [&[]; MAX_SEEDS];
		derivation_seeds[..self.seeds.len()].copy_from_slice(self.seeds);
		derivation_seeds[self.seeds.len()] = bump_array.as_slice();
		let expected_address =
			crate::create_program_address(&derivation_seeds[..=self.seeds.len()], self.owner)?;
		if self.account.address() != &expected_address {
			return Err(ProgramError::InvalidSeeds);
		}

		let target_signer = Signer::from(&combined_seeds[..=self.seeds.len()]);
		let empty_seeds: [Seed<'_>; 0] = [];
		let empty_signer = Signer::from(&empty_seeds);
		let mut all_signers: [Signer<'_, '_>; MAX_CPI_SIGNERS] =
			core::array::from_fn(|_| empty_signer.clone());
		all_signers[0] = target_signer;
		for (destination, signer) in all_signers[1..].iter_mut().zip(signers) {
			*destination = signer.clone();
		}
		let all_signers = &all_signers[..=signers.len()];

		let space = usize::try_from(self.space).map_err(|_| ProgramError::InvalidArgument)?;
		let rent = if let Some(rent) = rent {
			rent
		} else {
			Rent::get()?
		};
		if self.account.lamports() == 0 {
			SystemCreateAccount {
				from: self.payer,
				to: self.account,
				lamports: rent.try_minimum_balance(space)?,
				space: self.space,
				owner: self.owner,
			}
			.invoke_signed(all_signers)?;

			return Ok(());
		}

		let rent_exempt_balance = rent
			.try_minimum_balance(space)?
			.saturating_sub(self.account.lamports());

		if rent_exempt_balance > 0 {
			SystemTransfer {
				from: self.payer,
				to: self.account,
				lamports: rent_exempt_balance,
			}
			.invoke_signed(all_signers)?;
		}

		SystemAllocate {
			account: self.account,
			space: self.space,
		}
		.invoke_signed(all_signers)?;

		SystemAssign {
			account: self.account,
			owner: self.owner,
		}
		.invoke_signed(all_signers)
	}
}

/// Maximum number of bytes an account may grow by in a single instruction.
///
/// This limit is enforced by the Solana runtime. Attempting to grow an account
/// by more than this amount returns `ProgramError::InvalidRealloc`.
#[cfg(feature = "account-resize")]
pub const MAX_PERMITTED_DATA_INCREASE: usize = pinocchio::account::MAX_PERMITTED_DATA_INCREASE;

/// Lamport movement required to make a resized account rent-exempt.
#[cfg(feature = "account-resize")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RentAdjustment {
	/// No lamport movement is appropriate for this size transition.
	None,

	/// Transfer lamports from the rent account into the resized account.
	Fund {
		/// Missing lamports required by the target rent minimum.
		lamports: u64,
	},

	/// Return lamports above the target rent minimum to the rent account.
	Refund {
		/// Excess lamports that can be returned safely.
		lamports: u64,
	},
}

/// Pure account-reallocation plan computed before any balance or data mutation.
#[cfg(feature = "account-resize")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReallocPlan {
	/// Required account-data length after reallocation.
	pub target_size: usize,

	/// Rent transfer required for the target length.
	pub adjustment: RentAdjustment,
}

#[cfg(feature = "account-resize")]
impl ReallocPlan {
	/// Plan a resize using the already-computed rent minimum for `target_size`.
	///
	/// This function is pure: an error never changes account data or balances,
	/// and a successful plan can be inspected before any CPI or direct lamport
	/// mutation occurs.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidRealloc`] when one growth request exceeds
	/// [`MAX_PERMITTED_DATA_INCREASE`]. Shrinking and unchanged sizes are always
	/// accepted.
	pub fn try_new(
		current_size: usize,
		target_size: usize,
		current_lamports: u64,
		target_minimum_balance: u64,
	) -> Result<Self, ProgramError> {
		validate_realloc_size(current_size, target_size)?;

		Ok(Self::from_valid_size(
			current_size,
			target_size,
			current_lamports,
			target_minimum_balance,
		))
	}

	#[inline(always)]
	fn from_valid_size(
		current_size: usize,
		target_size: usize,
		current_lamports: u64,
		target_minimum_balance: u64,
	) -> Self {
		let adjustment = match target_size.cmp(&current_size) {
			core::cmp::Ordering::Greater => {
				let lamports = target_minimum_balance.saturating_sub(current_lamports);

				if lamports == 0 {
					RentAdjustment::None
				} else {
					RentAdjustment::Fund { lamports }
				}
			}
			core::cmp::Ordering::Less => {
				let lamports = current_lamports.saturating_sub(target_minimum_balance);

				if lamports == 0 {
					RentAdjustment::None
				} else {
					RentAdjustment::Refund { lamports }
				}
			}
			core::cmp::Ordering::Equal => RentAdjustment::None,
		};

		Self {
			target_size,
			adjustment,
		}
	}
}

/// Validates one runtime reallocation step against Solana's growth limit.
#[cfg(feature = "account-resize")]
#[inline(always)]
fn validate_realloc_size(current_size: usize, target_size: usize) -> ProgramResult {
	if target_size
		.checked_sub(current_size)
		.is_some_and(|growth| growth > MAX_PERMITTED_DATA_INCREASE)
	{
		return Err(ProgramError::InvalidRealloc);
	}

	Ok(())
}

/// Reallocates an account and adjusts its rent-exempt balance.
///
/// <!-- {=accountReallocationContract|trim|linePrefix:"/// ":true} -->
/// `target_size` is the exact physical allocation after the call. When the account grows, `rent_account` funds the missing rent before Pina resizes the data. When the account shrinks, `rent_account` receives the excess rent. New bytes are zero-initialized by the Solana runtime.
///
/// The Solana runtime limits account growth to `MAX_PERMITTED_DATA_INCREASE` bytes per top-level instruction. Pina rejects a single larger increase before it moves rent. Pinocchio does not expose the original serialized length, so cumulative growth from several reallocations in one instruction can still fail during `AccountView::resize`.
///
/// Propagate reallocation errors. If a later resize or callback fails after rent moves, Solana restores the account only when the instruction returns that error.<!-- {/accountReallocationContract} -->
///
/// # Examples
///
/// <!-- {=accountReallocationLowLevelExample|trim|linePrefix:"/// ":true} -->
/// Use `ReallocAccount` when the bytes do not use a compact Pina schema:
///
/// ```ignore
/// ReallocAccount {
/// 	account,
/// 	rent_account,
/// 	target_size,
/// 	program_id,
/// }
/// .invoke()?;
/// ```
///
/// Use `invoke_signed` when `rent_account` is a PDA that must sign the system transfer used for growth:
///
/// ```ignore
/// ReallocAccount {
/// 	account,
/// 	rent_account,
/// 	target_size,
/// 	program_id,
/// }
/// .invoke_signed(rent_account_signers)?;
/// ```
///
/// `ReallocAccountZeroed` has the same behavior and field names. Its name records the caller's intent that newly allocated bytes start at zero. The current Solana runtime zero-initializes new bytes for both builders.<!-- {/accountReallocationLowLevelExample} -->
///
/// # Errors
///
/// Returns `ProgramError::InvalidAccountData` if the account is not writable,
/// `ProgramError::InvalidAccountOwner` if the account is not owned by
/// `program_id`, and propagates any errors from rent sysvar access, lamport
/// transfer, or the runtime `resize` call.
#[cfg(feature = "account-resize")]
#[must_use = "account reallocation has no effect until invoke or invoke_signed is called"]
pub struct ReallocAccount<'account, 'rent_account, 'address> {
	/// Program-owned account whose data length and rent balance will change.
	pub account: &'account mut AccountView,

	/// Account that funds growth or receives excess rent after shrinking.
	pub rent_account: &'rent_account mut AccountView,

	/// Required account-data length after reallocation.
	pub target_size: usize,

	/// Executing program ID used to validate account ownership.
	pub program_id: &'address Address,
}

#[cfg(feature = "account-resize")]
impl ReallocAccount<'_, '_, '_> {
	/// Reallocates the account using transaction-level signatures.
	#[inline(always)]
	pub fn invoke(&mut self) -> ProgramResult {
		self.invoke_signed(&[])
	}

	/// Reallocates the account with PDA signer seeds for the rent account.
	///
	/// Signers are used only when growth requires a system transfer. Shrinking
	/// moves lamports directly from the program-owned account.
	#[inline(always)]
	pub fn invoke_signed(&mut self, signers: &[Signer<'_, '_>]) -> ProgramResult {
		realloc_account_inner(
			self.account,
			self.target_size,
			self.rent_account,
			self.program_id,
			signers,
		)
	}
}

/// Reallocates an account to `target_size` bytes with explicit zero-initialization.
///
/// This builder behaves identically to [`ReallocAccount`]. In the current
/// Solana runtime, new bytes are always zero-initialized regardless of which
/// variant is called. This builder exists for API symmetry with the runtime's
/// `realloc(new_len, zero_init)` parameter and to make zero-initialization
/// intent explicit at the call site.
///
/// <!-- {=accountReallocationContract|trim|linePrefix:"/// ":true} -->
/// `target_size` is the exact physical allocation after the call. When the account grows, `rent_account` funds the missing rent before Pina resizes the data. When the account shrinks, `rent_account` receives the excess rent. New bytes are zero-initialized by the Solana runtime.
///
/// The Solana runtime limits account growth to `MAX_PERMITTED_DATA_INCREASE` bytes per top-level instruction. Pina rejects a single larger increase before it moves rent. Pinocchio does not expose the original serialized length, so cumulative growth from several reallocations in one instruction can still fail during `AccountView::resize`.
///
/// Propagate reallocation errors. If a later resize or callback fails after rent moves, Solana restores the account only when the instruction returns that error.<!-- {/accountReallocationContract} -->
///
/// # Errors
///
/// Returns `ProgramError::InvalidAccountData` if the account is not writable,
/// `ProgramError::InvalidAccountOwner` if the account is not owned by
/// `program_id`, and propagates any errors from rent sysvar access, lamport
/// transfer, or the runtime `resize` call.
#[cfg(feature = "account-resize")]
#[must_use = "account reallocation has no effect until invoke or invoke_signed is called"]
pub struct ReallocAccountZeroed<'account, 'rent_account, 'address> {
	/// Program-owned account whose data length and rent balance will change.
	pub account: &'account mut AccountView,

	/// Account that funds growth or receives excess rent after shrinking.
	pub rent_account: &'rent_account mut AccountView,

	/// Required account-data length after reallocation.
	pub target_size: usize,

	/// Executing program ID used to validate account ownership.
	pub program_id: &'address Address,
}

#[cfg(feature = "account-resize")]
impl ReallocAccountZeroed<'_, '_, '_> {
	/// Reallocates the account using transaction-level signatures.
	#[inline(always)]
	pub fn invoke(&mut self) -> ProgramResult {
		self.invoke_signed(&[])
	}

	/// Reallocates the account with PDA signer seeds for the rent account.
	///
	/// Signers are used only when growth requires a system transfer. The Solana
	/// runtime zero-initializes every newly allocated byte.
	#[inline(always)]
	pub fn invoke_signed(&mut self, signers: &[Signer<'_, '_>]) -> ProgramResult {
		realloc_account_inner(
			self.account,
			self.target_size,
			self.rent_account,
			self.program_id,
			signers,
		)
	}
}

/// Rent-adjusts and resizes a validated compact account.
///
/// Unlike the raw reallocation builders, this API verifies the account's
/// current compact layout and constrains the target length to its declared
/// header and capacity bounds. Shrinking also validates the retained byte
/// slice, preventing active tail data from being truncated.
///
/// <!-- {=accountReallocationContract|trim|linePrefix:"/// ":true} -->
/// `target_size` is the exact physical allocation after the call. When the account grows, `rent_account` funds the missing rent before Pina resizes the data. When the account shrinks, `rent_account` receives the excess rent. New bytes are zero-initialized by the Solana runtime.
///
/// The Solana runtime limits account growth to `MAX_PERMITTED_DATA_INCREASE` bytes per top-level instruction. Pina rejects a single larger increase before it moves rent. Pinocchio does not expose the original serialized length, so cumulative growth from several reallocations in one instruction can still fail during `AccountView::resize`.
///
/// Propagate reallocation errors. If a later resize or callback fails after rent moves, Solana restores the account only when the instruction returns that error.<!-- {/accountReallocationContract} -->
///
/// # Examples
///
/// <!-- {=reallocCompactAccountExample|trim|linePrefix:"/// ":true} -->
/// `ReallocCompactAccount` is the lower-level compact builder. It changes the physical allocation immediately and does not edit or commit the compact view:
///
/// ```ignore
/// ReallocCompactAccount {
/// 	account,
/// 	rent_account,
/// 	target_size,
/// 	program_id,
/// }
/// .invoke::<Journal>()?;
/// ```
///
/// Call `invoke` before editing when the compact layout grows. To shrink, commit the shorter layout, drop its mutable data borrow, and then call `invoke`. Use `invoke_signed` instead when a PDA rent account must fund growth.<!-- {/reallocCompactAccountExample} -->
#[cfg(all(feature = "account-resize", feature = "compact"))]
#[must_use = "account reallocation has no effect until invoke or invoke_signed is called"]
pub struct ReallocCompactAccount<'account, 'rent_account, 'address> {
	/// Program-owned compact account to resize.
	pub account: &'account mut AccountView,
	/// Account that funds growth or receives excess rent after shrinkage.
	pub rent_account: &'rent_account mut AccountView,
	/// Required account-data length after reallocation.
	pub target_size: usize,
	/// Executing program ID used to validate ownership.
	pub program_id: &'address Address,
}

#[cfg(all(feature = "account-resize", feature = "compact"))]
impl ReallocCompactAccount<'_, '_, '_> {
	/// Validates and resizes the compact account.
	pub fn invoke<T: PinaCompactAccount>(&mut self) -> ProgramResult {
		self.invoke_signed::<T>(&[])
	}

	/// Validates and resizes the compact account with rent-account signer seeds.
	pub fn invoke_signed<T: PinaCompactAccount>(
		&mut self,
		signers: &[Signer<'_, '_>],
	) -> ProgramResult {
		self.account.assert_compact_type::<T>(self.program_id)?;
		T::validate_size(self.target_size)?;
		if self.target_size < self.account.data_len() {
			let data = self.account.try_borrow()?;
			T::validate_account_data(&data[..self.target_size])?;
		}

		realloc_account_inner(
			self.account,
			self.target_size,
			self.rent_account,
			self.program_id,
			signers,
		)
	}
}

/// Updates a compact account and enforces its exact committed size.
///
/// This builder handles the order required by compact zero-copy storage. It
/// grows the account before `edit_account` runs, verifies the callback's commit,
/// drops the mutable data borrow, and then shrinks the account. The rent account
/// funds growth and receives refunds after shrinkage.
///
/// <!-- {=accountReallocationContract|trim|linePrefix:"/// ":true} -->
/// `target_size` is the exact physical allocation after the call. When the account grows, `rent_account` funds the missing rent before Pina resizes the data. When the account shrinks, `rent_account` receives the excess rent. New bytes are zero-initialized by the Solana runtime.
///
/// The Solana runtime limits account growth to `MAX_PERMITTED_DATA_INCREASE` bytes per top-level instruction. Pina rejects a single larger increase before it moves rent. Pinocchio does not expose the original serialized length, so cumulative growth from several reallocations in one instruction can still fail during `AccountView::resize`.
///
/// Propagate reallocation errors. If a later resize or callback fails after rent moves, Solana restores the account only when the instruction returns that error.<!-- {/accountReallocationContract} -->
///
/// # Examples
///
/// <!-- {=resizeCompactAccountExample|trim|linePrefix:"/// ":true} -->
/// Use `ResizeCompactAccount` for an exact compact update. The callback receives the account bytes under one mutable borrow. Load the generated mutable view inside the callback, stage every tail, and call `commit()` before returning:
///
/// ```ignore
/// let target_size = Journal::projected_bytes(entries.len(), markers.len())?;
///
/// ResizeCompactAccount {
/// 	account,
/// 	rent_account,
/// 	target_size,
/// 	program_id,
/// }
/// .invoke::<Journal, _>(|data| {
/// 	let mut journal = Journal::try_from_bytes_mut(data)?;
/// 	journal
/// 		.set_entries(entries)
/// 		.map_err(|_| ProgramError::InvalidAccountData)?;
/// 	journal
/// 		.set_markers(markers)
/// 		.map_err(|_| ProgramError::InvalidAccountData)?;
/// 	journal.commit().map_err(|_| ProgramError::InvalidAccountData)?;
///
/// 	Ok(())
/// })?;
/// ```
///
/// The builder grows before the callback. After the callback, it verifies that the committed logical size equals `target_size`, drops the data borrow, and then shrinks and refunds rent when needed. Use `invoke_signed` with the same callback when `rent_account` is a PDA that funds growth.<!-- {/resizeCompactAccountExample} -->
#[cfg(all(feature = "account-resize", feature = "compact"))]
#[must_use = "compact account updates have no effect until invoke or invoke_signed is called"]
pub struct ResizeCompactAccount<'account, 'rent_account, 'address> {
	/// Program-owned compact account to update and resize.
	pub account: &'account mut AccountView,
	/// Account that funds growth or receives excess rent after shrinkage.
	pub rent_account: &'rent_account mut AccountView,
	/// Exact account-data length required by the staged compact layout.
	pub target_size: usize,
	/// Executing program ID used to validate ownership.
	pub program_id: &'address Address,
}

#[cfg(all(feature = "account-resize", feature = "compact"))]
impl ResizeCompactAccount<'_, '_, '_> {
	/// Updates a compact account and resizes it to the exact committed size.
	///
	/// Growth happens before `edit_account` runs so larger tails fit in the
	/// account. Use the supplied bytes to load the concrete mutable view, stage
	/// its tails, and call `commit()`. Shrinkage happens after the closure returns
	/// and Pina drops the mutable data borrow.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidAccountData`] when `target_size` is invalid,
	/// the compact account fails validation, or the committed size differs from
	/// `target_size`. It also propagates errors from `edit_account`, rent
	/// adjustment, and resizing.
	///
	/// Propagate errors from this method. Growth can move rent before the closure
	/// runs, and Solana restores those changes only when the instruction fails.
	pub fn invoke<T, R>(
		&mut self,
		edit_account: impl FnOnce(&mut [u8]) -> Result<R, ProgramError>,
	) -> Result<R, ProgramError>
	where
		T: PinaCompactAccount,
	{
		self.invoke_signed::<T, R>(&[], edit_account)
	}

	/// Updates a compact account with signer seeds for the rent account.
	///
	/// Signers are used only when growth requires a system transfer. Shrinkage
	/// returns excess rent directly to `rent_account` after the staged layout is
	/// committed.
	///
	/// # Errors
	///
	/// Returns the same errors as [`Self::invoke`] and propagates signer errors
	/// from the system transfer used to fund growth.
	pub fn invoke_signed<T, R>(
		&mut self,
		signers: &[Signer<'_, '_>],
		edit_account: impl FnOnce(&mut [u8]) -> Result<R, ProgramError>,
	) -> Result<R, ProgramError>
	where
		T: PinaCompactAccount,
	{
		self.invoke_signed_inner::<T, R>(signers, edit_account, None)
	}

	#[cfg(test)]
	fn invoke_signed_with_rent<T, R>(
		&mut self,
		signers: &[Signer<'_, '_>],
		edit_account: impl FnOnce(&mut [u8]) -> Result<R, ProgramError>,
		rent: Rent,
	) -> Result<R, ProgramError>
	where
		T: PinaCompactAccount,
	{
		self.invoke_signed_inner::<T, R>(signers, edit_account, Some(rent))
	}

	fn invoke_signed_inner<T, R>(
		&mut self,
		signers: &[Signer<'_, '_>],
		edit_account: impl FnOnce(&mut [u8]) -> Result<R, ProgramError>,
		rent: Option<Rent>,
	) -> Result<R, ProgramError>
	where
		T: PinaCompactAccount,
	{
		self.account.assert_compact_type::<T>(self.program_id)?;
		T::validate_size(self.target_size)?;

		let allocated_size = self.account.data_len();

		if self.target_size > allocated_size {
			realloc_account_inner_with_rent(
				self.account,
				self.target_size,
				self.rent_account,
				self.program_id,
				signers,
				rent,
			)?;
		}

		let result = {
			let mut data = self.account.try_borrow_mut()?;
			let result = edit_account(&mut data)?;
			let account = T::try_from_bytes(&data)?;

			if T::encoded_size(&account) != self.target_size {
				return Err(ProgramError::InvalidAccountData);
			}

			result
		};

		if self.target_size < allocated_size {
			realloc_account_inner_with_rent(
				self.account,
				self.target_size,
				self.rent_account,
				self.program_id,
				signers,
				rent,
			)?;
		}

		Ok(result)
	}
}

/// Shared implementation for [`ReallocAccount`] and [`ReallocAccountZeroed`].
///
/// Validates the account, computes the rent delta, performs the lamport
/// transfer, and resizes the account data.
#[cfg(feature = "account-resize")]
#[inline(always)]
fn realloc_account_inner(
	account: &mut AccountView,
	target_size: usize,
	rent_account: &mut AccountView,
	program_id: &Address,
	signers: &[Signer<'_, '_>],
) -> ProgramResult {
	realloc_account_inner_with_rent(
		account,
		target_size,
		rent_account,
		program_id,
		signers,
		None,
	)
}

#[cfg(feature = "account-resize")]
#[inline(always)]
fn realloc_account_inner_with_rent(
	account: &mut AccountView,
	target_size: usize,
	rent_account: &mut AccountView,
	program_id: &Address,
	signers: &[Signer<'_, '_>],
	rent: Option<Rent>,
) -> ProgramResult {
	use crate::AccountInfoValidation;

	// Validate the account is writable and owned by the program.
	account.assert_writable()?.assert_owner(program_id)?;

	let current_size = account.data_len();

	// Early return when the size is unchanged.
	if target_size == current_size {
		return Ok(());
	}

	// `resize` would reject either condition after rent movement. The original
	// serialized length is not exposed, so cumulative growth is still checked by
	// the runtime and its error must be propagated by callers.
	account.check_borrow_mut()?;
	validate_realloc_size(current_size, target_size)?;

	let rent = if let Some(rent) = rent {
		rent
	} else {
		Rent::get()?
	};
	let target_minimum_balance = rent.try_minimum_balance(target_size)?;
	let current_lamports = account.lamports();
	let plan = ReallocPlan::from_valid_size(
		current_size,
		target_size,
		current_lamports,
		target_minimum_balance,
	);

	match plan.adjustment {
		RentAdjustment::Fund { lamports } => {
			SystemTransfer {
				from: rent_account,
				to: account,
				lamports,
			}
			.invoke_signed(signers)?;
		}
		RentAdjustment::Refund { lamports } => {
			account.send(lamports, rent_account)?;
		}
		RentAdjustment::None => {}
	}

	// Resize the account data. The runtime zero-initializes new bytes.
	account.resize(plan.target_size)
}

/// Closes an account and returns its remaining lamports to a recipient.
///
/// Use this builder when stale data reuse is not part of the account's threat
/// model. Use [`CloseAccountZeroed`] to clear the account bytes first.
///
/// Closing is a direct mutation of program-owned accounts, not a CPI, so this
/// builder intentionally has no `invoke_signed` variant.
///
/// <!-- {=pinaPublicResultContract|trim|linePrefix:"/// ":true} -->
/// All APIs in this section are designed for on-chain determinism.
///
/// They return `ProgramError` values for caller-side propagation with `?`.
///
/// No panics needed.<!-- {/pinaPublicResultContract} -->
///
/// # Errors
///
/// Returns errors from lamport transfer or account close operations.
///
/// # Examples
///
/// ```ignore
/// // Close the escrow account and return rent to the authority:
/// CloseAccount {
/// 	account: escrow_account,
/// 	recipient: authority,
/// }
/// .invoke()?;
/// ```
#[must_use = "account closure has no effect until invoke is called"]
pub struct CloseAccount<'account, 'recipient> {
	/// Program-owned account to close.
	pub account: &'account mut AccountView,

	/// Writable account that receives the closed account's lamports.
	pub recipient: &'recipient mut AccountView,
}

impl CloseAccount<'_, '_> {
	/// Transfers the account's lamports to the recipient and closes it.
	#[inline(always)]
	pub fn invoke(&mut self) -> ProgramResult {
		self.account.close_with_recipient(self.recipient)
	}
}

/// Closes an account after zeroing its current data bytes in-place.
///
/// This builder clears the raw account data before transferring the remaining
/// lamports and closing the account. It does not implicitly reallocate the
/// account, even when the `account-resize` feature is enabled.
///
/// <!-- {=pinaPublicResultContract|trim|linePrefix:"/// ":true} -->
/// All APIs in this section are designed for on-chain determinism.
///
/// They return `ProgramError` values for caller-side propagation with `?`.
///
/// No panics needed.<!-- {/pinaPublicResultContract} -->
///
/// # Errors
///
/// Returns errors from account borrowing, lamport transfer, or account close
/// operations.
///
/// # Examples
///
/// ```ignore
/// // Zero the raw account bytes, then close the account and return rent:
/// CloseAccountZeroed {
/// 	account: escrow_account,
/// 	recipient: authority,
/// }
/// .invoke()?;
/// ```
#[must_use = "account closure has no effect until invoke is called"]
pub struct CloseAccountZeroed<'account, 'recipient> {
	/// Program-owned account whose bytes will be cleared before closing.
	pub account: &'account mut AccountView,

	/// Writable account that receives the closed account's lamports.
	pub recipient: &'recipient mut AccountView,
}

impl CloseAccountZeroed<'_, '_> {
	/// Clears the account data, transfers its lamports, and closes it.
	#[inline(always)]
	pub fn invoke(&mut self) -> ProgramResult {
		self.account.close_account_zeroed(self.recipient)
	}
}

/// Typed handle for passing validated accounts into CPI builders.
///
/// This is a lightweight, allocator-free wrapper around `&AccountView` plus the
/// writable bit the callee should observe. It is intentionally separate from
/// the raw `AccountView` so callers can build typed CPI account structs without
/// immediately reaching for unchecked runtime APIs.
///
/// This prototype keeps Pina's current architecture constraints intact:
///
/// - no heap allocation in the on-chain CPI path
/// - no `unsafe` in the wrapper layer
/// - const-generic account counts instead of `Vec`
/// - checked `pinocchio::cpi::invoke_signed` as the execution path for now
#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct CpiHandle<'a> {
	view: &'a AccountView,
	writable: bool,
	signer: bool,
}

impl<'a> CpiHandle<'a> {
	/// Construct a read-only CPI handle.
	#[inline(always)]
	pub const fn readonly(view: &'a AccountView) -> Self {
		Self {
			view,
			writable: false,
			signer: false,
		}
	}

	/// Construct a read-only handle that the callee requires to sign.
	///
	/// The signer bit describes the target instruction schema, not the outer
	/// instruction's current privileges. This permits [`Signer`] seeds passed to
	/// [`CpiContext::invoke_signed`] to satisfy PDA signer requirements.
	#[inline(always)]
	pub const fn readonly_signer(view: &'a AccountView) -> Self {
		Self {
			view,
			writable: false,
			signer: true,
		}
	}

	/// Construct a writable CPI handle.
	///
	/// Returns `InvalidAccountData` when the source account was not declared
	/// writable in the current instruction.
	#[inline(always)]
	pub fn writable(view: &'a AccountView) -> Result<Self, ProgramError> {
		if !view.is_writable() {
			return Err(ProgramError::InvalidAccountData);
		}

		Ok(Self {
			view,
			writable: true,
			signer: false,
		})
	}

	/// Construct a writable handle that the callee requires to sign.
	///
	/// Returns `InvalidAccountData` when the source account was not declared
	/// writable in the current instruction. The runtime verifies the requested
	/// signer privilege during invocation or derives it from supplied PDA seeds.
	#[inline(always)]
	pub fn writable_signer(view: &'a AccountView) -> Result<Self, ProgramError> {
		if !view.is_writable() {
			return Err(ProgramError::InvalidAccountData);
		}

		Ok(Self {
			view,
			writable: true,
			signer: true,
		})
	}

	/// Return the account address with the original borrow lifetime.
	#[inline(always)]
	pub fn address(&self) -> &'a Address {
		self.view.address()
	}

	/// Whether this handle should be passed to the callee as writable.
	#[inline(always)]
	pub const fn is_writable(&self) -> bool {
		self.writable
	}

	/// Whether the target instruction requires this account to sign.
	#[inline(always)]
	pub const fn is_signer(&self) -> bool {
		self.signer
	}

	#[inline(always)]
	pub(crate) fn instruction_account(self) -> InstructionAccount<'a> {
		InstructionAccount::new(self.address(), self.is_writable(), self.is_signer())
	}

	#[inline(always)]
	fn account_view(self) -> &'a AccountView {
		self.view
	}
}

/// Marker trait for a known CPI target program.
///
/// Generated CPI modules should emit a zero-sized program marker that
/// implements this trait. Pair it with [`Program`] to validate the executable
/// account once, before constructing Pinocchio-style instruction builders.
pub trait CpiProgramId {
	/// Canonical program address.
	const ID: Address;
}

/// A validated CPI target program account.
///
/// This wrapper makes program-ID verification structural for generated CPI
/// builders. It stores the original account view so callers can keep the
/// executable account in the same typed accounts struct as the rest of the CPI
/// inputs.
#[must_use]
pub struct Program<'a, T: CpiProgramId> {
	account: &'a AccountView,
	_marker: core::marker::PhantomData<T>,
}

impl<T: CpiProgramId> Clone for Program<'_, T> {
	#[inline(always)]
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: CpiProgramId> Copy for Program<'_, T> {}

impl<T: CpiProgramId> core::fmt::Debug for Program<'_, T> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("Program")
			.field("account", &self.account)
			.finish_non_exhaustive()
	}
}

impl<'a, T: CpiProgramId> Program<'a, T> {
	/// Validate `account` as the expected executable program.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidAccountData`] when the account is not
	/// executable or does not have the expected program address.
	#[inline(always)]
	pub fn try_new(account: &'a AccountView) -> Result<Self, ProgramError> {
		account.assert_program(&T::ID)?;

		Ok(Self {
			account,
			_marker: core::marker::PhantomData,
		})
	}

	/// The validated program account.
	#[inline(always)]
	pub const fn account(&self) -> &'a AccountView {
		self.account
	}

	/// The validated program address.
	#[inline(always)]
	pub fn address(&self) -> &'a Address {
		self.account.address()
	}
}

/// Convert a typed CPI accounts struct into a fixed-size array of handles.
///
/// This is the no-allocation counterpart to Anchor lang-v2's `ToCpiAccounts`.
/// The const generic keeps the final account count explicit at compile time so
/// Pina can stay within its allocator-free on-chain boundary.
pub trait ToCpiAccounts<'a, const ACCOUNTS: usize> {
	/// Collect the handles in the exact order expected by the callee
	/// instruction.
	fn to_cpi_handles(&self) -> [CpiHandle<'a>; ACCOUNTS];
}

impl<'a, const ACCOUNTS: usize> ToCpiAccounts<'a, ACCOUNTS> for [CpiHandle<'a>; ACCOUNTS] {
	#[inline(always)]
	fn to_cpi_handles(&self) -> [CpiHandle<'a>; ACCOUNTS] {
		*self
	}
}

/// Minimal typed CPI context built around [`CpiHandle`] and const generics.
///
/// This prototype intentionally omits heap-backed remaining-account lists.
/// Callers should include every required account in the typed accounts struct
/// for now. A future cursor-based account runtime can extend this with richer
/// remaining-account support while preserving `no_std` compatibility.
#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct CpiContext<'a, P, T, const ACCOUNTS: usize>
where
	P: CpiProgramId,
	T: ToCpiAccounts<'a, ACCOUNTS>,
{
	/// Typed account set in the exact order expected by the target program.
	pub accounts: T,

	/// Validated executable account for the target program.
	pub program: Program<'a, P>,
}

impl<'a, P, T, const ACCOUNTS: usize> CpiContext<'a, P, T, ACCOUNTS>
where
	P: CpiProgramId,
	T: ToCpiAccounts<'a, ACCOUNTS>,
{
	/// Builds a typed CPI context from a validated program and account set.
	#[inline(always)]
	pub const fn new(program: Program<'a, P>, accounts: T) -> Self {
		Self { accounts, program }
	}

	/// Invokes the CPI using transaction-level signatures.
	#[inline(always)]
	pub fn invoke(&self, data: &[u8]) -> ProgramResult {
		self.invoke_signed(data, &[])
	}

	/// Invokes the CPI with additional PDA signer seeds.
	///
	/// Both invocation paths use Pinocchio's checked static-array implementation.
	/// This keeps the context allocator-free and avoids unchecked invocation until
	/// Pina can prove stronger duplicate-account and aliasing invariants.
	#[inline(always)]
	pub fn invoke_signed(&self, data: &[u8], signers: &[Signer<'_, '_>]) -> ProgramResult {
		let handles = self.accounts.to_cpi_handles();
		let instruction_accounts = handles.map(CpiHandle::instruction_account);
		let account_views = handles.map(CpiHandle::account_view);
		let instruction = InstructionView {
			program_id: self.program.address(),
			data,
			accounts: &instruction_accounts,
		};

		pinocchio::cpi::invoke_signed::<ACCOUNTS, _>(&instruction, &account_views, signers)
	}
}

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
	use pinocchio::account::NOT_BORROWED;
	use pinocchio::account::RuntimeAccount;

	use super::*;
	#[cfg(all(feature = "account-resize", feature = "compact"))]
	use crate::AsCompactAccount;
	use crate::HasDiscriminator;
	use crate::LayoutKind;
	use crate::ZeroPodError;
	use crate::ZeroPodFixed;
	use crate::ZeroPodSchema;

	#[cfg(all(feature = "account-resize", feature = "compact"))]
	mod compact_cpi_state {
		include!(concat!(
			env!("CARGO_MANIFEST_DIR"),
			"/tests/support/compact_cpi_state.rs"
		));
	}
	#[cfg(all(feature = "account-resize", feature = "compact"))]
	use compact_cpi_state::TestCompactState;

	struct TestState;

	impl ZeroPodSchema for TestState {
		const LAYOUT: LayoutKind = LayoutKind::Fixed;
	}

	impl ZeroPodFixed for TestState {
		type Zc = [u8; 1];

		const SIZE: usize = 1;

		fn from_bytes(data: &[u8]) -> Result<&Self::Zc, ZeroPodError> {
			data.first_chunk().ok_or(ZeroPodError::BufferTooSmall)
		}

		fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, ZeroPodError> {
			data.first_chunk_mut().ok_or(ZeroPodError::BufferTooSmall)
		}

		fn validate(data: &[u8]) -> Result<(), ZeroPodError> {
			Self::from_bytes(data).map(|_| ())
		}
	}

	impl PinaAccount for TestState {}

	impl HasDiscriminator for TestState {
		type Type = u8;

		const VALUE: u8 = 7;
	}

	#[repr(C)]
	struct TestAccount<const N: usize> {
		header: RuntimeAccount,
		data: [u8; N],
	}

	impl<const N: usize> TestAccount<N> {
		fn new(address: Address, owner: Address, lamports: u64, data_len: usize) -> Self {
			assert!(data_len <= N);

			Self {
				header: RuntimeAccount {
					borrow_state: NOT_BORROWED,
					is_signer: 1,
					is_writable: 1,
					executable: 0,
					padding: (data_len as u32).to_le_bytes(),
					address,
					owner,
					lamports,
					data_len: data_len as u64,
				},
				data: [0; N],
			}
		}

		fn view(&mut self) -> AccountView {
			unsafe { AccountView::new_unchecked(core::ptr::addr_of_mut!(self.header)) }
		}
	}

	fn test_rent() -> Rent {
		Rent::from_bytes(&1u64.to_le_bytes()).unwrap_or_else(|error| panic!("test rent: {error:?}"))
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn realloc_plan_covers_unchanged_growth_and_shrinkage() {
		assert_eq!(
			ReallocPlan::try_new(8, 8, 10, 20),
			Ok(ReallocPlan {
				target_size: 8,
				adjustment: RentAdjustment::None,
			})
		);
		assert_eq!(
			ReallocPlan::try_new(8, 16, 10, 25),
			Ok(ReallocPlan {
				target_size: 16,
				adjustment: RentAdjustment::Fund { lamports: 15 },
			})
		);
		assert_eq!(
			ReallocPlan::try_new(8, 16, 25, 25),
			Ok(ReallocPlan {
				target_size: 16,
				adjustment: RentAdjustment::None,
			})
		);
		assert_eq!(
			ReallocPlan::try_new(16, 8, 25, 10),
			Ok(ReallocPlan {
				target_size: 8,
				adjustment: RentAdjustment::Refund { lamports: 15 },
			})
		);
		assert_eq!(
			ReallocPlan::try_new(16, 8, 10, 10),
			Ok(ReallocPlan {
				target_size: 8,
				adjustment: RentAdjustment::None,
			})
		);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn realloc_plan_rejects_oversized_growth() {
		let target_size = MAX_PERMITTED_DATA_INCREASE + 1;

		assert_eq!(
			ReallocPlan::try_new(0, target_size, 0, 0),
			Err(ProgramError::InvalidRealloc)
		);
	}

	#[test]
	fn cpi_handle_arrays_are_typed_account_sets() {
		let owner = Address::new_from_array([9; 32]);
		let mut stored = TestAccount::<0>::new(Address::new_from_array([1; 32]), owner, 1, 0);
		let view = stored.view();
		let accounts = [CpiHandle::readonly_signer(&view)];
		let handles = accounts.to_cpi_handles();

		assert_eq!(handles[0].address(), view.address());
		assert!(!handles[0].is_writable());
		assert!(handles[0].is_signer());
	}

	#[test]
	fn create_account_executes_with_calculated_rent() {
		let owner = Address::new_from_array([9; 32]);
		let mut stored_from = TestAccount::<0>::new(Address::new_from_array([1; 32]), owner, 1, 0);
		let mut stored_to = TestAccount::<0>::new(Address::new_from_array([2; 32]), owner, 0, 0);
		let from = stored_from.view();
		let to = stored_to.view();

		CreateAccount {
			from: &from,
			to: &to,
			space: 8,
			owner: &owner,
		}
		.invoke_signed_with_rent(&[], test_rent(), 8)
		.unwrap_or_else(|error| panic!("create account: {error:?}"));
	}

	#[test]
	fn typed_and_raw_pda_builders_execute_with_calculated_rent() {
		let owner = Address::new_from_array([9; 32]);
		let seeds: &[&[u8]] = &[b"state"];
		let (address, bump) = crate::try_find_program_address(seeds, &owner)
			.unwrap_or_else(|| panic!("derive test address"));
		let mut stored_payer = TestAccount::<0>::new(Address::new_from_array([1; 32]), owner, 1, 0);
		let payer = stored_payer.view();
		let rent = test_rent();

		let mut stored_typed = TestAccount::<32>::new(address, owner, 0, TestState::SIZE);
		let mut typed = stored_typed.view();
		let result = CreateProgramAccount {
			account: &mut typed,
			payer: &payer,
			owner: &owner,
			seeds,
		}
		.invoke_signed_with_rent::<TestState>(&[], rent)
		.unwrap_or_else(|error| panic!("create typed PDA: {error:?}"));
		assert_eq!(result, (address, bump));
		assert_eq!(stored_typed.data[0], TestState::VALUE);
		<TestState as ZeroPodFixed>::validate(&stored_typed.data[..TestState::SIZE])
			.unwrap_or_else(|error| panic!("validate initialized state: {error:?}"));
		let state = TestState::from_bytes(&stored_typed.data[..TestState::SIZE])
			.unwrap_or_else(|error| panic!("read initialized state: {error:?}"));
		assert_eq!(state[0], TestState::VALUE);
		let mut copied_state = [0; TestState::SIZE];
		copied_state.copy_from_slice(&stored_typed.data[..TestState::SIZE]);
		let _state = TestState::from_bytes_mut(&mut copied_state)
			.unwrap_or_else(|error| panic!("mutably read initialized state: {error:?}"));

		let mut stored_explicit = TestAccount::<32>::new(address, owner, 0, TestState::SIZE);
		let mut explicit = stored_explicit.view();
		CreateProgramAccountWithBump {
			account: &mut explicit,
			payer: &payer,
			owner: &owner,
			seeds,
			bump,
		}
		.invoke_signed_with_rent::<TestState>(&[], rent)
		.unwrap_or_else(|error| panic!("create typed PDA with explicit bump: {error:?}"));
		assert_eq!(stored_explicit.data[0], TestState::VALUE);

		let mut stored_raw = TestAccount::<32>::new(address, owner, 0, 8);
		let raw = stored_raw.view();
		let result = AllocateAccount {
			account: &raw,
			payer: &payer,
			space: 8,
			owner: &owner,
			seeds,
		}
		.invoke_signed_with_rent(&[], rent)
		.unwrap_or_else(|error| panic!("allocate raw PDA: {error:?}"));
		assert_eq!(result, (address, bump));
	}

	#[cfg(all(feature = "account-resize", feature = "compact"))]
	#[test]
	fn compact_pda_builder_executes_with_calculated_rent() {
		let owner = Address::new_from_array([9; 32]);
		let seeds: &[&[u8]] = &[b"compact-state"];
		let (address, bump) = crate::try_find_program_address(seeds, &owner)
			.unwrap_or_else(|| panic!("derive compact test address"));
		let mut stored_payer = TestAccount::<0>::new(Address::new_from_array([1; 32]), owner, 1, 0);
		let payer = stored_payer.view();
		let initial_size = TestCompactState::HEADER_SIZE + 2 * size_of::<u64>();
		let mut stored_state = TestAccount::<64>::new(address, owner, 0, initial_size);
		let mut state = stored_state.view();

		let result = CreateCompactProgramAccount {
			account: &mut state,
			payer: &payer,
			owner: &owner,
			seeds,
			space: initial_size,
		}
		.invoke_signed_with_rent::<TestCompactState>(&[], test_rent())
		.unwrap_or_else(|error| panic!("create compact PDA: {error:?}"));

		assert_eq!(result, (address, bump));
		assert_eq!(state.data_len(), initial_size);
		let data = state
			.try_borrow()
			.unwrap_or_else(|error| panic!("borrow compact state: {error:?}"));
		let compact = TestCompactState::try_from_bytes(&data)
			.unwrap_or_else(|error| panic!("validate compact state: {error:?}"));
		assert!(TestCompactState::matches_discriminator(&data));
		assert_eq!(compact.value, 0);
		assert!(compact.items().is_empty());
		drop(compact);
		drop(data);

		let mut data = state
			.try_borrow_mut()
			.unwrap_or_else(|error| panic!("mutably borrow compact state: {error:?}"));
		let mut compact = TestCompactState::try_from_bytes_mut(&mut data)
			.unwrap_or_else(|error| panic!("mutably validate compact state: {error:?}"));
		compact.value = 4;
		let values = [crate::PodU64::from(8), crate::PodU64::from(13)];
		compact
			.set_items(&values)
			.unwrap_or_else(|error| panic!("set compact items: {error:?}"));
		assert_eq!(
			compact
				.commit()
				.unwrap_or_else(|error| panic!("commit compact state: {error:?}")),
			initial_size
		);
	}

	#[cfg(all(feature = "account-resize", feature = "compact"))]
	#[test]
	fn compact_resize_builder_grows_commits_and_returns_the_callback_value() {
		let owner = Address::new_from_array([9; 32]);
		let target_size = TestCompactState::projected_bytes(2)
			.unwrap_or_else(|error| panic!("calculate compact target: {error:?}"));
		let mut stored_account = TestAccount::<64>::new(
			Address::new_from_array([1; 32]),
			owner,
			1,
			TestCompactState::MIN_SIZE,
		);
		let mut stored_rent_account =
			TestAccount::<0>::new(Address::new_from_array([2; 32]), owner, 1_000, 0);
		let mut account = stored_account.view();
		let mut rent_account = stored_rent_account.view();
		let items = [crate::PodU64::from(8), crate::PodU64::from(13)];
		let initial_lamports = account.lamports() + rent_account.lamports();

		{
			let mut data = account
				.try_borrow_mut()
				.unwrap_or_else(|error| panic!("borrow compact state: {error:?}"));
			TestCompactState::initialize(&mut data)
				.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));
		}

		let result = ResizeCompactAccount {
			account: &mut account,
			rent_account: &mut rent_account,
			target_size,
			program_id: &owner,
		}
		.invoke_signed_with_rent::<TestCompactState, _>(
			&[],
			|data| {
				let mut state = TestCompactState::try_from_bytes_mut(data)?;
				state.value = 21;
				state
					.set_items(&items)
					.map_err(|_| ProgramError::InvalidAccountData)?;
				state
					.commit()
					.map_err(|_| ProgramError::InvalidAccountData)?;

				Ok(34)
			},
			test_rent(),
		)
		.unwrap_or_else(|error| panic!("grow compact state: {error:?}"));

		assert_eq!(result, 34);
		assert_eq!(account.data_len(), target_size);
		assert_eq!(
			account.lamports() + rent_account.lamports(),
			initial_lamports
		);
		account
			.with_compact_account::<TestCompactState, _>(&owner, |state| {
				assert_eq!(state.value, 21);
				assert_eq!(state.items(), items);

				Ok(())
			})
			.unwrap_or_else(|error| panic!("read grown compact state: {error:?}"));
	}

	#[cfg(all(feature = "account-resize", feature = "compact"))]
	#[test]
	fn compact_resize_builder_commits_before_shrinking_and_refunds_rent() {
		let owner = Address::new_from_array([9; 32]);
		let initial_size = TestCompactState::projected_bytes(2)
			.unwrap_or_else(|error| panic!("calculate initial compact size: {error:?}"));
		let mut stored_account =
			TestAccount::<64>::new(Address::new_from_array([1; 32]), owner, 1_000, initial_size);
		let mut stored_rent_account =
			TestAccount::<0>::new(Address::new_from_array([2; 32]), owner, 1, 0);
		let mut account = stored_account.view();
		let mut rent_account = stored_rent_account.view();
		let items = [crate::PodU64::from(8), crate::PodU64::from(13)];
		let initial_lamports = account.lamports() + rent_account.lamports();
		let initial_rent_lamports = rent_account.lamports();

		{
			let mut data = account
				.try_borrow_mut()
				.unwrap_or_else(|error| panic!("borrow compact state: {error:?}"));
			let mut state = TestCompactState::initialize(&mut data)
				.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));
			state
				.set_items(&items)
				.unwrap_or_else(|error| panic!("set compact items: {error:?}"));
			state
				.commit()
				.unwrap_or_else(|error| panic!("commit compact items: {error:?}"));
		}

		ResizeCompactAccount {
			account: &mut account,
			rent_account: &mut rent_account,
			target_size: TestCompactState::MIN_SIZE,
			program_id: &owner,
		}
		.invoke_signed_with_rent::<TestCompactState, _>(
			&[],
			|data| {
				let mut state = TestCompactState::try_from_bytes_mut(data)?;
				state
					.set_items(&[])
					.map_err(|_| ProgramError::InvalidAccountData)?;
				state
					.commit()
					.map_err(|_| ProgramError::InvalidAccountData)?;

				Ok(())
			},
			test_rent(),
		)
		.unwrap_or_else(|error| panic!("shrink compact state: {error:?}"));

		assert_eq!(account.data_len(), TestCompactState::MIN_SIZE);
		assert_eq!(
			account.lamports() + rent_account.lamports(),
			initial_lamports
		);
		assert!(rent_account.lamports() > initial_rent_lamports);
	}

	#[cfg(all(feature = "account-resize", feature = "compact"))]
	#[test]
	fn compact_resize_builder_skips_realloc_for_an_unchanged_size() {
		let owner = Address::new_from_array([9; 32]);
		let mut stored_account = TestAccount::<64>::new(
			Address::new_from_array([1; 32]),
			owner,
			1,
			TestCompactState::MIN_SIZE,
		);
		let mut stored_rent_account =
			TestAccount::<0>::new(Address::new_from_array([2; 32]), owner, 1, 0);
		let mut account = stored_account.view();
		let mut rent_account = stored_rent_account.view();

		{
			let mut data = account
				.try_borrow_mut()
				.unwrap_or_else(|error| panic!("borrow compact state: {error:?}"));
			TestCompactState::initialize(&mut data)
				.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));
		}

		let empty_seeds: [Seed<'_>; 0] = [];
		let signer = Signer::from(&empty_seeds);
		let result = ResizeCompactAccount {
			account: &mut account,
			rent_account: &mut rent_account,
			target_size: TestCompactState::MIN_SIZE,
			program_id: &owner,
		}
		.invoke_signed::<TestCompactState, _>(&[signer], |data| {
			let mut state = TestCompactState::try_from_bytes_mut(data)?;
			state.value = 55;
			state
				.commit()
				.map_err(|_| ProgramError::InvalidAccountData)?;

			Ok(89)
		})
		.unwrap_or_else(|error| panic!("update compact state: {error:?}"));

		assert_eq!(result, 89);
		assert_eq!(account.data_len(), TestCompactState::MIN_SIZE);
	}

	#[cfg(all(feature = "account-resize", feature = "compact"))]
	#[test]
	fn compact_resize_builder_rejects_mismatched_and_failed_updates() {
		let owner = Address::new_from_array([9; 32]);
		let allocated_size = TestCompactState::projected_bytes(1)
			.unwrap_or_else(|error| panic!("calculate allocated compact size: {error:?}"));
		let mut stored_account =
			TestAccount::<64>::new(Address::new_from_array([1; 32]), owner, 1, allocated_size);
		let mut stored_rent_account =
			TestAccount::<0>::new(Address::new_from_array([2; 32]), owner, 1, 0);
		let mut account = stored_account.view();
		let mut rent_account = stored_rent_account.view();

		{
			let mut data = account
				.try_borrow_mut()
				.unwrap_or_else(|error| panic!("borrow compact state: {error:?}"));
			TestCompactState::initialize(&mut data)
				.unwrap_or_else(|error| panic!("initialize compact state: {error:?}"));
		}

		let mismatch = ResizeCompactAccount {
			account: &mut account,
			rent_account: &mut rent_account,
			target_size: allocated_size,
			program_id: &owner,
		}
		.invoke::<TestCompactState, _>(|data| {
			let mut state = TestCompactState::try_from_bytes_mut(data)?;
			state
				.commit()
				.map_err(|_| ProgramError::InvalidAccountData)?;

			Ok(())
		});
		assert_eq!(mismatch, Err(ProgramError::InvalidAccountData));
		assert_eq!(account.data_len(), allocated_size);

		let callback_error = ResizeCompactAccount {
			account: &mut account,
			rent_account: &mut rent_account,
			target_size: allocated_size,
			program_id: &owner,
		}
		.invoke::<TestCompactState, _>(|_| Err::<(), _>(ProgramError::InvalidArgument));
		assert_eq!(callback_error, Err(ProgramError::InvalidArgument));
		assert_eq!(account.data_len(), allocated_size);

		let invalid_target = ResizeCompactAccount {
			account: &mut account,
			rent_account: &mut rent_account,
			target_size: TestCompactState::HEADER_SIZE + 1,
			program_id: &owner,
		}
		.invoke::<TestCompactState, _>(|_| Ok(()));
		assert_eq!(invalid_target, Err(ProgramError::InvalidAccountData));
		assert_eq!(account.data_len(), allocated_size);
	}

	#[cfg(all(feature = "account-resize", feature = "compact"))]
	#[test]
	fn compact_resize_builder_propagates_growth_and_shrink_realloc_failures() {
		let owner = Address::new_from_array([9; 32]);
		let grown_size = TestCompactState::projected_bytes(1)
			.unwrap_or_else(|error| panic!("calculate grown compact size: {error:?}"));
		let mut stored_growth_account = TestAccount::<64>::new(
			Address::new_from_array([1; 32]),
			owner,
			1,
			TestCompactState::MIN_SIZE,
		);
		let mut stored_empty_rent_account =
			TestAccount::<0>::new(Address::new_from_array([2; 32]), owner, 0, 0);
		let mut growth_account = stored_growth_account.view();
		let mut empty_rent_account = stored_empty_rent_account.view();
		{
			let mut data = growth_account
				.try_borrow_mut()
				.unwrap_or_else(|error| panic!("borrow growth state: {error:?}"));
			TestCompactState::initialize(&mut data)
				.unwrap_or_else(|error| panic!("initialize growth state: {error:?}"));
		}
		let callback_count = core::cell::Cell::new(0);
		let edit_account = |_: &mut [u8]| {
			callback_count.set(callback_count.get() + 1);
			Ok::<(), ProgramError>(())
		};
		edit_account(&mut []).unwrap_or_else(|error| panic!("exercise growth callback: {error:?}"));
		assert_eq!(callback_count.get(), 1);
		let growth = ResizeCompactAccount {
			account: &mut growth_account,
			rent_account: &mut empty_rent_account,
			target_size: grown_size,
			program_id: &owner,
		}
		.invoke_signed_with_rent::<TestCompactState, _>(
			&[],
			edit_account,
			Rent::from_bytes(&u64::MAX.to_le_bytes())
				.unwrap_or_else(|error| panic!("overflowing test rent: {error:?}")),
		);
		assert!(growth.is_err());
		assert_eq!(callback_count.get(), 1);

		let mut stored_shrink_account =
			TestAccount::<64>::new(Address::new_from_array([3; 32]), owner, 1_000, grown_size);
		let mut stored_full_rent_account =
			TestAccount::<0>::new(Address::new_from_array([4; 32]), owner, u64::MAX, 0);
		let mut shrink_account = stored_shrink_account.view();
		let mut full_rent_account = stored_full_rent_account.view();
		let item = [crate::PodU64::from(8)];
		{
			let mut data = shrink_account
				.try_borrow_mut()
				.unwrap_or_else(|error| panic!("borrow shrink state: {error:?}"));
			let mut state = TestCompactState::initialize(&mut data)
				.unwrap_or_else(|error| panic!("initialize shrink state: {error:?}"));
			state
				.set_items(&item)
				.unwrap_or_else(|error| panic!("set shrink item: {error:?}"));
			state
				.commit()
				.unwrap_or_else(|error| panic!("commit shrink item: {error:?}"));
		}
		let shrink = ResizeCompactAccount {
			account: &mut shrink_account,
			rent_account: &mut full_rent_account,
			target_size: TestCompactState::MIN_SIZE,
			program_id: &owner,
		}
		.invoke_signed_with_rent::<TestCompactState, _>(
			&[],
			|data| {
				let mut state = TestCompactState::try_from_bytes_mut(data)?;
				state
					.set_items(&[])
					.map_err(|_| ProgramError::InvalidAccountData)?;
				state
					.commit()
					.map_err(|_| ProgramError::InvalidAccountData)?;
				Ok(())
			},
			test_rent(),
		);
		assert_eq!(shrink, Err(ProgramError::ArithmeticOverflow));
	}

	#[test]
	fn prefunded_pda_allocation_runs_transfer_allocate_and_assign() {
		let owner = Address::new_from_array([9; 32]);
		let seeds: &[&[u8]] = &[b"prefunded"];
		let (address, bump) = crate::try_find_program_address(seeds, &owner)
			.unwrap_or_else(|| panic!("derive prefunded address"));
		let mut stored_target = TestAccount::<32>::new(address, owner, 1, 8);
		let mut stored_payer = TestAccount::<0>::new(Address::new_from_array([1; 32]), owner, 1, 0);
		let target = stored_target.view();
		let payer = stored_payer.view();
		let empty_seeds: [Seed<'_>; 0] = [];
		let extra_signer = Signer::from(&empty_seeds);

		AllocateAccountWithBump {
			account: &target,
			payer: &payer,
			space: 8,
			owner: &owner,
			seeds,
			bump,
		}
		.invoke_signed_with_rent(&[extra_signer], test_rent())
		.unwrap_or_else(|error| panic!("allocate prefunded PDA: {error:?}"));

		let rent = test_rent();
		let fully_funded = rent
			.try_minimum_balance(8)
			.unwrap_or_else(|error| panic!("calculate full funding: {error:?}"));
		let mut stored_funded = TestAccount::<32>::new(address, owner, fully_funded, 8);
		let funded = stored_funded.view();
		AllocateAccountWithBump {
			account: &funded,
			payer: &payer,
			space: 8,
			owner: &owner,
			seeds,
			bump,
		}
		.invoke_signed_with_rent(&[], rent)
		.unwrap_or_else(|error| panic!("allocate fully funded PDA: {error:?}"));
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn realloc_growth_executes_rent_transfer_and_resize() {
		let owner = Address::new_from_array([9; 32]);
		let mut stored_account =
			TestAccount::<32>::new(Address::new_from_array([1; 32]), owner, 1, 8);
		let mut stored_rent_account =
			TestAccount::<0>::new(Address::new_from_array([2; 32]), owner, 1, 0);
		let mut account = stored_account.view();
		let mut rent_account = stored_rent_account.view();

		realloc_account_inner_with_rent(
			&mut account,
			16,
			&mut rent_account,
			&owner,
			&[],
			Some(test_rent()),
		)
		.unwrap_or_else(|error| panic!("grow account: {error:?}"));

		assert_eq!(account.data_len(), 16);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn realloc_prefunded_growth_resizes_without_moving_lamports() {
		let owner = Address::new_from_array([9; 32]);
		let mut stored_account =
			TestAccount::<32>::new(Address::new_from_array([1; 32]), owner, 1_000, 8);
		let mut stored_rent_account =
			TestAccount::<0>::new(Address::new_from_array([2; 32]), owner, 1, 0);
		let mut account = stored_account.view();
		let mut rent_account = stored_rent_account.view();

		realloc_account_inner_with_rent(
			&mut account,
			16,
			&mut rent_account,
			&owner,
			&[],
			Some(test_rent()),
		)
		.unwrap_or_else(|error| panic!("grow prefunded account: {error:?}"));

		assert_eq!(account.data_len(), 16);
		assert_eq!(account.lamports(), 1_000);
		assert_eq!(rent_account.lamports(), 1);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn realloc_shrink_returns_excess_rent_and_resizes() {
		let owner = Address::new_from_array([9; 32]);
		let mut stored_account =
			TestAccount::<32>::new(Address::new_from_array([1; 32]), owner, 1_000, 16);
		let mut stored_rent_account =
			TestAccount::<0>::new(Address::new_from_array([2; 32]), owner, 1, 0);
		let mut account = stored_account.view();
		let mut rent_account = stored_rent_account.view();
		let initial_total = account.lamports() + rent_account.lamports();

		realloc_account_inner_with_rent(
			&mut account,
			8,
			&mut rent_account,
			&owner,
			&[],
			Some(test_rent()),
		)
		.unwrap_or_else(|error| panic!("shrink account: {error:?}"));

		assert_eq!(account.data_len(), 8);
		assert_eq!(account.lamports() + rent_account.lamports(), initial_total);
		assert!(rent_account.lamports() > 1);
	}
}
