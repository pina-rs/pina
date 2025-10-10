use core::panic::Location;

use crate::log;
use crate::ProgramError;
use crate::ProgramResult;
use crate::Pubkey;

/// Parses an instruction from the instruction data.
pub fn parse_instruction<'a, T: TryFrom<u8>>(
	api_id: &'a Pubkey,
	program_id: &'a Pubkey,
	data: &'a [u8],
) -> Result<(T, &'a [u8]), ProgramError> {
	// Validate the program id is valid.
	if program_id.ne(api_id) {
		return Err(ProgramError::IncorrectProgramId);
	}

	// Parse data for instruction discriminator.
	let (tag, data) = data
		.split_first()
		.ok_or(ProgramError::InvalidInstructionData)?;

	// Get instruction for discriminator.
	let ix = T::try_from(*tag).or(Err(ProgramError::InvalidInstructionData))?;

	// Return
	Ok((ix, data))
}

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
pub fn log_caller() {}

#[cfg(feature = "token")]
pub fn try_get_associated_token_address(
	wallet_address: &Pubkey,
	token_mint_address: &Pubkey,
	token_program_id: &Pubkey,
) -> Option<(Pubkey, u8)> {
	crate::try_find_program_address(
		&[wallet_address, token_program_id, token_mint_address],
		&pinocchio_associated_token_account::ID,
	)
}
