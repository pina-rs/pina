//! CPI and account-allocation helpers used by on-chain instruction handlers.
//!
//! These utilities wrap common system-program patterns (create, allocate,
//! assign, close) with consistent `ProgramError` behavior and PDA signing.
//! All APIs in this module are designed for on-chain determinism and return
//! `ProgramError` values for caller-side propagation with `?` instead of
//! panicking.
//!
//! Seed-based helpers require deterministic seed ordering and consistent
//! program IDs across derivation and verification.

use bytemuck::Pod;
use pinocchio::AccountView;
use pinocchio::Address;
use pinocchio::cpi::Seed;
use pinocchio::cpi::Signer;
use pinocchio::error::ProgramError;
use pinocchio::sysvars::Sysvar;
use pinocchio::sysvars::rent::Rent;
use pinocchio_system::instructions::Allocate;
use pinocchio_system::instructions::Assign;
use pinocchio_system::instructions::CreateAccount;
use pinocchio_system::instructions::Transfer;

use crate::HasDiscriminator;
use crate::LamportTransfer;
use crate::MAX_SEEDS;
use crate::ProgramResult;

/// Creates a new system account owned by `owner`.
///
/// Calculates the rent-exempt balance for `space`, then issues a single
/// `CreateAccount` CPI from `from` to `to`.
///
/// # Errors
///
/// Returns errors from rent sysvar access, rent minimum-balance computation,
/// or the underlying system-program CPI.
#[inline(always)]
pub fn create_account<'a>(
	from: &'a AccountView,
	to: &'a AccountView,
	space: usize,
	owner: &Address,
) -> ProgramResult {
	let lamports = Rent::get()?.try_minimum_balance(space)?;

	CreateAccount {
		from,
		to,
		lamports,
		space: space as u64,
		owner,
	}
	.invoke()
}

/// Creates a new PDA-backed program account and returns `(address, bump)`.
///
/// This helper derives the canonical PDA for `seeds` + `owner`, allocates
/// account storage for `T`, and assigns account ownership to `owner`.
///
/// <!-- {=pinaPdaSeedContract|trim|linePrefix:"/// ":true} -->/// Seed-based APIs require deterministic seed ordering.
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
#[inline(always)]
pub fn create_program_account<'a, T: HasDiscriminator + Pod>(
	target_account: &'a AccountView,
	payer: &'a AccountView,
	owner: &Address,
	seeds: &[&[u8]],
) -> Result<(Address, u8), ProgramError> {
	let Some((address, bump)) = crate::try_find_program_address(seeds, owner) else {
		return Err(ProgramError::InvalidSeeds);
	};

	create_program_account_with_bump::<T>(target_account, payer, owner, seeds, bump)?;

	Ok((address, bump))
}

/// Creates a new PDA-backed program account using a caller-provided `bump`.
///
/// Prefer [`create_program_account`] when you want canonical bump derivation.
/// Use this function when the bump is instruction data and must be validated.
///
/// <!-- {=pinaPdaSeedContract|trim|linePrefix:"/// ":true} -->/// Seed-based APIs require deterministic seed ordering.
///
/// Program IDs must stay consistent across derivation and verification.
///
/// When a bump is required, prefer canonical bump derivation.
///
/// Use explicit bumps when needed.<!-- {/pinaPdaSeedContract} -->
///
/// # Errors
///
/// Returns any error produced by [`allocate_account_with_bump`], including
/// invalid seed layouts and system-program CPI failures.
#[inline(always)]
pub fn create_program_account_with_bump<'a, T: HasDiscriminator + Pod>(
	target_account: &'a AccountView,
	payer: &'a AccountView,
	owner: &Address,
	seeds: &[&[u8]],
	bump: u8,
) -> ProgramResult {
	// Allocate space.
	allocate_account_with_bump(target_account, payer, size_of::<T>(), owner, seeds, bump)?;

	Ok(())
}

/// Allocates space for a new program account, returning the derived `address`
/// and the canonical `bump`.
///
/// This is the lower-level allocator used by [`create_program_account`] for
/// cases where caller code wants manual discriminator/data initialization.
///
/// <!-- {=pinaPdaSeedContract|trim|linePrefix:"/// ":true} -->/// Seed-based APIs require deterministic seed ordering.
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
/// allocation errors surfaced by [`allocate_account_with_bump`].
#[inline(always)]
pub fn allocate_account<'a>(
	target_account: &'a AccountView,
	payer: &'a AccountView,
	space: usize,
	owner: &Address,
	seeds: &[&[u8]],
) -> Result<(Address, u8), ProgramError> {
	let Some((address, bump)) = crate::try_find_program_address(seeds, owner) else {
		return Err(ProgramError::InvalidSeeds);
	};

	allocate_account_with_bump(target_account, payer, space, owner, seeds, bump)?;

	Ok((address, bump))
}

/// Appends a single-byte bump seed to the provided seeds array, returning
/// a fixed-size `[Seed; MAX_SEEDS]` suitable for PDA signing.
///
/// # Errors
///
/// Returns `ProgramError::InvalidSeeds` if `seeds.len() >= MAX_SEEDS`.
///
/// <!-- {=pinaPdaSeedContract|trim|linePrefix:"/// ":true} -->/// Seed-based APIs require deterministic seed ordering.
///
/// Program IDs must stay consistent across derivation and verification.
///
/// When a bump is required, prefer canonical bump derivation.
///
/// Use explicit bumps when needed.<!-- {/pinaPdaSeedContract} -->
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

/// Allocates space for a new program account with user-provided bump.
///
/// Two paths are taken depending on whether the target account already has
/// lamports:
///
/// - **Zero balance** — a single `CreateAccount` CPI is issued.
/// - **Non-zero balance** — a `Transfer` (to top up rent), `Allocate`, and
///   `Assign` are issued separately. This covers the case where the account was
///   pre-funded (e.g. by a previous failed transaction).
///
/// <!-- {=pinaPdaSeedContract|trim|linePrefix:"/// ":true} -->/// Seed-based APIs require deterministic seed ordering.
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
/// `Assign`.
#[inline(always)]
pub fn allocate_account_with_bump<'a>(
	target_account: &'a AccountView,
	payer: &'a AccountView,
	space: usize,
	owner: &Address,
	seeds: &[&[u8]],
	bump: u8,
) -> ProgramResult {
	// Combine seeds
	let bump_array = [bump];
	let combined_seeds = combine_seeds_with_bump(seeds, &bump_array)?;
	let seeds_slice = &combined_seeds[..=seeds.len()];
	let signer = Signer::from(seeds_slice);
	let signers = &[signer];
	// Allocate space for account
	let rent = Rent::get()?;

	if target_account.lamports().eq(&0) {
		let lamports = rent.try_minimum_balance(space)?;

		CreateAccount {
			from: payer,
			to: target_account,
			lamports,
			space: space as u64,
			owner,
		}
		.invoke_signed(signers)?;
	} else {
		// Otherwise, if balance is nonzero:

		// 1) transfer sufficient lamports for rent exemption
		let rent_exempt_balance = rent
			.try_minimum_balance(space)?
			.saturating_sub(target_account.lamports());
		if rent_exempt_balance > 0 {
			Transfer {
				from: payer,
				to: target_account,
				lamports: rent_exempt_balance,
			}
			.invoke_signed(signers)?;
		}

		// 2) allocate space for the account
		Allocate {
			account: target_account,
			space: space as u64,
		}
		.invoke_signed(signers)?;

		// 3) assign our program as the owner
		Assign {
			account: target_account,
			owner,
		}
		.invoke_signed(signers)?;
	}

	Ok(())
}

/// Maximum number of bytes an account may grow by in a single instruction.
///
/// This limit is enforced by the Solana runtime. Attempting to grow an account
/// by more than this amount will cause `resize` to return
/// `ProgramError::InvalidRealloc`.
pub const MAX_PERMITTED_DATA_INCREASE: usize = 10_240;

/// Reallocates an account to `new_size` bytes, adjusting rent automatically.
///
/// When **growing**, transfers the additional rent-exempt lamports required from
/// `payer` to `account` via a system-program CPI. When **shrinking**, returns
/// excess rent lamports from `account` to `payer` by direct lamport
/// manipulation (the account must be owned by the executing program for this
/// path).
///
/// New bytes are zero-initialized by the Solana runtime.
///
/// # Limits
///
/// The Solana runtime limits account growth to [`MAX_PERMITTED_DATA_INCREASE`]
/// (10 KiB) per top-level instruction. Exceeding this limit returns
/// `ProgramError::InvalidRealloc`.
///
/// # Errors
///
/// Returns `ProgramError::InvalidAccountData` if the account is not writable,
/// `ProgramError::InvalidAccountOwner` if the account is not owned by
/// `program_id`, and propagates any errors from rent sysvar access, lamport
/// transfer, or the runtime `resize` call.
#[inline(always)]
pub fn realloc_account<'a>(
	account: &'a AccountView,
	new_size: usize,
	payer: &'a AccountView,
	program_id: &Address,
) -> ProgramResult {
	realloc_account_inner(account, new_size, payer, program_id)
}

/// Reallocates an account to `new_size` bytes with explicit zero-initialization,
/// adjusting rent automatically.
///
/// This function behaves identically to [`realloc_account`]. In the current
/// Solana runtime, new bytes are always zero-initialized regardless of which
/// variant is called. This function exists for API symmetry with the runtime's
/// `realloc(new_len, zero_init)` parameter and to make zero-initialization
/// intent explicit at the call site.
///
/// When **growing**, transfers the additional rent-exempt lamports required from
/// `payer` to `account` via a system-program CPI. When **shrinking**, returns
/// excess rent lamports from `account` to `payer` by direct lamport
/// manipulation (the account must be owned by the executing program for this
/// path).
///
/// # Limits
///
/// The Solana runtime limits account growth to [`MAX_PERMITTED_DATA_INCREASE`]
/// (10 KiB) per top-level instruction. Exceeding this limit returns
/// `ProgramError::InvalidRealloc`.
///
/// # Errors
///
/// Returns `ProgramError::InvalidAccountData` if the account is not writable,
/// `ProgramError::InvalidAccountOwner` if the account is not owned by
/// `program_id`, and propagates any errors from rent sysvar access, lamport
/// transfer, or the runtime `resize` call.
#[inline(always)]
pub fn realloc_account_zero<'a>(
	account: &'a AccountView,
	new_size: usize,
	payer: &'a AccountView,
	program_id: &Address,
) -> ProgramResult {
	realloc_account_inner(account, new_size, payer, program_id)
}

/// Shared implementation for [`realloc_account`] and [`realloc_account_zero`].
///
/// Validates the account, computes the rent delta, performs the lamport
/// transfer, and resizes the account data.
#[inline(always)]
fn realloc_account_inner<'a>(
	account: &'a AccountView,
	new_size: usize,
	payer: &'a AccountView,
	program_id: &Address,
) -> ProgramResult {
	use crate::AccountInfoValidation;

	// Validate the account is writable and owned by the program.
	account.assert_writable()?.assert_owner(program_id)?;

	let current_size = account.data_len();

	// Early return when the size is unchanged.
	if new_size == current_size {
		return Ok(());
	}

	let rent = Rent::get()?;
	let new_minimum_balance = rent.try_minimum_balance(new_size)?;
	let current_lamports = account.lamports();

	if new_size > current_size {
		// Growing: transfer additional rent from payer to account.
		let required_lamports = new_minimum_balance.saturating_sub(current_lamports);
		if required_lamports > 0 {
			Transfer {
				from: payer,
				to: account,
				lamports: required_lamports,
			}
			.invoke()?;
		}
	} else {
		// Shrinking: return excess rent from account to payer.
		let excess_lamports = current_lamports.saturating_sub(new_minimum_balance);
		if excess_lamports > 0 {
			account.send(excess_lamports, payer)?;
		}
	}

	// Resize the account data. The runtime zero-initializes new bytes.
	account.resize(new_size)
}

/// Closes an account and returns the remaining rent lamports to the provided
/// recipient.
///
/// Zeroes account data before closing to prevent stale data from being read
/// by subsequent transactions.
///
/// <!-- {=pinaPublicResultContract|trim|linePrefix:"/// ":true} -->/// All APIs in this section are designed for on-chain determinism.
///
/// They return `ProgramError` values for caller-side propagation with `?`.
///
/// No panics needed.<!-- {/pinaPublicResultContract} -->
///
/// # Errors
///
/// Returns errors from lamport transfer, data resize, or account close
/// operations.
#[inline(always)]
pub fn close_account(account_info: &AccountView, recipient: &AccountView) -> ProgramResult {
	// Return rent lamports.
	account_info.send(account_info.lamports(), recipient)?;
	// Zero account data before closing.
	account_info.resize(0)?;
	// Close the account.
	account_info.close()
}
