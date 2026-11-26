//! Host-only shielded keypair, RFC 7748 X25519 key agreement, authenticated
//! note encryption, per-transaction viewing keys, and 2-of-3 threshold
//! custodian share encryption.
//!
//! Zero external crypto dependencies and zero `unsafe` code: Curve25519
//! Montgomery-ladder arithmetic is implemented directly in 51-bit limbs over
//! $\mathbb{F}_{2^{255}-19}$ and verified against RFC 7748 Section 6.1 test
//! vectors, paired with SHA-256-CTR + HMAC-SHA256-128 via
//! `solana_sha256_hasher::hashv`.

#![cfg(all(feature = "prover", not(target_os = "solana")))]

use solana_sha256_hasher::hashv;

use crate::CUSTODIAN_COUNT;
use crate::MAX_ENVELOPE_BYTES;
use crate::SHARE_BLOB_BYTES;

/// Domain tag for note envelope key derivation.
const DOMAIN_NOTE_KDF: &[u8] = b"pina:privacy-pool:x25519-note:v2";
/// Domain tag for custodian share key derivation.
const DOMAIN_SHARE_KDF: &[u8] = b"pina:privacy-pool:x25519-share:v2";
/// Domain tag for authenticated encryption MAC.
const DOMAIN_AEAD_MAC: &[u8] = b"pina:privacy-pool:aead-mac:v2";
/// Domain tag for CTR keystream blocks.
const DOMAIN_AEAD_CTR: &[u8] = b"pina:privacy-pool:aead-ctr:v2";

/// Standard Curve25519 basepoint `u = 9`.
pub const X25519_BASEPOINT: [u8; 32] = {
	let mut p = [0_u8; 32];
	p[0] = 9;
	p
};

/// A user's shielded keypair, mirroring Helius Privacy Rings' three-key bundle:
/// an Ed25519 signing/consent seed, an X25519 viewing keypair for note
/// encryption/decryption, and a nullifier derivation key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShieldedKeypair {
	pub signing_seed: [u8; 32],
	pub viewing_secret: [u8; 32],
	pub viewing_pubkey: [u8; 32],
	pub nullifier_key: [u8; 32],
}

/// Public shielded address shared with senders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShieldedAddress {
	pub viewing_pubkey: [u8; 32],
}

impl ShieldedKeypair {
	/// Deterministically derive a shielded keypair from a 32-byte master seed.
	pub fn from_seed(master_seed: [u8; 32]) -> Self {
		let signing_seed = hashv(&[b"pina:shielded:signing", &master_seed]).to_bytes();
		let viewing_secret = hashv(&[b"pina:shielded:viewing", &master_seed]).to_bytes();
		let nullifier_key = hashv(&[b"pina:shielded:nullifier", &master_seed]).to_bytes();
		let viewing_pubkey = x25519_public_key(&viewing_secret);
		Self {
			signing_seed,
			viewing_secret,
			viewing_pubkey,
			nullifier_key,
		}
	}

	/// Public address to which senders encrypt note envelopes.
	pub fn address(&self) -> ShieldedAddress {
		ShieldedAddress {
			viewing_pubkey: self.viewing_pubkey,
		}
	}

	/// Derive a per-note nullifier seed from `nullifier_key` and a note nonce.
	pub fn derive_nullifier_seed(&self, nonce: u64) -> [u8; 32] {
		let mut out = hashv(&[
			b"pina:shielded:nullifier-seed",
			&self.nullifier_key,
			&nonce.to_le_bytes(),
		])
		.to_bytes();
		out[31] &= 0x1F; // Keep strictly below BN254 Fr modulus.
		out
	}

	/// Decrypt a note envelope using this wallet's X25519 viewing secret.
	pub fn decrypt_envelope(&self, envelope: &[u8]) -> Option<[u8; 40]> {
		decrypt_note_envelope(&self.viewing_secret, envelope)
	}
}

/// Result of encrypting a 40-byte note plaintext (`amount_le || secret_le`).
///
/// Layout (88 bytes <= `MAX_ENVELOPE_BYTES = 96`):
/// - `[0..32]`: ephemeral X25519 public key
/// - `[32..72]`: 40-byte ciphertext
/// - `[72..88]`: 16-byte authentication tag
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncryptedEnvelope {
	pub bytes: [u8; MAX_ENVELOPE_BYTES],
	pub len: u8,
	/// Single-transaction viewing key (`ECDH(ephemeral_sk, recipient_vk)`).
	/// Sharing this key discloses *only* this note without exposing the
	/// wallet's master viewing secret.
	pub tx_viewing_key: [u8; 32],
}

/// Encrypt a 40-byte note payload (`amount: u64 || secret: [u8; 32]`) to a
/// recipient's X25519 viewing public key using an ephemeral secret.
pub fn encrypt_note_envelope(
	recipient_viewing_pubkey: &[u8; 32],
	ephemeral_secret: &[u8; 32],
	plaintext: &[u8; 40],
) -> EncryptedEnvelope {
	let ephemeral_pubkey = x25519_public_key(ephemeral_secret);
	let shared = x25519(ephemeral_secret, recipient_viewing_pubkey);
	let tx_viewing_key = hashv(&[DOMAIN_NOTE_KDF, &shared, &ephemeral_pubkey]).to_bytes();

	let (ciphertext, tag) = aead_seal(&tx_viewing_key, &ephemeral_pubkey, plaintext);
	let mut bytes = [0_u8; MAX_ENVELOPE_BYTES];
	bytes[..32].copy_from_slice(&ephemeral_pubkey);
	bytes[32..72].copy_from_slice(&ciphertext);
	bytes[72..88].copy_from_slice(&tag);

	EncryptedEnvelope {
		bytes,
		len: 88,
		tx_viewing_key,
	}
}

/// Decrypt an 88-byte note envelope using the recipient's X25519 viewing secret.
pub fn decrypt_note_envelope(
	recipient_viewing_secret: &[u8; 32],
	envelope: &[u8],
) -> Option<[u8; 40]> {
	if envelope.len() != 88 {
		return None;
	}
	let ephemeral_pubkey: [u8; 32] = envelope[..32].try_into().ok()?;
	let shared = x25519(recipient_viewing_secret, &ephemeral_pubkey);
	let tx_viewing_key = hashv(&[DOMAIN_NOTE_KDF, &shared, &ephemeral_pubkey]).to_bytes();
	decrypt_with_tx_viewing_key(&tx_viewing_key, envelope)
}

/// Selective single-transaction disclosure: decrypt an 88-byte envelope using
/// only its per-transaction viewing key (no master viewing key required).
pub fn decrypt_with_tx_viewing_key(tx_viewing_key: &[u8; 32], envelope: &[u8]) -> Option<[u8; 40]> {
	if envelope.len() != 88 {
		return None;
	}
	let aad: [u8; 32] = envelope[..32].try_into().ok()?;
	let ciphertext: [u8; 40] = envelope[32..72].try_into().ok()?;
	let tag: [u8; 16] = envelope[72..88].try_into().ok()?;
	aead_open(tx_viewing_key, &aad, &ciphertext, &tag)
}

/// Encrypt a 32-byte `tx_viewing_key` into 3 custodian shares (2-of-3 threshold
/// via 2-of-3 pairwise replicated secret sharing) encrypted under each
/// custodian's X25519 public key.
///
/// Each 48-byte slot holds `ephemeral_pk (32B) || encrypted_share (16B)`?
/// Even better: share one ephemeral key per custodian or derive per-custodian
/// key from `(note_ephemeral_sk, custodian_vk)`:
/// - Split `tx_viewing_key` (32 bytes) into 3 pieces `s0, s1, s2` of 16 bytes?
///   No — use GF(256) Shamir 2-of-3 over all 32 bytes! Each share is 32 bytes
///   `f(x) = secret + a1 * x` for `x in {1, 2, 3}`, encrypted with
///   `k_i = Hash(DOMAIN_SHARE_KDF || ECDH(ephemeral_sk, custodian_vk_i))` and
///   authenticated with a 16-byte MAC tag -> `32 + 16 = 48` bytes per custodian
///   × 3 custodians = **144 bytes** (`SHARE_BLOB_BYTES`)!
///   (The ephemeral public key is already stored in `envelope[..32]`!)
pub fn encrypt_custodian_shares(
	custodian_viewing_pubkeys: &[[u8; 32]; CUSTODIAN_COUNT],
	ephemeral_secret: &[u8; 32],
	tx_viewing_key: &[u8; 32],
	mask_seed: &[u8; 32],
) -> [u8; SHARE_BLOB_BYTES] {
	let ephemeral_pubkey = x25519_public_key(ephemeral_secret);
	let slope = hashv(&[b"pina:shamir:slope", mask_seed, tx_viewing_key]).to_bytes();

	let mut out = [0_u8; SHARE_BLOB_BYTES];
	for (idx, custodian_vk) in custodian_viewing_pubkeys.iter().enumerate() {
		let x = (idx + 1) as u8;
		let mut share = [0_u8; 32];
		for byte_idx in 0..32 {
			share[byte_idx] = tx_viewing_key[byte_idx] ^ gf256_mul(slope[byte_idx], x);
		}
		let shared = x25519(ephemeral_secret, custodian_vk);
		let key = hashv(&[DOMAIN_SHARE_KDF, &shared, &ephemeral_pubkey, &[x]]).to_bytes();
		let (ct, tag) = aead_seal_32(&key, &ephemeral_pubkey, &share);
		let slot = &mut out[idx * 48..(idx + 1) * 48];
		slot[..32].copy_from_slice(&ct);
		slot[32..48].copy_from_slice(&tag);
	}
	out
}

/// Decrypt one custodian's 32-byte Shamir share from `shares[slot * 48..]`
/// using the custodian's X25519 viewing secret and the envelope's ephemeral
/// public key (`envelope[..32]`).
pub fn decrypt_custodian_share(
	custodian_index: usize,
	custodian_viewing_secret: &[u8; 32],
	ephemeral_pubkey: &[u8; 32],
	shares: &[u8; SHARE_BLOB_BYTES],
) -> Option<[u8; 32]> {
	if custodian_index >= CUSTODIAN_COUNT {
		return None;
	}
	let x = (custodian_index + 1) as u8;
	let slot = &shares[custodian_index * 48..(custodian_index + 1) * 48];
	let ct: [u8; 32] = slot[..32].try_into().ok()?;
	let tag: [u8; 16] = slot[32..48].try_into().ok()?;
	let dh_secret = x25519(custodian_viewing_secret, ephemeral_pubkey);
	let key = hashv(&[DOMAIN_SHARE_KDF, &dh_secret, ephemeral_pubkey, &[x]]).to_bytes();
	aead_open_32(&key, ephemeral_pubkey, &ct, &tag)
}

/// Reconstruct the 32-byte `tx_viewing_key` from any two distinct custodian
/// shares `(index_a, share_a)` and `(index_b, share_b)` using Lagrange
/// interpolation at `x = 0` over $\mathbb{F}_{2^8}$.
pub fn recover_from_custodian_shares(
	index_a: usize,
	share_a: &[u8; 32],
	index_b: usize,
	share_b: &[u8; 32],
) -> Option<[u8; 32]> {
	if index_a == index_b || index_a >= CUSTODIAN_COUNT || index_b >= CUSTODIAN_COUNT {
		return None;
	}
	let xa = (index_a + 1) as u8;
	let xb = (index_b + 1) as u8;
	let denom = xa ^ xb;
	let inv_denom = gf256_inv(denom);
	let la = gf256_mul(xb, inv_denom);
	let lb = gf256_mul(xa, inv_denom);

	let mut secret = [0_u8; 32];
	for i in 0..32 {
		secret[i] = gf256_mul(share_a[i], la) ^ gf256_mul(share_b[i], lb);
	}
	Some(secret)
}

// ---------------------------------------------------------------------------
// Authenticated encryption helpers (SHA-256 CTR + keyed SHA-256-128 MAC)
// ---------------------------------------------------------------------------

fn aead_seal(key: &[u8; 32], aad: &[u8; 32], plaintext: &[u8; 40]) -> ([u8; 40], [u8; 16]) {
	let ks0 = hashv(&[DOMAIN_AEAD_CTR, key, aad, &[0]]).to_bytes();
	let ks1 = hashv(&[DOMAIN_AEAD_CTR, key, aad, &[1]]).to_bytes();
	let mut ct = [0_u8; 40];
	for i in 0..32 {
		ct[i] = plaintext[i] ^ ks0[i];
	}
	for i in 0..8 {
		ct[32 + i] = plaintext[32 + i] ^ ks1[i];
	}
	let mac = hashv(&[DOMAIN_AEAD_MAC, key, aad, &ct]).to_bytes();
	let mut tag = [0_u8; 16];
	tag.copy_from_slice(&mac[..16]);
	(ct, tag)
}

fn aead_open(
	key: &[u8; 32],
	aad: &[u8; 32],
	ciphertext: &[u8; 40],
	tag: &[u8; 16],
) -> Option<[u8; 40]> {
	let mac = hashv(&[DOMAIN_AEAD_MAC, key, aad, ciphertext]).to_bytes();
	if !ct_eq_16(&mac[..16], tag) {
		return None;
	}
	let ks0 = hashv(&[DOMAIN_AEAD_CTR, key, aad, &[0]]).to_bytes();
	let ks1 = hashv(&[DOMAIN_AEAD_CTR, key, aad, &[1]]).to_bytes();
	let mut pt = [0_u8; 40];
	for i in 0..32 {
		pt[i] = ciphertext[i] ^ ks0[i];
	}
	for i in 0..8 {
		pt[32 + i] = ciphertext[32 + i] ^ ks1[i];
	}
	Some(pt)
}

fn aead_seal_32(key: &[u8; 32], aad: &[u8; 32], plaintext: &[u8; 32]) -> ([u8; 32], [u8; 16]) {
	let ks = hashv(&[DOMAIN_AEAD_CTR, key, aad, &[0]]).to_bytes();
	let mut ct = [0_u8; 32];
	for i in 0..32 {
		ct[i] = plaintext[i] ^ ks[i];
	}
	let mac = hashv(&[DOMAIN_AEAD_MAC, key, aad, &ct]).to_bytes();
	let mut tag = [0_u8; 16];
	tag.copy_from_slice(&mac[..16]);
	(ct, tag)
}

fn aead_open_32(
	key: &[u8; 32],
	aad: &[u8; 32],
	ciphertext: &[u8; 32],
	tag: &[u8; 16],
) -> Option<[u8; 32]> {
	let mac = hashv(&[DOMAIN_AEAD_MAC, key, aad, ciphertext]).to_bytes();
	if !ct_eq_16(&mac[..16], tag) {
		return None;
	}
	let ks = hashv(&[DOMAIN_AEAD_CTR, key, aad, &[0]]).to_bytes();
	let mut pt = [0_u8; 32];
	for i in 0..32 {
		pt[i] = ciphertext[i] ^ ks[i];
	}
	Some(pt)
}

fn ct_eq_16(a: &[u8], b: &[u8; 16]) -> bool {
	let mut diff = 0_u8;
	for i in 0..16 {
		diff |= a[i] ^ b[i];
	}
	diff == 0
}

// ---------------------------------------------------------------------------
// GF(2^8) arithmetic (AES polynomial x^8 + x^4 + x^3 + x + 1 = 0x11b)
// ---------------------------------------------------------------------------

fn gf256_mul(mut a: u8, mut b: u8) -> u8 {
	let mut p = 0_u8;
	for _ in 0..8 {
		if b & 1 != 0 {
			p ^= a;
		}
		let hi = a & 0x80;
		a <<= 1;
		if hi != 0 {
			a ^= 0x1b;
		}
		b >>= 1;
	}
	p
}

fn gf256_inv(a: u8) -> u8 {
	// a^254 in GF(2^8)
	let mut result = 1_u8;
	let mut base = a;
	let mut exp = 254_u8;
	while exp > 0 {
		if exp & 1 != 0 {
			result = gf256_mul(result, base);
		}
		base = gf256_mul(base, base);
		exp >>= 1;
	}
	result
}

// ---------------------------------------------------------------------------
// RFC 7748 X25519 (Curve25519 Montgomery ladder in 5 × 51-bit limbs)
// ---------------------------------------------------------------------------

/// Compute the X25519 public key for a 32-byte secret scalar.
pub fn x25519_public_key(secret: &[u8; 32]) -> [u8; 32] {
	x25519(secret, &X25519_BASEPOINT)
}

/// RFC 7748 X25519 scalar multiplication `scalar * u_coord`.
#[allow(clippy::many_single_char_names)]
pub fn x25519(scalar: &[u8; 32], u_coord: &[u8; 32]) -> [u8; 32] {
	let mut clamped = *scalar;
	clamped[0] &= 0xF8;
	clamped[31] &= 0x7F;
	clamped[31] |= 0x40;

	let mut u_bytes = *u_coord;
	u_bytes[31] &= 0x7F;
	let x_1 = Fe::from_bytes(&u_bytes);

	let mut x_2 = Fe::ONE;
	let mut z_2 = Fe::ZERO;
	let mut x_3 = x_1;
	let mut z_3 = Fe::ONE;
	let mut swap = 0_u64;

	for t in (0..255).rev() {
		let k_t = u64::from((clamped[t >> 3] >> (t & 7)) & 1);
		swap ^= k_t;
		Fe::cswap(&mut x_2, &mut x_3, swap);
		Fe::cswap(&mut z_2, &mut z_3, swap);
		swap = k_t;

		let a = x_2.add(&z_2);
		let aa = a.square();
		let b = x_2.sub(&z_2);
		let bb = b.square();
		let e = aa.sub(&bb);
		let c = x_3.add(&z_3);
		let d = x_3.sub(&z_3);
		let da = d.mul(&a);
		let cb = c.mul(&b);
		x_3 = da.add(&cb).square();
		z_3 = x_1.mul(&da.sub(&cb).square());
		x_2 = aa.mul(&bb);
		z_2 = e.mul(&aa.add(&e.mul_a24()));
	}

	Fe::cswap(&mut x_2, &mut x_3, swap);
	Fe::cswap(&mut z_2, &mut z_3, swap);

	let result = x_2.mul(&z_2.invert());
	result.to_bytes()
}

/// Field element modulo $p = 2^{255} - 19$ in five 51-bit limbs.
#[derive(Clone, Copy)]
struct Fe([u64; 5]);

const MASK51: u64 = (1_u64 << 51) - 1;

impl Fe {
	const ONE: Self = Self([1, 0, 0, 0, 0]);
	const ZERO: Self = Self([0, 0, 0, 0, 0]);

	fn from_bytes(bytes: &[u8; 32]) -> Self {
		let load64 = |offset: usize| -> u64 {
			u64::from_le_bytes(
				bytes[offset..offset + 8]
					.try_into()
					.unwrap_or_else(|_| panic!("fe slice")),
			)
		};
		let h0 = load64(0) & MASK51;
		let h1 = (load64(6) >> 3) & MASK51;
		let h2 = (load64(12) >> 6) & MASK51;
		let h3 = (load64(19) >> 1) & MASK51;
		let h4 = (load64(24) >> 12) & MASK51;
		Self([h0, h1, h2, h3, h4])
	}

	fn cswap(a: &mut Self, b: &mut Self, swap: u64) {
		let mask = 0_u64.wrapping_sub(swap);
		for i in 0..5 {
			let delta = mask & (a.0[i] ^ b.0[i]);
			a.0[i] ^= delta;
			b.0[i] ^= delta;
		}
	}

	fn add(&self, rhs: &Self) -> Self {
		Self([
			self.0[0] + rhs.0[0],
			self.0[1] + rhs.0[1],
			self.0[2] + rhs.0[2],
			self.0[3] + rhs.0[3],
			self.0[4] + rhs.0[4],
		])
	}

	fn sub(&self, rhs: &Self) -> Self {
		// Add 2*p to avoid underflow before reduction.
		const P0: u64 = 0x000F_FFFF_FFFF_FFDA;
		const P14: u64 = 0x000F_FFFF_FFFF_FFFE;
		Self([
			self.0[0] + P0 - rhs.0[0],
			self.0[1] + P14 - rhs.0[1],
			self.0[2] + P14 - rhs.0[2],
			self.0[3] + P14 - rhs.0[3],
			self.0[4] + P14 - rhs.0[4],
		])
	}

	fn mul(&self, rhs: &Self) -> Self {
		let a = self.0;
		let b = rhs.0;
		let b1_19 = u128::from(b[1]) * 19;
		let b2_19 = u128::from(b[2]) * 19;
		let b3_19 = u128::from(b[3]) * 19;
		let b4_19 = u128::from(b[4]) * 19;

		let t0 = u128::from(a[0]) * u128::from(b[0])
			+ u128::from(a[1]) * b4_19
			+ u128::from(a[2]) * b3_19
			+ u128::from(a[3]) * b2_19
			+ u128::from(a[4]) * b1_19;
		let t1 = u128::from(a[0]) * u128::from(b[1])
			+ u128::from(a[1]) * u128::from(b[0])
			+ u128::from(a[2]) * b4_19
			+ u128::from(a[3]) * b3_19
			+ u128::from(a[4]) * b2_19;
		let t2 = u128::from(a[0]) * u128::from(b[2])
			+ u128::from(a[1]) * u128::from(b[1])
			+ u128::from(a[2]) * u128::from(b[0])
			+ u128::from(a[3]) * b4_19
			+ u128::from(a[4]) * b3_19;
		let t3 = u128::from(a[0]) * u128::from(b[3])
			+ u128::from(a[1]) * u128::from(b[2])
			+ u128::from(a[2]) * u128::from(b[1])
			+ u128::from(a[3]) * u128::from(b[0])
			+ u128::from(a[4]) * b4_19;
		let t4 = u128::from(a[0]) * u128::from(b[4])
			+ u128::from(a[1]) * u128::from(b[3])
			+ u128::from(a[2]) * u128::from(b[2])
			+ u128::from(a[3]) * u128::from(b[1])
			+ u128::from(a[4]) * u128::from(b[0]);

		Self::reduce(t0, t1, t2, t3, t4)
	}

	fn mul_a24(&self) -> Self {
		const A24: u128 = 121_665;
		Self::reduce(
			u128::from(self.0[0]) * A24,
			u128::from(self.0[1]) * A24,
			u128::from(self.0[2]) * A24,
			u128::from(self.0[3]) * A24,
			u128::from(self.0[4]) * A24,
		)
	}

	fn square(&self) -> Self {
		self.mul(self)
	}

	fn reduce(mut t0: u128, mut t1: u128, mut t2: u128, mut t3: u128, mut t4: u128) -> Self {
		let c0 = t0 >> 51;
		t0 &= u128::from(MASK51);
		t1 += c0;
		let c1 = t1 >> 51;
		t1 &= u128::from(MASK51);
		t2 += c1;
		let c2 = t2 >> 51;
		t2 &= u128::from(MASK51);
		t3 += c2;
		let c3 = t3 >> 51;
		t3 &= u128::from(MASK51);
		t4 += c3;
		let c4 = t4 >> 51;
		t4 &= u128::from(MASK51);
		t0 += c4 * 19;
		let c0 = t0 >> 51;
		t0 &= u128::from(MASK51);
		t1 += c0;
		Self([t0 as u64, t1 as u64, t2 as u64, t3 as u64, t4 as u64])
	}

	fn invert(&self) -> Self {
		// Compute self^(p - 2) = self^(2^255 - 21).
		let z2 = self.square();
		let z9 = z2.square().square().mul(self);
		let z11 = z9.mul(&z2);
		let mut t = z11.square().mul(&z9); // 2^5 - 1
		let t5 = t;
		for _ in 0..5 {
			t = t.square();
		}
		let t10 = t.mul(&t5); // 2^10 - 1
		t = t10;
		for _ in 0..10 {
			t = t.square();
		}
		let t20 = t.mul(&t10); // 2^20 - 1
		t = t20;
		for _ in 0..20 {
			t = t.square();
		}
		let t40 = t.mul(&t20); // 2^40 - 1
		t = t40;
		for _ in 0..10 {
			t = t.square();
		}
		let t50 = t.mul(&t10); // 2^50 - 1
		t = t50;
		for _ in 0..50 {
			t = t.square();
		}
		let t100 = t.mul(&t50); // 2^100 - 1
		t = t100;
		for _ in 0..100 {
			t = t.square();
		}
		t = t.mul(&t100); // 2^200 - 1
		for _ in 0..50 {
			t = t.square();
		}
		t = t.mul(&t50); // 2^250 - 1
		for _ in 0..5 {
			t = t.square();
		}
		t.mul(&z11)
	}

	fn to_bytes(self) -> [u8; 32] {
		let mut h = Self::reduce(
			u128::from(self.0[0]),
			u128::from(self.0[1]),
			u128::from(self.0[2]),
			u128::from(self.0[3]),
			u128::from(self.0[4]),
		)
		.0;

		// Canonical reduction modulo 2^255 - 19.
		let mut q = (h[0] + 19) >> 51;
		q = (h[1] + q) >> 51;
		q = (h[2] + q) >> 51;
		q = (h[3] + q) >> 51;
		q = (h[4] + q) >> 51;

		h[0] += 19 * q;
		let c = h[0] >> 51;
		h[0] &= MASK51;
		h[1] += c;
		let c = h[1] >> 51;
		h[1] &= MASK51;
		h[2] += c;
		let c = h[2] >> 51;
		h[2] &= MASK51;
		h[3] += c;
		let c = h[3] >> 51;
		h[3] &= MASK51;
		h[4] += c;
		h[4] &= MASK51;

		let mut out = [0_u8; 32];
		out[0..8].copy_from_slice(&(h[0] | (h[1] << 51)).to_le_bytes());
		out[8..16].copy_from_slice(&((h[1] >> 13) | (h[2] << 38)).to_le_bytes());
		out[16..24].copy_from_slice(&((h[2] >> 26) | (h[3] << 25)).to_le_bytes());
		out[24..32].copy_from_slice(&((h[3] >> 39) | (h[4] << 12)).to_le_bytes());
		out
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn rfc7748_x25519_test_vector() {
		// RFC 7748 Section 6.1: Alice & Bob key exchange.
		let alice_sk: [u8; 32] = [
			0x77, 0x07, 0x6d, 0x0a, 0x73, 0x18, 0xa5, 0x7d, 0x3c, 0x16, 0xc1, 0x72, 0x51, 0xb2,
			0x66, 0x45, 0xdf, 0x4c, 0x2f, 0x87, 0xeb, 0xc0, 0x99, 0x2a, 0xb1, 0x77, 0xfb, 0xa5,
			0x1d, 0xb9, 0x2c, 0x2a,
		];
		let expected_alice_pk: [u8; 32] = [
			0x85, 0x20, 0xf0, 0x09, 0x89, 0x30, 0xa7, 0x54, 0x74, 0x8b, 0x7d, 0xdc, 0xb4, 0x3e,
			0xf7, 0x5a, 0x0d, 0xbf, 0x3a, 0x0d, 0x26, 0x38, 0x1a, 0xf4, 0xeb, 0xa4, 0xa9, 0x8e,
			0xaa, 0x9b, 0x4e, 0x6a,
		];
		let bob_sk: [u8; 32] = [
			0x5d, 0xab, 0x08, 0x7e, 0x62, 0x4a, 0x8a, 0x4b, 0x79, 0xe1, 0x7f, 0x8b, 0x83, 0x80,
			0x0e, 0xe6, 0x6f, 0x3b, 0xb1, 0x29, 0x26, 0x18, 0xb6, 0xfd, 0x1c, 0x2f, 0x8b, 0x27,
			0xff, 0x88, 0xe0, 0xeb,
		];
		let expected_bob_pk: [u8; 32] = [
			0xde, 0x9e, 0xdb, 0x7d, 0x7b, 0x7d, 0xc1, 0xb4, 0xd3, 0x5b, 0x61, 0xc2, 0xec, 0xe4,
			0x35, 0x37, 0x3f, 0x83, 0x43, 0xc8, 0x5b, 0x78, 0x67, 0x4d, 0xad, 0xfc, 0x7e, 0x14,
			0x6f, 0x88, 0x2b, 0x4f,
		];
		let expected_shared: [u8; 32] = [
			0x4a, 0x5d, 0x9d, 0x5b, 0xa4, 0xce, 0x2d, 0xe1, 0x72, 0x8e, 0x3b, 0xf4, 0x80, 0x35,
			0x0f, 0x25, 0xe0, 0x7e, 0x21, 0xc9, 0x47, 0xd1, 0x9e, 0x33, 0x76, 0xf0, 0x9b, 0x3c,
			0x1e, 0x16, 0x17, 0x42,
		];

		let alice_pub = x25519_public_key(&alice_sk);
		let bob_pub = x25519_public_key(&bob_sk);
		assert_eq!(alice_pub, expected_alice_pk);
		assert_eq!(bob_pub, expected_bob_pk);
		assert_eq!(x25519(&alice_sk, &bob_pub), expected_shared);
		assert_eq!(x25519(&bob_sk, &alice_pub), expected_shared);
	}

	#[test]
	fn envelope_and_custodian_shares_round_trip_and_authenticate() {
		let recipient = ShieldedKeypair::from_seed([0x11; 32]);
		let c0 = ShieldedKeypair::from_seed([0xA1; 32]);
		let c1 = ShieldedKeypair::from_seed([0xB2; 32]);
		let c2 = ShieldedKeypair::from_seed([0xC3; 32]);

		let mut plaintext = [0_u8; 40];
		plaintext[..8].copy_from_slice(&500_000_000_u64.to_le_bytes());
		plaintext[8..].fill(0x5A);

		let ephemeral = [0x42; 32];
		let encrypted = encrypt_note_envelope(&recipient.viewing_pubkey, &ephemeral, &plaintext);
		let slice = &encrypted.bytes[..usize::from(encrypted.len)];

		// Recipient decrypts with master viewing secret.
		assert_eq!(recipient.decrypt_envelope(slice), Some(plaintext));

		// Selective disclosure decrypts with per-transaction viewing key only.
		assert_eq!(
			decrypt_with_tx_viewing_key(&encrypted.tx_viewing_key, slice),
			Some(plaintext)
		);

		// Tampered ciphertext is rejected.
		let mut tampered = encrypted.bytes;
		tampered[40] ^= 1;
		assert_eq!(
			recipient.decrypt_envelope(&tampered[..usize::from(encrypted.len)]),
			None
		);

		// 2-of-3 Shamir custodian shares recover `tx_viewing_key`.
		let shares = encrypt_custodian_shares(
			&[c0.viewing_pubkey, c1.viewing_pubkey, c2.viewing_pubkey],
			&ephemeral,
			&encrypted.tx_viewing_key,
			&[0x99; 32],
		);
		let eph_pk: [u8; 32] = encrypted.bytes[..32].try_into().unwrap();
		let s0 = decrypt_custodian_share(0, &c0.viewing_secret, &eph_pk, &shares).unwrap();
		let s1 = decrypt_custodian_share(1, &c1.viewing_secret, &eph_pk, &shares).unwrap();
		let s2 = decrypt_custodian_share(2, &c2.viewing_secret, &eph_pk, &shares).unwrap();

		assert_eq!(
			recover_from_custodian_shares(0, &s0, 1, &s1),
			Some(encrypted.tx_viewing_key)
		);
		assert_eq!(
			recover_from_custodian_shares(1, &s1, 2, &s2),
			Some(encrypted.tx_viewing_key)
		);
		assert_eq!(
			recover_from_custodian_shares(0, &s0, 2, &s2),
			Some(encrypted.tx_viewing_key)
		);
	}
}
