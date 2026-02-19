//! PDA (Program Derived Address) functions.
//!
//! These wrapper functions provide PDA derivation across native and Solana
//! targets.

use crate::Address;
use crate::ProgramError;

/// Find a valid program derived address and its corresponding bump seed.
///
/// Returns `None` if no valid PDA exists.
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
#[inline]
pub fn find_program_address(seeds: &[&[u8]], program_id: &Address) -> (Address, u8) {
	try_find_program_address(seeds, program_id)
		.unwrap_or_else(|| panic!("could not find program address from seeds"))
}

/// Create a valid program derived address without searching for a bump seed.
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
