#[cfg(feature = "verbose-logs")]
use core::panic::Location;

use crate::Address;
use crate::IntoDiscriminator;
use crate::ProgramError;
use crate::ProgramResult;
#[cfg(feature = "verbose-logs")]
use crate::log;
#[cfg(feature = "verbose-logs")]
use crate::log_verbose;

/// Parses an instruction discriminator from the raw instruction data.
///
/// 1. Verifies that `program_id` matches `api_id`.
/// 2. Reads the discriminator bytes and converts them into `T`.
///
/// # Error mapping
///
/// To preserve external compatibility, `ProgramError::Custom(_)` from
/// discriminator parsing is remapped to `InvalidInstructionData`.
///
/// When the `logs` feature is enabled, the original custom error code is
/// emitted before remapping so diagnostic detail is still available during
/// development.
/// <!-- {=pinaPublicResultContract|trim|linePrefix:"/// ":true} -->
/// All APIs in this section are designed for on-chain determinism.
///
/// They return `ProgramError` values for caller-side propagation with `?`.
///
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
			ProgramError::Custom(code) => remap_custom_error(code),
			error => error,
		}
	})
}

/// Maps a custom discriminator-parse error to `InvalidInstructionData`.
///
/// Outlined with `#[cold]` and `#[inline(never)]` deliberately. Inlining this
/// error path changed how the compiler laid out the surrounding dispatch code
/// and cost a measured 7 CU on `pina_bpf_program/hello`, even though the arm
/// never runs for a valid discriminator. Keeping it as a separate cold
/// function holds the hot path at its previous cost.
#[cold]
#[inline(never)]
fn remap_custom_error(code: u32) -> ProgramError {
	// The formatted detail stays behind `verbose-logs`, so the default build
	// does not link `core::fmt` for this path.
	#[cfg(feature = "verbose-logs")]
	{
		log!(
			"parse_instruction: remapping ProgramError::Custom({}) to InvalidInstructionData",
			code
		);
	}
	let _ = code;
	ProgramError::InvalidInstructionData
}

/// Asserts a boolean condition, logging `msg` and returning `err` on failure.
///
/// Intended for compact guard checks inside instruction handlers.
///
/// The message is logged in every build that enables `logs`. With
/// `verbose-logs` it goes through the formatted logger together with the
/// caller location; without it the raw string is written with a single
/// `sol_log_` syscall over the caller's own slice.
///
/// <!-- {=pinaPublicResultContract|trim|linePrefix:"/// ":true} -->
/// All APIs in this section are designed for on-chain determinism.
///
/// They return `ProgramError` values for caller-side propagation with `?`.
///
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
		#[cfg(feature = "verbose-logs")]
		{
			log_verbose!("{}", msg);
		}
		#[cfg(not(feature = "verbose-logs"))]
		{
			log_raw(msg);
		}

		log_caller();
		Err(err.into())
	}
}

/// Writes a caller-supplied string to the log without formatting.
///
/// The message is borrowed from the call site, so this lowers to a single
/// `sol_log_` syscall over an already-materialized slice instead of building a
/// formatted message through [`log_verbose!`].
#[cfg(all(feature = "logs", not(feature = "verbose-logs")))]
#[inline(always)]
fn log_raw(msg: &str) {
	solana_program_log::logger::log_message(msg.as_bytes());
}

/// No-op variant used when logging is compiled out entirely.
///
/// The message argument is still consumed so call sites keep identical
/// semantics across feature combinations.
#[cfg(not(feature = "logs"))]
#[inline(always)]
fn log_raw(msg: &str) {
	let _ = msg;
}

/// Logs caller file/line/column when `verbose-logs` feature is enabled.
///
/// Used internally by assertion helpers and account validation methods. The
/// location format pulls in `core::fmt`, so it is part of the opt-in verbose
/// diagnostics rather than the default failure path.
#[cfg(feature = "verbose-logs")]
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

/// No-op variant used when the `verbose-logs` feature is disabled.
#[cfg(not(feature = "verbose-logs"))]
#[inline(always)]
pub fn log_caller() {}

/// Derives the associated token account address for the given wallet, mint,
/// and token program. Returns `None` if no valid PDA exists.
///
/// <!-- {=pinaTokenFeatureGateContract|trim|linePrefix:"/// ":true} -->
/// This API is gated behind the `token` feature. Keep token-specific code behind `#[cfg(feature = "token")]` so on-chain programs that do not use SPL token interfaces can avoid extra dependencies.<!-- {/pinaTokenFeatureGateContract} -->
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
