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
/// about 1,350 compute units per call. Like [`create_program_address`], it
/// rejects a seed longer than [`MAX_SEED_LEN`](crate::MAX_SEED_LEN): the hash
/// concatenates the seeds, so without that check a 33-byte seed would hash like
/// a 32-byte seed followed by a 1-byte one. The only address it accepts that
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
	// A seed of constant or fixed-size length folds this check away.
	seeds.iter().all(|seed| seed.len() <= crate::MAX_SEED_LEN)
		&& Address::derive_address(seeds, Some(bump), program_id) == *address
}

/// Returns whether `address` is the program address `seeds` and `bump`
/// derive for `program_id`, for a seed list whose length is only known at
/// run time, without checking that the address is off the ed25519 curve.
///
/// `inputs` is scratch space for the hash inputs: it must hold at least
/// `seeds.len() + 3` slices. This is the creation builders' counterpart of
/// [`is_derived_address`], used where the account is about to be created
/// through `invoke_signed` with the same seeds and bump. The runtime derives
/// the signer's address with the curve check during that call, so an on-curve
/// hash is still rejected, just by the runtime instead of by this check.
///
/// Like [`is_derived_address`], it rejects a seed longer than
/// [`MAX_SEED_LEN`](crate::MAX_SEED_LEN) before hashing, so the builder
/// returns `InvalidSeeds` for it as it did with `create_program_address`,
/// instead of leaving the rejection to the runtime.
#[inline(always)]
pub(crate) fn hashes_to_program_address<'a>(
	address: &Address,
	seeds: &[&'a [u8]],
	bump: &'a [u8; 1],
	program_id: &'a Address,
	inputs: &mut [&'a [u8]],
) -> bool {
	if seeds.iter().any(|seed| seed.len() > crate::MAX_SEED_LEN) {
		return false;
	}

	let seeds_len = seeds.len();
	inputs[..seeds_len].copy_from_slice(seeds);
	inputs[seeds_len] = bump.as_slice();
	inputs[seeds_len + 1] = program_id.as_ref();
	inputs[seeds_len + 2] = solana_address::PDA_MARKER.as_slice();

	solana_sha256_hasher::hashv(&inputs[..seeds_len + 3]).to_bytes() == *address.as_array()
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

	/// Seeds are hashed back to back, so a 33-byte seed hashes like a valid
	/// 32-byte seed followed by a 1-byte one. The check must reject it, as
	/// `create_program_address` does, instead of accepting that address.
	#[test]
	fn derived_address_check_rejects_a_seed_longer_than_the_limit() {
		let bytes = [3_u8; 33];
		let split: [&[u8]; 2] = [&bytes[..32], &bytes[32..]];
		let (address, bump) = try_find_program_address(&split, &crate::system::ID)
			.expect("the split seeds derive a program address");
		let bump_seed = [bump];

		assert!(is_derived_address(
			&address,
			&split,
			bump,
			&crate::system::ID
		));
		assert_eq!(
			create_program_address(&[&bytes, &bump_seed], &crate::system::ID),
			Err(ProgramError::InvalidSeeds)
		);
		assert!(!is_derived_address(
			&address,
			&[&bytes],
			bump,
			&crate::system::ID
		));
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
