//! Poseidon hashing and Groth16 verification, dispatched per target.
//!
//! On SBF both run through the runtime syscalls (`sol_poseidon`,
//! `sol_alt_bn128_group_op`) — see the `syscalls` module. On the host, where
//! tests and the `prover` feature execute the same program code, the
//! identical operations are computed with arkworks against the same circom
//! x5 parameters, so a host-generated proof verifies on-chain and a
//! host-predicted root matches the tree the program writes. Host builds
//! with neither `test` nor `prover` never execute these paths; they compile
//! to explicit `unimplemented` panics so the SBF artifact never carries a
//! field-arithmetic dependency.

use pina::ProgramError;

use crate::PrivacyPoolError;

/// Verified-key material as plain values, decoupled from any account view
/// so the verifier reads like the equation it checks.
pub struct Groth16Key {
	pub alpha_g1: [u8; 64],
	pub beta_g2: [u8; 128],
	pub gamma_g2: [u8; 128],
	pub delta_g2: [u8; 128],
	pub ic: [[u8; 64]; crate::MAX_PUBLIC_INPUTS],
	pub ic_len: usize,
}

/// Concatenate a G₁ point (64 bytes) and a G₂ point (128 bytes) into one
/// pairing input element.
pub fn pairing_element(g1: &[u8; 64], g2: &[u8; 128]) -> [u8; 192] {
	let mut element = [0_u8; 192];
	element[..64].copy_from_slice(g1);
	element[64..].copy_from_slice(g2);
	element
}

/// The zero-hash chain for empty subtrees: `zero[0] = 0`,
/// `zero[ℓ] = H(zero[ℓ−1], zero[ℓ−1])`.
pub fn zero_hashes() -> Result<[[u8; 32]; crate::TREE_DEPTH + 1], ProgramError> {
	let mut zeros = [[0_u8; 32]; crate::TREE_DEPTH + 1];
	for level in 1..=crate::TREE_DEPTH {
		zeros[level] = poseidon2(&zeros[level - 1], &zeros[level - 1])?;
	}
	Ok(zeros)
}

/// Verify a Groth16 proof against installed key material.
///
/// Checks `e(A, B) · e(−C, δ) · e(−IC(x), γ) · e(−α, β) = 1`, with
/// `IC(x) = ic₀ + Σ icᵢ₊₁ · xᵢ` accumulated through G₁ addition and
/// multiplication. All coordinates are little-endian affine.
pub fn verify_groth16(
	key: &Groth16Key,
	proof_a: &[u8; 64],
	proof_b: &[u8; 128],
	proof_c: &[u8; 64],
	public_inputs: &[[u8; 32]],
) -> Result<bool, ProgramError> {
	if key.ic_len == 0
		|| key.ic_len > crate::MAX_PUBLIC_INPUTS
		|| public_inputs.len() + 1 != key.ic_len
	{
		return Err(ProgramError::from(
			PrivacyPoolError::InvalidVerifyingKeySlot,
		));
	}

	// IC(x) = ic0 + ic1·x0 + ic2·x1 … Public inputs arrive little-endian
	// (the tree's storage order) and reverse into the big-endian scalar
	// words the multiplication selector consumes.
	let mut accumulator = key.ic[0];
	for (index, input) in public_inputs.iter().enumerate() {
		let mut scalar_be = [0_u8; 32];
		scalar_be.copy_from_slice(input);
		scalar_be.reverse();
		let product = g1_mul(&key.ic[index + 1], &scalar_be)?;
		accumulator = g1_add(&accumulator, &product)?;
	}

	// e(−A, B) · e(IC(x), γ) · e(C, δ) · e(α, β) = 1, the raw Groth16
	// equation with everything on one side. The A negation happened
	// host-side at proof serialization; on-chain pairing inputs are all
	// positive points.
	let mut pairing_input = [0_u8; 768];
	pairing_input[..192].copy_from_slice(&pairing_element(proof_a, proof_b));
	pairing_input[192..384].copy_from_slice(&pairing_element(&accumulator, &key.gamma_g2));
	pairing_input[384..576].copy_from_slice(&pairing_element(proof_c, &key.delta_g2));
	pairing_input[576..].copy_from_slice(&pairing_element(&key.alpha_g1, &key.beta_g2));
	pairing_check(&pairing_input)
}

#[cfg(any(target_os = "solana", test, feature = "prover"))]
pub use executable::g1_add;
#[cfg(any(target_os = "solana", test, feature = "prover"))]
pub use executable::g1_mul;
#[cfg(any(target_os = "solana", test, feature = "prover"))]
#[cfg(any(target_os = "solana", test, feature = "prover"))]
pub use executable::pairing_check;
#[cfg(any(target_os = "solana", test, feature = "prover"))]
pub use executable::poseidon2;
#[cfg(not(any(target_os = "solana", test, feature = "prover")))]
pub use stub::g1_add;
#[cfg(not(any(target_os = "solana", test, feature = "prover")))]
pub use stub::g1_mul;
#[cfg(not(any(target_os = "solana", test, feature = "prover")))]
#[cfg(not(any(target_os = "solana", test, feature = "prover")))]
pub use stub::pairing_check;
#[cfg(not(any(target_os = "solana", test, feature = "prover")))]
pub use stub::poseidon2;

/// Placeholder implementations for host builds without `test` or `prover`.
/// Nothing links these paths into an executed program; they exist so plain
/// `cargo check` and clippy see the same signatures without pulling the
/// arkworks dependency tree into the build graph.
#[cfg(not(any(target_os = "solana", test, feature = "prover")))]
mod stub {
	use pina::ProgramError;

	fn unreachable_host() -> ProgramError {
		ProgramError::from(crate::PrivacyPoolError::ArithmeticOverflow)
	}

	pub fn poseidon2(_left: &[u8; 32], _right: &[u8; 32]) -> Result<[u8; 32], ProgramError> {
		Err(unreachable_host())
	}

	pub fn g1_add(_left: &[u8; 64], _right: &[u8; 64]) -> Result<[u8; 64], ProgramError> {
		Err(unreachable_host())
	}

	pub fn g1_mul(_point: &[u8; 64], _scalar: &[u8; 32]) -> Result<[u8; 64], ProgramError> {
		Err(unreachable_host())
	}

	pub fn negate_g1(_point: &[u8; 64]) -> Result<[u8; 64], ProgramError> {
		Err(unreachable_host())
	}

	pub fn pairing_check(_input: &[u8; 768]) -> Result<bool, ProgramError> {
		Err(unreachable_host())
	}
}

/// Real implementations: the runtime syscalls on SBF, arkworks on host.
#[cfg(any(target_os = "solana", test, feature = "prover"))]
mod executable {
	use pina::ProgramError;

	#[cfg(target_os = "solana")]
	pub use crate::syscalls::g1_add;
	#[cfg(target_os = "solana")]
	pub use crate::syscalls::g1_mul;
	#[cfg(target_os = "solana")]
	#[cfg(target_os = "solana")]
	pub use crate::syscalls::pairing_check;
	#[cfg(target_os = "solana")]
	pub use crate::syscalls::poseidon2;

	/// Host implementation: arkworks against the same x5 parameters the
	/// syscall implements, so both sides agree byte for byte.
	#[cfg(not(target_os = "solana"))]
	pub(crate) mod host {
		extern crate alloc;

		use alloc::vec::Vec;

		use ark_bn254::Bn254;
		use ark_bn254::Fr;
		use ark_bn254::g1::G1Affine;
		use ark_bn254::g2::G2Affine;
		use ark_ec::AffineRepr;
		use ark_ec::pairing::Pairing;
		use ark_ff::BigInteger;
		use ark_ff::Field;
		use ark_ff::PrimeField;
		use ark_ff::Zero;
		use light_poseidon::PoseidonParameters;
		use light_poseidon::parameters::bn254_x5;

		use super::*;

		fn invalid() -> ProgramError {
			ProgramError::from(crate::PrivacyPoolError::ProofVerificationFailed)
		}

		fn parameters() -> PoseidonParameters<Fr> {
			bn254_x5::get_poseidon_parameters::<Fr>(3)
				.unwrap_or_else(|error| panic!("poseidon parameters: {error:?}"))
		}

		fn fr_from_le(bytes: &[u8; 32]) -> Option<Fr> {
			let limbs: [u64; 4] = core::array::from_fn(|index| {
				let mut limb = [0_u8; 8];
				limb.copy_from_slice(&bytes[index * 8..(index + 1) * 8]);
				u64::from_le_bytes(limb)
			});
			Fr::from_bigint(ark_ff::BigInteger256::new(limbs))
		}

		fn fr_to_le(value: &Fr) -> [u8; 32] {
			let mut out = [0_u8; 32];
			let bytes = value.into_bigint().to_bytes_le();
			out.copy_from_slice(&bytes);
			out
		}

		pub fn poseidon2(left: &[u8; 32], right: &[u8; 32]) -> Result<[u8; 32], ProgramError> {
			let l = fr_from_le(left).ok_or_else(invalid)?;
			let r = fr_from_le(right).ok_or_else(invalid)?;
			let params = parameters();
			let mut state = [Fr::zero(), l, r];
			let half = params.full_rounds / 2;
			let all_rounds = params.full_rounds + params.partial_rounds;
			for round in 0..all_rounds {
				for (element, slot) in state.iter_mut().enumerate().take(params.width) {
					*slot += params.ark[round * params.width + element];
				}
				if round < half || round >= half + params.partial_rounds {
					for slot in &mut state {
						*slot = slot.pow([params.alpha]);
					}
				} else {
					state[0] = state[0].pow([params.alpha]);
				}
				let previous = state;
				for row in 0..params.width {
					state[row] = (0..params.width).fold(Fr::zero(), |acc, column| {
						acc + previous[column] * params.mds[row][column]
					});
				}
			}
			Ok(fr_to_le(&state[0]))
		}

		fn fq_from_be(bytes: &[u8; 32]) -> Option<ark_bn254::Fq> {
			field_from_be::<ark_bn254::Fq>(bytes)
		}

		fn fr_from_be(bytes: &[u8; 32]) -> Option<Fr> {
			field_from_be::<Fr>(bytes)
		}

		fn field_from_be<F: PrimeField<BigInt = ark_ff::BigInteger256>>(
			bytes: &[u8; 32],
		) -> Option<F> {
			// Big-endian words: the first eight bytes are the most
			// significant limb, so chunk 0 lands in limb 3.
			let limbs: [u64; 4] = core::array::from_fn(|index| {
				let word: [u8; 8] = bytes[(3 - index) * 8..(4 - index) * 8]
					.try_into()
					.unwrap_or_else(|_| panic!("field word"));
				u64::from_be_bytes(word)
			});
			F::from_bigint(ark_ff::BigInteger256::new(limbs))
		}

		fn word(bytes: &[u8], start: usize) -> Option<[u8; 32]> {
			bytes
				.get(start..start + 32)
				.and_then(|slice| slice.try_into().ok())
		}

		fn g1_from_be(bytes: &[u8; 64]) -> Result<G1Affine, ProgramError> {
			let x = fq_from_be(&word(bytes, 0).ok_or_else(invalid)?);
			let y = fq_from_be(&word(bytes, 32).ok_or_else(invalid)?);
			let point = G1Affine::new(x.ok_or_else(invalid)?, y.ok_or_else(invalid)?);
			if !point.is_on_curve() || point.is_zero() {
				return Err(invalid());
			}
			Ok(point)
		}

		/// G₂ words arrive as `x.c1, x.c0, y.c1, y.c0`, each big-endian —
		/// the EIP-197 layout the BE syscall selectors consume.
		fn g2_from_be(bytes: &[u8; 128]) -> Result<G2Affine, ProgramError> {
			let component = |offset: usize| -> Result<ark_bn254::Fq2, ProgramError> {
				let c1 = fq_from_be(&word(bytes, offset).ok_or_else(invalid)?);
				let c0 = fq_from_be(&word(bytes, offset + 32).ok_or_else(invalid)?);
				Ok(ark_bn254::Fq2::new(
					c0.ok_or_else(invalid)?,
					c1.ok_or_else(invalid)?,
				))
			};
			let point = G2Affine::new(component(0)?, component(64)?);
			if !point.is_on_curve() || point.is_zero() {
				return Err(invalid());
			}
			Ok(point)
		}

		fn g1_to_be(point: &G1Affine) -> [u8; 64] {
			let mut out = [0_u8; 64];
			if !point.is_zero() {
				out[..32]
					.copy_from_slice(&point.x().unwrap_or_default().into_bigint().to_bytes_be());
				out[32..]
					.copy_from_slice(&point.y().unwrap_or_default().into_bigint().to_bytes_be());
			}
			out
		}

		pub fn g1_add(left: &[u8; 64], right: &[u8; 64]) -> Result<[u8; 64], ProgramError> {
			let sum = (g1_from_be(left)? + g1_from_be(right)?).into();
			Ok(g1_to_be(&sum))
		}

		pub fn g1_mul(point: &[u8; 64], scalar: &[u8; 32]) -> Result<[u8; 64], ProgramError> {
			let scalar = fr_from_be(scalar).ok_or_else(invalid)?;
			let product: G1Affine = (g1_from_be(point)? * scalar).into();
			Ok(g1_to_be(&product))
		}

		pub fn pairing_check(input: &[u8; 768]) -> Result<bool, ProgramError> {
			let mut pairs = Vec::with_capacity(4);
			for index in 0..4 {
				let element = &input[index * 192..(index + 1) * 192];
				let mut g1 = [0_u8; 64];
				let mut g2 = [0_u8; 128];
				g1.copy_from_slice(&element[..64]);
				g2.copy_from_slice(&element[64..]);
				pairs.push((g1_from_be(&g1)?, g2_from_be(&g2)?));
			}
			// PairingOutput derives PartialEq; compare against one directly.
			let g1_prepared: Vec<<Bn254 as Pairing>::G1Prepared> =
				pairs.iter().map(|(g1, _)| (*g1).into()).collect();
			let g2_prepared: Vec<<Bn254 as Pairing>::G2Prepared> =
				pairs.iter().map(|(_, g2)| (*g2).into()).collect();
			let product = Bn254::multi_pairing(g1_prepared, g2_prepared);
			Ok(product.0 == <Bn254 as Pairing>::TargetField::ONE)
		}
	}

	#[cfg(not(target_os = "solana"))]
	pub use host::g1_add;
	#[cfg(not(target_os = "solana"))]
	pub use host::g1_mul;
	#[cfg(not(target_os = "solana"))]
	#[cfg(not(target_os = "solana"))]
	pub use host::pairing_check;
	#[cfg(not(target_os = "solana"))]
	pub use host::poseidon2;
}
