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

/// Creates a new non-program account.
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

/// Creates a new program account and returns the address and canonical bump.
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

/// Creates a new program account with user-provided bump.
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

/// Closes an account and returns the remaining rent lamports to the provided
/// recipient.
///
/// Zeroes account data before closing to prevent stale data from being read
/// by subsequent transactions.
#[inline(always)]
pub fn close_account(account_info: &AccountView, recipient: &AccountView) -> ProgramResult {
	// Return rent lamports.
	account_info.send(account_info.lamports(), recipient)?;
	// Zero account data before closing.
	account_info.resize(0)?;
	// Close the account.
	account_info.close()
}
