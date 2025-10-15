use core::panic::Location;

use crate::log;
use crate::IntoDiscriminator;
use crate::ProgramError;
use crate::ProgramResult;
use crate::Pubkey;

/// Parses an instruction from the instruction data.
pub fn parse_instruction<'a, T: IntoDiscriminator>(
	api_id: &'a Pubkey,
	program_id: &'a Pubkey,
	data: &'a [u8],
) -> Result<T, ProgramError> {
	// Validate the program id is valid.
	if program_id.ne(api_id) {
		return Err(ProgramError::IncorrectProgramId);
	}

	// Get instruction for discriminator.
	T::discriminator_from_bytes(data)
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
#[inline(always)]
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
