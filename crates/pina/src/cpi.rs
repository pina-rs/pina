use bytemuck::Pod;
use pinocchio::account_info::AccountInfo;
use pinocchio::instruction::Seed;
use pinocchio::instruction::Signer;
use pinocchio::program_error::ProgramError;
use pinocchio::pubkey::MAX_SEEDS;
use pinocchio::pubkey::Pubkey;
use pinocchio::pubkey::try_find_program_address;
use pinocchio::sysvars::Sysvar;
use pinocchio::sysvars::rent::Rent;
use pinocchio_system::instructions::Allocate;
use pinocchio_system::instructions::Assign;
use pinocchio_system::instructions::CreateAccount;
use pinocchio_system::instructions::Transfer;

use crate::HasDiscriminator;
use crate::LamportTransfer;
use crate::ProgramResult;

/// Creates a new non-program account.
#[inline(always)]
pub fn create_account<'a>(
	from: &'a AccountInfo,
	to: &'a AccountInfo,
	space: usize,
	owner: &Pubkey,
) -> ProgramResult {
	let lamports = Rent::get()?.minimum_balance(space);

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
	target_account: &'a AccountInfo,
	payer: &'a AccountInfo,
	owner: &Pubkey,
	seeds: &[&[u8]],
) -> Result<(Pubkey, u8), ProgramError> {
	let Some((address, bump)) = try_find_program_address(seeds, owner) else {
		return Err(ProgramError::InvalidSeeds);
	};

	create_program_account_with_bump::<T>(target_account, payer, owner, seeds, bump)?;

	Ok((address, bump))
}

/// Creates a new program account with user-provided bump.
#[inline(always)]
pub fn create_program_account_with_bump<'a, T: HasDiscriminator + Pod>(
	target_account: &'a AccountInfo,
	payer: &'a AccountInfo,
	owner: &Pubkey,
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
	target_account: &'a AccountInfo,
	payer: &'a AccountInfo,
	space: usize,
	owner: &Pubkey,
	seeds: &[&[u8]],
) -> Result<(Pubkey, u8), ProgramError> {
	let Some((address, bump)) = try_find_program_address(seeds, owner) else {
		return Err(ProgramError::InvalidSeeds);
	};

	allocate_account_with_bump(target_account, payer, space, owner, seeds, bump)?;

	Ok((address, bump))
}

/// Appends a single-byte bump seed to the provided seeds array, returning
/// a fixed-size `[&[u8]; MAX_SEEDS]` suitable for PDA signing.
///
/// # Panics
///
/// Panics if `seeds.len() >= MAX_SEEDS`. On-chain panics are fatal and will
/// abort the transaction.
// SECURITY: this function panics rather than returning a Result. Callers
// must ensure seed count is within bounds before calling.
// TODO: consider returning `Result` instead of panicking to give callers
// the option of a graceful error path.
pub fn combine_seeds_with_bump<'a>(seeds: &[&'a [u8]], bump: &'a [u8; 1]) -> [&'a [u8]; MAX_SEEDS] {
	assert!(
		seeds.len() < MAX_SEEDS,
		"number of seeds must be less than MAX_SEEDS"
	);

	// Create our backing storage on the stack, initialized with empty slices.
	// Using a block ensures `storage` lives as long as the returned slice needs it
	// to.
	let mut storage: [&'a [u8]; MAX_SEEDS] = [&[]; MAX_SEEDS];

	// 1. Copy the original seeds into our storage array.
	let seeds_len = seeds.len();
	storage[..seeds_len].copy_from_slice(seeds);

	// 2. Add the single-byte bump slice to the end.
	storage[seeds_len] = bump;

	storage
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
	target_account: &'a AccountInfo,
	payer: &'a AccountInfo,
	space: usize,
	owner: &Pubkey,
	seeds: &[&[u8]],
	bump: u8,
) -> ProgramResult {
	// Combine seeds
	let bump_array = [bump];
	let combined_seeds = combine_seeds_with_bump(seeds, &bump_array).map(Seed::from);
	let seeds = &combined_seeds[..=seeds.len()];
	let signer = Signer::from(seeds);
	let signers = &[signer];
	// Allocate space for account
	let rent = Rent::get()?;

	if target_account.lamports().eq(&0) {
		let lamports = rent.minimum_balance(space);

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
			.minimum_balance(space)
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
#[inline(always)]
pub fn close_account(account_info: &AccountInfo, recipient: &AccountInfo) -> ProgramResult {
	// Return rent lamports.
	account_info.send(account_info.lamports(), recipient)?;
	// Realloc data to zero.
	account_info.close()
}
