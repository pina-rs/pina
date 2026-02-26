//! PDA (Program Derived Address) functions.
//!
//! These wrapper functions provide PDA derivation across native and Solana
//! targets.
//!
//! Seed-based APIs require deterministic seed ordering and consistent program
//! IDs across derivation and verification.

use crate::Address;
use crate::ProgramError;

/// Find a valid program derived address and its corresponding bump seed.
///
/// Returns `None` if no valid PDA exists.
///
/// This is the preferred PDA derivation API in `pina` because it is explicit
/// about failure and avoids panics in on-chain code paths.
///
/// # Examples
///
/// ```
/// use pina::try_find_program_address;
///
/// let program_id = pina::address!("11111111111111111111111111111111");
/// let seeds: &[&[u8]] = &[b"vault"];
///
/// if let Some((pda, bump)) = try_find_program_address(seeds, &program_id) {
/// 	// `pda` is the derived address, `bump` is the canonical bump seed.
/// 	assert!(bump <= 255);
/// }
/// ```
#[inline]
pub fn try_find_program_address(seeds: &[&[u8]], program_id: &Address) -> Option<(Address, u8)> {
	Address::try_find_program_address(seeds, program_id)
}

/// Find a valid program derived address and its corresponding bump seed.
///
/// # Panics
///
/// Panics if no valid PDA exists.
///
/// Prefer [`try_find_program_address`] for recoverable error handling.
#[deprecated(
	since = "0.3.0",
	note = "use `try_find_program_address` instead, which returns `Option` and avoids panicking \
	        on-chain"
)]
#[inline]
pub fn find_program_address(seeds: &[&[u8]], program_id: &Address) -> (Address, u8) {
	try_find_program_address(seeds, program_id)
		.unwrap_or_else(|| panic!("could not find program address from seeds"))
}

/// Create a valid program derived address without searching for a bump seed.
///
/// Use this when your instruction already carries a bump and you want to
/// verify exact PDA derivation against user-provided seeds.
///
/// <!-- {=pinaPdaSeedContract|trim|linePrefix:"/// ":true} -->/// Seed-based APIs require deterministic seed ordering.
///
/// Program IDs must stay consistent across derivation and verification.
///
/// When a bump is required, prefer canonical bump derivation.
///
/// Use explicit bumps when needed.<!-- {/pinaPdaSeedContract} -->
///
/// # Examples
///
/// ```
/// use pina::create_program_address;
/// use pina::try_find_program_address;
///
/// let program_id = pina::address!("11111111111111111111111111111111");
/// let seeds: &[&[u8]] = &[b"vault"];
///
/// // First derive the canonical PDA and bump:
/// let (pda, bump) =
/// 	try_find_program_address(seeds, &program_id).unwrap_or_else(|| panic!("no valid PDA"));
///
/// // Then recreate the address using the known bump:
/// let bump_seed = [bump];
/// let recreated = create_program_address(&[b"vault", &bump_seed], &program_id)
/// 	.unwrap_or_else(|e| panic!("failed to recreate: {e:?}"));
/// assert_eq!(pda, recreated);
/// ```
#[inline]
pub fn create_program_address(
	seeds: &[&[u8]],
	program_id: &Address,
) -> Result<Address, ProgramError> {
	Address::create_program_address(seeds, program_id).map_err(|_| ProgramError::InvalidSeeds)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn native_find_and_create_program_address_roundtrip() {
		let seeds: &[&[u8]] = &[b"pina-test"];
		let (pda, bump) =
			try_find_program_address(seeds, &crate::system::ID).unwrap_or_else(|| {
				panic!("expected to derive pda");
			});
		let bump_seed = [bump];
		let seeds_with_bump: &[&[u8]] = &[b"pina-test", &bump_seed];
		let recreated = create_program_address(seeds_with_bump, &crate::system::ID)
			.unwrap_or_else(|err| panic!("failed to recreate pda: {err:?}"));

		assert_eq!(pda, recreated);
	}
}
