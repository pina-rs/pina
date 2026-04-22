//! Kani model-checking harnesses for pina_pod_primitives.
//!
//! These proofs verify that checked arithmetic operations, round-trip
//! conversions, and bitwise operations maintain critical invariants for
//! all possible inputs.
//!
//! Run with: `cargo kani --harness <name>` (or `kani` script via devenv)

#![cfg(kani)]

use pina_pod_primitives::*;

// ---------------------------------------------------------------------------
// Round-trip invariants
// ---------------------------------------------------------------------------

#[kani::proof]
fn pod_u16_roundtrip() {
	let val: u16 = kani::any();
	let pod = PodU16::from_primitive(val);
	assert_eq!(pod.get(), val);
}

#[kani::proof]
fn pod_i16_roundtrip() {
	let val: i16 = kani::any();
	let pod = PodI16::from_primitive(val);
	assert_eq!(pod.get(), val);
}

#[kani::proof]
fn pod_u32_roundtrip() {
	let val: u32 = kani::any();
	let pod = PodU32::from_primitive(val);
	assert_eq!(pod.get(), val);
}

#[kani::proof]
fn pod_i32_roundtrip() {
	let val: i32 = kani::any();
	let pod = PodI32::from_primitive(val);
	assert_eq!(pod.get(), val);
}

#[kani::proof]
fn pod_u64_roundtrip() {
	let val: u64 = kani::any();
	let pod = PodU64::from_primitive(val);
	assert_eq!(pod.get(), val);
}

#[kani::proof]
fn pod_i64_roundtrip() {
	let val: i64 = kani::any();
	let pod = PodI64::from_primitive(val);
	assert_eq!(pod.get(), val);
}

// ---------------------------------------------------------------------------
// Checked arithmetic: overflow must return None
// ---------------------------------------------------------------------------

#[kani::proof]
fn pod_u16_checked_add_matches_native() {
	let a: u16 = kani::any();
	let b: u16 = kani::any();
	let pod_result = PodU16::from(a).checked_add(b);
	let native_result = a.checked_add(b);
	match (pod_result, native_result) {
		(Some(p), Some(n)) => assert_eq!(p.get(), n),
		(None, None) => {}
		_ => panic!("checked_add mismatch"),
	}
}

#[kani::proof]
fn pod_u16_checked_sub_matches_native() {
	let a: u16 = kani::any();
	let b: u16 = kani::any();
	let pod_result = PodU16::from(a).checked_sub(b);
	let native_result = a.checked_sub(b);
	match (pod_result, native_result) {
		(Some(p), Some(n)) => assert_eq!(p.get(), n),
		(None, None) => {}
		_ => panic!("checked_sub mismatch"),
	}
}

#[kani::proof]
fn pod_u32_checked_mul_matches_native() {
	let a: u32 = kani::any();
	let b: u32 = kani::any();
	let pod_result = PodU32::from(a).checked_mul(b);
	let native_result = a.checked_mul(b);
	match (pod_result, native_result) {
		(Some(p), Some(n)) => assert_eq!(p.get(), n),
		(None, None) => {}
		_ => panic!("checked_mul mismatch"),
	}
}

#[kani::proof]
fn pod_u64_checked_div_matches_native() {
	let a: u64 = kani::any();
	let b: u64 = kani::any();
	let pod_result = PodU64::from(a).checked_div(b);
	let native_result = a.checked_div(b);
	match (pod_result, native_result) {
		(Some(p), Some(n)) => assert_eq!(p.get(), n),
		(None, None) => {}
		_ => panic!("checked_div mismatch"),
	}
}

// ---------------------------------------------------------------------------
// Saturating arithmetic clamps correctly
// ---------------------------------------------------------------------------

#[kani::proof]
fn pod_u16_saturating_add_matches_native() {
	let a: u16 = kani::any();
	let b: u16 = kani::any();
	let pod = PodU16::from(a).saturating_add(b);
	assert_eq!(pod.get(), a.saturating_add(b));
}

#[kani::proof]
fn pod_i16_saturating_sub_matches_native() {
	let a: i16 = kani::any();
	let b: i16 = kani::any();
	let pod = PodI16::from(a).saturating_sub(b);
	assert_eq!(pod.get(), a.saturating_sub(b));
}

// ---------------------------------------------------------------------------
// Bitwise operations match native
// ---------------------------------------------------------------------------

#[kani::proof]
fn pod_u32_bitand_matches_native() {
	let a: u32 = kani::any();
	let b: u32 = kani::any();
	let pod = PodU32::from(a) & b;
	assert_eq!(pod.get(), a & b);
}

#[kani::proof]
fn pod_u32_bitor_matches_native() {
	let a: u32 = kani::any();
	let b: u32 = kani::any();
	let pod = PodU32::from(a) | b;
	assert_eq!(pod.get(), a | b);
}

#[kani::proof]
fn pod_u32_bitxor_matches_native() {
	let a: u32 = kani::any();
	let b: u32 = kani::any();
	let pod = PodU32::from(a) ^ b;
	assert_eq!(pod.get(), a ^ b);
}

#[kani::proof]
fn pod_u32_not_matches_native() {
	let a: u32 = kani::any();
	let pod = !PodU32::from(a);
	assert_eq!(pod.get(), !a);
}

// ---------------------------------------------------------------------------
// Ordering matches native
// ---------------------------------------------------------------------------

#[kani::proof]
fn pod_u64_ordering_matches_native() {
	let a: u64 = kani::any();
	let b: u64 = kani::any();
	let pod_a = PodU64::from(a);
	let pod_b = PodU64::from(b);
	assert_eq!(pod_a.cmp(&pod_b), a.cmp(&b));
}

#[kani::proof]
fn pod_i64_ordering_matches_native() {
	let a: i64 = kani::any();
	let b: i64 = kani::any();
	let pod_a = PodI64::from(a);
	let pod_b = PodI64::from(b);
	assert_eq!(pod_a.cmp(&pod_b), a.cmp(&b));
}

// ---------------------------------------------------------------------------
// PodBool invariants
// ---------------------------------------------------------------------------

#[kani::proof]
fn pod_bool_from_bool_is_canonical() {
	let b: bool = kani::any();
	let pod = PodBool::from_bool(b);
	assert!(pod.is_canonical());
}

#[kani::proof]
fn pod_bool_not_matches_native() {
	let b: bool = kani::any();
	let pod = PodBool::from_bool(b);
	let not_pod = !pod;
	assert_eq!(bool::from(not_pod), !b);
}
