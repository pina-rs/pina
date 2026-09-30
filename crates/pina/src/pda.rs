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
/// <!-- {=pinaPdaSeedContract|trim|linePrefix:"/// ":true} -->
/// Seed-based APIs require deterministic seed ordering.
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

/// Returns whether `address` is the program address `seeds` and `bump`
/// derive for `program_id`, without checking that it is off the ed25519
/// curve.
///
/// This hashes the same input as [`create_program_address`] with the
/// `sol_sha256` syscall instead of `sol_create_program_address`, which saves
/// about 1,350 compute units per call. The only address it accepts that
/// [`create_program_address`] rejects is an on-curve hash of those inputs.
///
/// That makes it the right check for re-verifying an account the program
/// created at a derived address from the bump it stored: the program created
/// the account through `invoke_signed`, and the runtime only signs for an
/// off-curve address, so the stored bump already derived a valid program
/// address. Use [`create_program_address`] to check a bump a caller supplies,
/// and [`try_find_program_address`] when the bump must be canonical.
///
/// # Examples
///
/// ```
/// use pina::is_derived_address;
/// use pina::try_find_program_address;
///
/// let program_id = pina::address!("11111111111111111111111111111111");
/// let (pda, bump) = try_find_program_address(&[b"vault"], &program_id)
/// 	.unwrap_or_else(|| panic!("no valid PDA"));
///
/// assert!(is_derived_address(&pda, &[b"vault"], bump, &program_id));
/// assert!(!is_derived_address(&pda, &[b"other"], bump, &program_id));
/// ```
#[inline(always)]
pub fn is_derived_address<const N: usize>(
	address: &Address,
	seeds: &[&[u8]; N],
	bump: u8,
	program_id: &Address,
) -> bool {
	Address::derive_address(seeds, Some(bump), program_id) == *address
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

	/// The hash-only check agrees with `create_program_address` for every
	/// bump that derives a valid program address, and rejects other seeds,
	/// bumps, and program IDs.
	#[test]
	fn derived_address_check_matches_create_program_address() {
		let seeds: [&[u8]; 2] = [b"pina-test", b"derived"];
		let mut checked_bumps = 0;

		for bump in 0..=u8::MAX {
			let bump_seed = [bump];
			let Ok(address) =
				create_program_address(&[seeds[0], seeds[1], &bump_seed], &crate::system::ID)
			else {
				continue;
			};

			assert!(is_derived_address(
				&address,
				&seeds,
				bump,
				&crate::system::ID
			));
			assert!(!is_derived_address(
				&address,
				&seeds,
				bump.wrapping_add(1),
				&crate::system::ID
			));
			assert!(!is_derived_address(
				&address,
				&[seeds[0]],
				bump,
				&crate::system::ID
			));
			assert!(!is_derived_address(
				&address,
				&seeds,
				bump,
				&Address::new_from_array([9; 32])
			));
			checked_bumps += 1;
		}

		assert!(checked_bumps > 0);
	}

	/// For an on-curve hash, `create_program_address` fails while the
	/// hash-only check still matches the hash, which is the one case the two
	/// disagree on.
	#[test]
	fn derived_address_check_skips_only_the_curve_check() {
		let seeds: [&[u8]; 1] = [b"pina-test"];
		let on_curve_bump = (0..=u8::MAX).find(|bump| {
			create_program_address(&[seeds[0], &[*bump]], &crate::system::ID).is_err()
		});
		assert!(on_curve_bump.is_some(), "expected an on-curve bump");
		let on_curve_bump = on_curve_bump.unwrap_or_default();
		let hash = Address::derive_address(&seeds, Some(on_curve_bump), &crate::system::ID);

		assert!(is_derived_address(
			&hash,
			&seeds,
			on_curve_bump,
			&crate::system::ID
		));
	}
}
