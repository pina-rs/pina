use core::panic::Location;

use crate::Address;
use crate::IntoDiscriminator;
use crate::ProgramError;
use crate::ProgramResult;
use crate::log;

/// Parses an instruction discriminator from the raw instruction data.
///
/// 1. Verifies that `program_id` matches `api_id`.
/// 2. Reads the discriminator bytes and converts them into `T`.
///
/// # Error mapping
///
/// If `discriminator_from_bytes` returns a `ProgramError::Custom(_)` (i.e.
/// `InvalidDiscriminator`), it is mapped to `InvalidInstructionData` so the
/// caller sees a generic "bad data" error instead of an internal framework
/// error.
// TODO: the error remapping above suppresses detail that could be useful
// for debugging. Consider preserving the original error or logging it.
pub fn parse_instruction<'a, T: IntoDiscriminator>(
	api_id: &'a Address,
	program_id: &'a Address,
	data: &'a [u8],
) -> Result<T, ProgramError> {
	// Validate the program id is valid.
	if program_id.ne(api_id) {
		return Err(ProgramError::IncorrectProgramId);
	}

	// Get instruction for discriminator.
	T::discriminator_from_bytes(data).map_err(|error| {
		match error {
			ProgramError::Custom(_) => ProgramError::InvalidInstructionData,
			error => error,
		}
	})
}

/// Asserts a boolean condition, logging `msg` and returning `err` on failure.
#[track_caller]
#[inline(always)]
pub fn assert(v: bool, err: impl Into<ProgramError>, msg: &str) -> ProgramResult {
	if v {
		Ok(())
	} else {
		log!("{}", msg);
		log_caller();
		Err(err.into())
	}
}

#[cfg(feature = "logs")]
#[track_caller]
#[inline(always)]
pub fn log_caller() {
	let caller = Location::caller();
	log!(
		"Location: {}:{}:{}",
		caller.file(),
		caller.line(),
		caller.column()
	);
}

#[cfg(not(feature = "logs"))]
#[inline(always)]
pub fn log_caller() {}

/// Derives the associated token account address for the given wallet, mint,
/// and token program. Returns `None` if no valid PDA exists.
#[cfg(feature = "token")]
pub fn try_get_associated_token_address(
	wallet_address: &Address,
	token_mint_address: &Address,
	token_program_id: &Address,
) -> Option<(Address, u8)> {
	crate::try_find_program_address(
		&[
			wallet_address.as_ref(),
			token_program_id.as_ref(),
			token_mint_address.as_ref(),
		],
		&pinocchio_associated_token_account::ID,
	)
}
