#[cfg(feature = "logs")]
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
/// <!-- {=pinaPublicResultContract|trim|linePrefix:"/// ":true} -->///// All APIs in this section are designed for on-chain determinism.

/// They return `ProgramError` values for caller-side propagation with `?`.

/// No panics needed.<!-- {/pinaPublicResultContract} -->
///
/// # Examples
///
/// ```
/// use pina::IntoDiscriminator;
/// use pina::ProgramError;
/// use pina::parse_instruction;
///
/// let program_id = pina::system::ID;
/// let data = [7u8, 0, 0, 0];
///
/// let disc: u8 = parse_instruction(&program_id, &program_id, &data)
/// 	.unwrap_or_else(|e| panic!("parse failed: {e:?}"));
/// assert_eq!(disc, 7);
///
/// // Mismatched program IDs produce an error:
/// let other_id = pina::Address::new_from_array([1u8; 32]);
/// let err = parse_instruction::<u8>(&program_id, &other_id, &data).unwrap_err();
/// assert_eq!(err, ProgramError::IncorrectProgramId);
/// ```
pub fn parse_instruction<'a, T: IntoDiscriminator>(
	api_id: &'a Address,
	program_id: &'a Address,
	data: &'a [u8],
) -> Result<T, ProgramError> {
	// Validate the program id is valid.
	if program_id.ne(api_id) {
		return Err(ProgramError::IncorrectProgramId);
	}

	// Defense-in-depth: reject data that is too short for the discriminator.
	if data.len() < T::BYTES {
		return Err(ProgramError::InvalidInstructionData);
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
///
/// Intended for compact guard checks inside instruction handlers.
///
/// <!-- {=pinaPublicResultContract|trim|linePrefix:"/// ":true} -->///// All APIs in this section are designed for on-chain determinism.

/// They return `ProgramError` values for caller-side propagation with `?`.

/// No panics needed.<!-- {/pinaPublicResultContract} -->
///
/// # Examples
///
/// ```
/// use pina::ProgramError;
///
/// // Passing assertion returns Ok:
/// pina::assert(true, ProgramError::InvalidArgument, "always passes")
/// 	.unwrap_or_else(|e| panic!("unexpected: {e:?}"));
///
/// // Failing assertion returns the provided error:
/// let result = pina::assert(false, ProgramError::InvalidArgument, "amount is zero");
/// assert_eq!(result, Err(ProgramError::InvalidArgument));
/// ```
#[track_caller]
#[inline(always)]
pub fn assert(v: bool, err: impl Into<ProgramError>, msg: &str) -> ProgramResult {
	if v {
		Ok(())
	} else {
		#[cfg(not(feature = "logs"))]
		let _ = msg;

		log!("{}", msg);
		log_caller();
		Err(err.into())
	}
}

/// Logs caller file/line/column when `logs` feature is enabled.
///
/// Used internally by assertion helpers and account validation methods.
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

/// No-op variant used when the `logs` feature is disabled.
#[cfg(not(feature = "logs"))]
#[inline(always)]
pub fn log_caller() {}

/// Derives the associated token account address for the given wallet, mint,
/// and token program. Returns `None` if no valid PDA exists.
///
/// <!-- {=pinaTokenFeatureGateContract|trim|linePrefix:"/// ":true} -->///// This API is gated behind the `token` feature. Keep token-specific code behind `#[cfg(feature = "token")]` so on-chain programs that do not use SPL token interfaces can avoid extra dependencies.<!-- {/pinaTokenFeatureGateContract} -->
///
/// # Examples
///
/// ```ignore
/// let ata = try_get_associated_token_address(&wallet, &mint, &token::ID);
/// if let Some((address, bump)) = ata {
/// 	// Use the derived ATA address...
/// }
/// ```
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
