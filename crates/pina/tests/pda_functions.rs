//! Curve-checked PDA derivation and recovery from invalid seeds.

use pina::ProgramError;
use pina::create_program_address;
use pina::try_find_program_address;

const SYSTEM_ID: pina::Address = pina::address!("11111111111111111111111111111111");

#[test]
fn try_find_program_address_returns_some_for_valid_seeds() {
	let result = try_find_program_address(&[b"test-seed"], &SYSTEM_ID);

	assert!(result.is_some(), "expected to derive a PDA");
}

#[test]
fn try_find_program_address_deterministic() {
	let (addr1, bump1) =
		try_find_program_address(&[b"hello"], &SYSTEM_ID).unwrap_or_else(|| panic!("no PDA"));
	let (addr2, bump2) =
		try_find_program_address(&[b"hello"], &SYSTEM_ID).unwrap_or_else(|| panic!("no PDA"));

	assert_eq!(addr1, addr2, "PDA derivation should be deterministic");
	assert_eq!(bump1, bump2, "bump should be deterministic");
}

#[test]
fn try_find_program_address_different_seeds_produce_different_addresses() {
	let (addr1, _) =
		try_find_program_address(&[b"seed-a"], &SYSTEM_ID).unwrap_or_else(|| panic!("no PDA"));
	let (addr2, _) =
		try_find_program_address(&[b"seed-b"], &SYSTEM_ID).unwrap_or_else(|| panic!("no PDA"));

	assert_ne!(
		addr1, addr2,
		"different seeds should produce different PDAs"
	);
}

#[test]
fn create_program_address_roundtrip() {
	let (pda, bump) =
		try_find_program_address(&[b"roundtrip"], &SYSTEM_ID).unwrap_or_else(|| panic!("no PDA"));

	let bump_seed = [bump];
	let recreated = create_program_address(&[b"roundtrip", &bump_seed], &SYSTEM_ID)
		.unwrap_or_else(|e| panic!("failed to recreate PDA: {e:?}"));

	assert_eq!(pda, recreated, "roundtrip PDA should match");
}

#[test]
fn create_program_address_wrong_bump_fails() {
	let (_pda, bump) =
		try_find_program_address(&[b"bump-test"], &SYSTEM_ID).unwrap_or_else(|| panic!("no PDA"));

	// Use a different bump — this should either fail or produce a different address.
	let wrong_bump = bump.wrapping_add(1);
	let wrong_bump_seed = [wrong_bump];
	let result = create_program_address(&[b"bump-test", &wrong_bump_seed], &SYSTEM_ID);

	match result {
		Ok(addr) => {
			let correct = try_find_program_address(&[b"bump-test"], &SYSTEM_ID)
				.unwrap_or_else(|| panic!("no PDA"));
			assert_ne!(
				addr, correct.0,
				"wrong bump should produce a different address"
			);
		}
		Err(err) => {
			assert_eq!(err, ProgramError::InvalidSeeds);
		}
	}
}

#[test]
fn curve_preflight_returns_a_recoverable_error_for_an_on_curve_seed_hash() {
	let seed = b"recoverable-pda";
	let bump = (0..=u8::MAX)
		.find(|bump| {
			create_program_address(&[seed, &[*bump]], &SYSTEM_ID) == Err(ProgramError::InvalidSeeds)
		})
		.expect("at least one bump hashes to an on-curve address");
	let bump_seed = [bump];
	let seeds: &[&[u8]] = &[seed, &bump_seed];
	let raw_address = pina::Address::derive_address(&[seed], Some(bump), &SYSTEM_ID);
	assert!(raw_address.is_on_curve());

	// This is the optional preflight documented for noncanonical creation.
	// Handling its error lets the instruction continue before any signed CPI.
	let (valid, valid_bump) =
		try_find_program_address(&[seed], &SYSTEM_ID).expect("a valid fallback PDA");
	let result = create_program_address(seeds, &SYSTEM_ID);
	assert_eq!(result, Err(ProgramError::InvalidSeeds));
	assert_eq!(result.unwrap_or(valid), valid);
	assert_eq!(
		create_program_address(&[seed, &[valid_bump]], &SYSTEM_ID),
		Ok(valid)
	);
}

#[test]
fn try_find_program_address_empty_seeds() {
	let result = try_find_program_address(&[], &SYSTEM_ID);

	assert!(result.is_some(), "empty seeds should still derive a PDA");
}

#[test]
fn try_find_program_address_multiple_seeds() {
	let result = try_find_program_address(&[b"prefix", b"middle", b"suffix"], &SYSTEM_ID);

	assert!(result.is_some(), "multi-seed PDA should derive");
}

/// A program and seeds whose canonical bump is 250, so the search has to step
/// past five bumps that hash onto the curve. The address and bump were derived
/// independently with `getProgramDerivedAddress` from `@solana/kit`.
const VAULT_PROGRAM: pina::Address = pina::address!("BHvLHF6mJpWxywWY5S2tsHdDtHirHyeRxoS6uF6T5FoY");
const VAULT_SEEDS: [&[u8]; 2] = [b"vault", b"pina-pda-4"];
const VAULT_ADDRESS: pina::Address = pina::address!("FvDWmqvqDnoT8hvD4uVMPnnLNCpbbX5MLPjpTCQVgnAp");
const VAULT_BUMP: u8 = 250;

#[test]
fn try_find_program_address_matches_an_independent_derivation() {
	assert_eq!(
		try_find_program_address(&VAULT_SEEDS, &VAULT_PROGRAM),
		Some((VAULT_ADDRESS, VAULT_BUMP))
	);

	// The canonical bump is the highest one that derives a valid address.
	for bump in VAULT_BUMP + 1..=u8::MAX {
		assert_eq!(
			create_program_address(&[VAULT_SEEDS[0], VAULT_SEEDS[1], &[bump]], &VAULT_PROGRAM),
			Err(ProgramError::InvalidSeeds),
			"bump {bump} unexpectedly derived a valid address"
		);
	}
}

#[test]
#[allow(deprecated)]
fn find_program_address_returns_the_canonical_address_and_bump() {
	assert_eq!(
		pina::find_program_address(&VAULT_SEEDS, &VAULT_PROGRAM),
		(VAULT_ADDRESS, VAULT_BUMP)
	);
}

#[test]
fn try_find_program_address_rejects_invalid_seeds() {
	let too_long = [7_u8; pina::MAX_SEED_LEN + 1];
	assert_eq!(try_find_program_address(&[&too_long], &VAULT_PROGRAM), None);

	// The search appends the bump as one more seed, so 16 caller seeds leave
	// no room for it.
	let too_many: [&[u8]; 16] = [b"seed"; 16];
	assert_eq!(try_find_program_address(&too_many, &VAULT_PROGRAM), None);
}

#[test]
#[allow(deprecated)]
#[should_panic(expected = "could not find program address from seeds")]
fn find_program_address_panics_for_invalid_seeds() {
	let too_long = [7_u8; pina::MAX_SEED_LEN + 1];
	let _ = pina::find_program_address(&[&too_long], &VAULT_PROGRAM);
}

/// The associated token addresses of one wallet and mint under each token
/// program, derived independently with `getProgramDerivedAddress` from
/// `@solana/kit` over the seeds wallet, token program, mint.
#[cfg(feature = "token")]
#[test]
fn associated_token_addresses_match_independent_derivations() {
	let wallet = pina::address!("4Nd1mL5g7dUvNbKQjnYQgQki71RJKVQ1BM8DT6vKrrf5");
	let mint = pina::address!("CktRuQ2mttxyPjdvVSxGJySLjeRGna43E77gzHu6HotE");
	let legacy = pina::address!("6tXvgeoHarRaepCJVhKSHJWjawYJ7Htto7mswUTNqdDD");
	let token_2022 = pina::address!("Pvsbbj8RbipfnRsAdrWw6BhaiJbVAney3kvgPAjCTqu");

	assert_eq!(
		pina::try_get_associated_token_address(&wallet, &mint, &pina::token::ID),
		Some((legacy, 255))
	);
	assert_eq!(
		pina::try_get_associated_token_address(&wallet, &mint, &pina::token_2022::ID),
		Some((token_2022, 253))
	);

	// The wallet comes first in the seeds, so swapping it with the mint names
	// a different account.
	let swapped = pina::try_get_associated_token_address(&mint, &wallet, &pina::token::ID)
		.unwrap_or_else(|| panic!("swapped seeds should still derive an address"));
	assert_ne!(swapped.0, legacy);
}
