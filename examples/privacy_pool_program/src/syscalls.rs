//! Runtime syscall bindings for Poseidon and BN254 group operations.
//!
//! SBF-only. Pinocchio re-exports the raw extern declarations; this module
//! is the narrow safe surface over exactly the three selectors the
//! verifier needs: the Poseidon hash, big-endian G₁ addition and
//! multiplication, and the big-endian product-of-pairings check. The
//! big-endian (EIP-197) encoding is the one ecosystem Groth16 verifiers
//! standardized on: points arrive as big-endian affine coordinates
//! (`x.c1, x.c0, y.c1, y.c0` for G₂) and scalars as big-endian field
//! elements, and the only point negation happens host-side when the proof
//! is serialized — the on-chain equation needs none.
//!
//! Everything else in the workspace stays `unsafe`-free; the `unsafe` here
//! is confined to passing fixed-size stack buffers to extern "C"
//! functions whose contracts the callers satisfy by construction:
//!
//! - every buffer is a fixed-size array whose address is valid for the
//!   syscall's duration, with no aliasing between the input and output
//!   arguments of one call;
//! - every length argument is the array's compile-time length, so the
//!   syscalls never read or write past a buffer's end;
//! - return values are checked, and every error maps to a `ProgramError`.
//!
//! These are the same invariants `crates/pina/src/verification.rs` documents
//! for its own contained extern boundaries.

#![allow(unsafe_code)]
#![cfg(target_os = "solana")]

use pina::ProgramError;
use pina::pinocchio::syscalls::sol_alt_bn128_group_op;
use pina::pinocchio::syscalls::sol_poseidon;

use crate::PrivacyPoolError;

/// `sol_alt_bn128_group_op` selector for big-endian G₁ addition.
const OP_G1_ADD_BE: u64 = 0;
/// Selector for big-endian G₁ scalar multiplication.
const OP_G1_MUL_BE: u64 = 2;
/// Selector for the big-endian product-of-pairings check.
const OP_PAIRING_BE: u64 = 3;

/// Poseidon-2 over little-endian BN254 field elements, matching the circom
/// x5 parameters (state width 3) the runtime syscall implements.
pub fn poseidon2(left: &[u8; 32], right: &[u8; 32]) -> Result<[u8; 32], ProgramError> {
	// The syscall consumes an array of `SolBytes` (`ptr, len` pairs), which
	// is exactly the memory layout of a `&[&[u8]]` slice.
	let elements: [&[u8]; 2] = [left, right];
	let mut output = [0_u8; 32];
	// SAFETY: `elements` and `output` are stack values alive for the call's
	// duration; the element count is the slice's length, and the output
	// buffer's 32 bytes match the syscall's result width.
	let status = unsafe {
		sol_poseidon(
			0, // BN254 x5 curve parameters
			1, // little-endian
			elements.as_ptr() as *const u8,
			elements.len() as u64,
			output.as_mut_ptr(),
		)
	};
	if status != 0 {
		return Err(ProgramError::from(PrivacyPoolError::ArithmeticOverflow));
	}
	Ok(output)
}

/// Big-endian G₁ point addition over the syscall's group-op selector.
pub fn g1_add(left: &[u8; 64], right: &[u8; 64]) -> Result<[u8; 64], ProgramError> {
	let mut input = [0_u8; 128];
	input[..64].copy_from_slice(left);
	input[64..].copy_from_slice(right);
	let mut output = [0_u8; 64];
	group_op(OP_G1_ADD_BE, &input, &mut output)?;
	Ok(output)
}

/// Big-endian G₁ scalar multiplication: 64-byte point, 32-byte scalar.
pub fn g1_mul(point: &[u8; 64], scalar: &[u8; 32]) -> Result<[u8; 64], ProgramError> {
	let mut input = [0_u8; 96];
	input[..64].copy_from_slice(point);
	input[64..].copy_from_slice(scalar);
	let mut output = [0_u8; 64];
	group_op(OP_G1_MUL_BE, &input, &mut output)?;
	Ok(output)
}

/// Product-of-pairings check over four (G₁, G₂) pairs in big-endian
/// encoding. The input is 768 bytes: four 192-byte elements, each a
/// 64-byte G₁ point followed by a 128-byte G₂ point. Returns whether the
/// product equals one; the syscall writes a big-endian 32-byte result
/// whose final byte carries the verdict.
pub fn pairing_check(input: &[u8; 768]) -> Result<bool, ProgramError> {
	let mut output = [0_u8; 32];
	// SAFETY: `input` and `output` are fixed-size stack arrays; the length
	// argument equals the input array's length, which is exactly four
	// 192-byte pairing elements, and the result buffer matches the
	// syscall's 32-byte output width.
	let status = unsafe {
		sol_alt_bn128_group_op(
			OP_PAIRING_BE,
			input.as_ptr(),
			input.len() as u64,
			output.as_mut_ptr(),
		)
	};
	if status != 0 {
		return Err(ProgramError::from(
			PrivacyPoolError::ProofVerificationFailed,
		));
	}
	Ok(output[31] == 1 && output[..31].iter().all(|byte| *byte == 0))
}

/// Shared extern call for the G₁ selectors.
fn group_op(op: u64, input: &[u8], output: &mut [u8; 64]) -> Result<(), ProgramError> {
	// SAFETY: both buffers are fixed-size stack arrays, valid and
	// non-aliasing for the call, with lengths equal to their sizes.
	let status = unsafe {
		sol_alt_bn128_group_op(op, input.as_ptr(), input.len() as u64, output.as_mut_ptr())
	};
	if status != 0 {
		return Err(ProgramError::from(
			PrivacyPoolError::ProofVerificationFailed,
		));
	}
	Ok(())
}
