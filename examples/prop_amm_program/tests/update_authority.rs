//! Regression guards for the hard-coded update authority fixture.
//!
//! The example intentionally keeps a fixed, source-visible updater key (the
//! Anchor v2 benchmark shape), but the fixture key must not live inside the
//! brute-force space of uniform seeds. The original fixture derived its
//! keypair from `[7u8; 32]`, so anyone who read the public source rebuilt the
//! signing key — and with it full oracle price control — in at most 256
//! guesses (security sweep finding D1). These tests pin the exploit class
//! closed while keeping the fixture deterministic.

use prop_amm_program::UPDATE_AUTHORITY;
use solana_keypair::Keypair;
use solana_signer::Signer;

/// The committed fixture seed: the 32 ASCII bytes of a descriptive phrase, not
/// a repeated single byte. Must match the seed documented on
/// [`UPDATE_AUTHORITY`] in `src/lib.rs`.
const UPDATE_AUTHORITY_SEED: [u8; 32] = *b"pina example fixture updater key";

/// The committed constant is exactly the keypair of the committed seed, so
/// every suite can rebuild the fixture key deterministically.
#[test]
fn update_authority_is_the_keypair_of_the_committed_seed() {
	assert_eq!(
		Keypair::new_from_array(UPDATE_AUTHORITY_SEED).pubkey(),
		UPDATE_AUTHORITY,
		"UPDATE_AUTHORITY must stay derived from the committed fixture seed"
	);
}

/// No uniform seed may reproduce the fixture authority.
///
/// Sweeps every repeated-single-byte seed `[n; 32]` for `n` in `0..=255` and
/// rejects any that derives [`UPDATE_AUTHORITY`]. The original `[7u8; 32]`
/// fixture fails exactly here, which is what made its private key
/// recoverable; any future edit that moves the constant back into that space
/// fails this loop instead of shipping a guessable authority.
#[test]
fn update_authority_is_not_reproducible_from_a_uniform_seed() {
	for byte in 0u8..=255 {
		assert_ne!(
			Keypair::new_from_array([byte; 32]).pubkey(),
			UPDATE_AUTHORITY,
			"seed [{byte}; 32] reproduces UPDATE_AUTHORITY: the authority key is recoverable from \
			 a uniform seed again"
		);
	}
}
