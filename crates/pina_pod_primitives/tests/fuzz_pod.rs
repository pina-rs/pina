//! Property-based fuzz tests for Pod type deserialization, round-trip
//! correctness, and safety under arbitrary byte patterns.

use bytemuck::try_from_bytes;
use pina_pod_primitives::PodBool;
use pina_pod_primitives::PodI16;
use pina_pod_primitives::PodI32;
use pina_pod_primitives::PodI64;
use pina_pod_primitives::PodI128;
use pina_pod_primitives::PodU16;
use pina_pod_primitives::PodU32;
use pina_pod_primitives::PodU64;
use pina_pod_primitives::PodU128;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Round-trip: from_primitive(x).into() == x
// ---------------------------------------------------------------------------

proptest! {
	#[test]
	fn roundtrip_u16(val: u16) {
		let pod = PodU16::from_primitive(val);
		let back: u16 = pod.into();
		prop_assert_eq!(back, val);
	}

	#[test]
	fn roundtrip_i16(val: i16) {
		let pod = PodI16::from_primitive(val);
		let back: i16 = pod.into();
		prop_assert_eq!(back, val);
	}

	#[test]
	fn roundtrip_u32(val: u32) {
		let pod = PodU32::from_primitive(val);
		let back: u32 = pod.into();
		prop_assert_eq!(back, val);
	}

	#[test]
	fn roundtrip_i32(val: i32) {
		let pod = PodI32::from_primitive(val);
		let back: i32 = pod.into();
		prop_assert_eq!(back, val);
	}

	#[test]
	fn roundtrip_u64(val: u64) {
		let pod = PodU64::from_primitive(val);
		let back: u64 = pod.into();
		prop_assert_eq!(back, val);
	}

	#[test]
	fn roundtrip_i64(val: i64) {
		let pod = PodI64::from_primitive(val);
		let back: i64 = pod.into();
		prop_assert_eq!(back, val);
	}

	#[test]
	fn roundtrip_u128(val: u128) {
		let pod = PodU128::from_primitive(val);
		let back: u128 = pod.into();
		prop_assert_eq!(back, val);
	}

	#[test]
	fn roundtrip_i128(val: i128) {
		let pod = PodI128::from_primitive(val);
		let back: i128 = pod.into();
		prop_assert_eq!(back, val);
	}
}

// ---------------------------------------------------------------------------
// Arbitrary byte patterns safely interpreted as Pod types via bytemuck
// ---------------------------------------------------------------------------

proptest! {
	#[test]
	fn arbitrary_bytes_as_pod_bool(byte: u8) {
		let bytes = [byte];
		let result = try_from_bytes::<PodBool>(&bytes);
		// Must always succeed — PodBool is repr(transparent) over u8.
		let pod = result.unwrap_or_else(|e| panic!("try_from_bytes failed for byte {byte}: {e}"));
		// Conversion to bool must never panic.
		let _: bool = (*pod).into();
	}

	#[test]
	fn arbitrary_bytes_as_pod_u16(bytes: [u8; 2]) {
		let result = try_from_bytes::<PodU16>(&bytes);
		let pod = result.unwrap_or_else(|e| {
			panic!("try_from_bytes failed for bytes {bytes:?}: {e}")
		});
		let _: u16 = (*pod).into();
	}

	#[test]
	fn arbitrary_bytes_as_pod_i16(bytes: [u8; 2]) {
		let result = try_from_bytes::<PodI16>(&bytes);
		let pod = result.unwrap_or_else(|e| {
			panic!("try_from_bytes failed for bytes {bytes:?}: {e}")
		});
		let _: i16 = (*pod).into();
	}

	#[test]
	fn arbitrary_bytes_as_pod_u32(bytes: [u8; 4]) {
		let result = try_from_bytes::<PodU32>(&bytes);
		let pod = result.unwrap_or_else(|e| {
			panic!("try_from_bytes failed for bytes {bytes:?}: {e}")
		});
		let _: u32 = (*pod).into();
	}

	#[test]
	fn arbitrary_bytes_as_pod_i32(bytes: [u8; 4]) {
		let result = try_from_bytes::<PodI32>(&bytes);
		let pod = result.unwrap_or_else(|e| {
			panic!("try_from_bytes failed for bytes {bytes:?}: {e}")
		});
		let _: i32 = (*pod).into();
	}

	#[test]
	fn arbitrary_bytes_as_pod_u64(bytes: [u8; 8]) {
		let result = try_from_bytes::<PodU64>(&bytes);
		let pod = result.unwrap_or_else(|e| {
			panic!("try_from_bytes failed for bytes {bytes:?}: {e}")
		});
		let _: u64 = (*pod).into();
	}

	#[test]
	fn arbitrary_bytes_as_pod_i64(bytes: [u8; 8]) {
		let result = try_from_bytes::<PodI64>(&bytes);
		let pod = result.unwrap_or_else(|e| {
			panic!("try_from_bytes failed for bytes {bytes:?}: {e}")
		});
		let _: i64 = (*pod).into();
	}

	#[test]
	fn arbitrary_bytes_as_pod_u128(bytes: [u8; 16]) {
		let result = try_from_bytes::<PodU128>(&bytes);
		let pod = result.unwrap_or_else(|e| {
			panic!("try_from_bytes failed for bytes {bytes:?}: {e}")
		});
		let _: u128 = (*pod).into();
	}

	#[test]
	fn arbitrary_bytes_as_pod_i128(bytes: [u8; 16]) {
		let result = try_from_bytes::<PodI128>(&bytes);
		let pod = result.unwrap_or_else(|e| {
			panic!("try_from_bytes failed for bytes {bytes:?}: {e}")
		});
		let _: i128 = (*pod).into();
	}
}

// ---------------------------------------------------------------------------
// PodBool: only 0 and 1 are canonical
// ---------------------------------------------------------------------------

proptest! {
	#[test]
	fn pod_bool_canonical_only_zero_and_one(byte: u8) {
		let pod = PodBool(byte);
		if byte == 0 || byte == 1 {
			prop_assert!(pod.is_canonical());
		} else {
			prop_assert!(!pod.is_canonical());
		}
	}

	#[test]
	fn pod_bool_from_bool_always_canonical(b: bool) {
		let pod = PodBool::from_bool(b);
		prop_assert!(pod.is_canonical());
		let back: bool = pod.into();
		prop_assert_eq!(back, b);
	}
}

// ---------------------------------------------------------------------------
// Boundary values for all Pod integer types
// ---------------------------------------------------------------------------

#[test]
fn boundary_values_u16() {
	for &val in &[0u16, 1, u16::MAX - 1, u16::MAX] {
		let pod = PodU16::from_primitive(val);
		let back: u16 = pod.into();
		assert_eq!(back, val);
	}
}

#[test]
fn boundary_values_i16() {
	for &val in &[i16::MIN, i16::MIN + 1, -1i16, 0, 1, i16::MAX - 1, i16::MAX] {
		let pod = PodI16::from_primitive(val);
		let back: i16 = pod.into();
		assert_eq!(back, val);
	}
}

#[test]
fn boundary_values_u32() {
	for &val in &[0u32, 1, u32::MAX - 1, u32::MAX] {
		let pod = PodU32::from_primitive(val);
		let back: u32 = pod.into();
		assert_eq!(back, val);
	}
}

#[test]
fn boundary_values_i32() {
	for &val in &[i32::MIN, i32::MIN + 1, -1i32, 0, 1, i32::MAX - 1, i32::MAX] {
		let pod = PodI32::from_primitive(val);
		let back: i32 = pod.into();
		assert_eq!(back, val);
	}
}

#[test]
fn boundary_values_u64() {
	for &val in &[0u64, 1, u64::MAX - 1, u64::MAX] {
		let pod = PodU64::from_primitive(val);
		let back: u64 = pod.into();
		assert_eq!(back, val);
	}
}

#[test]
fn boundary_values_i64() {
	for &val in &[i64::MIN, i64::MIN + 1, -1i64, 0, 1, i64::MAX - 1, i64::MAX] {
		let pod = PodI64::from_primitive(val);
		let back: i64 = pod.into();
		assert_eq!(back, val);
	}
}

#[test]
fn boundary_values_u128() {
	for &val in &[0u128, 1, u128::MAX - 1, u128::MAX] {
		let pod = PodU128::from_primitive(val);
		let back: u128 = pod.into();
		assert_eq!(back, val);
	}
}

#[test]
fn boundary_values_i128() {
	for &val in &[
		i128::MIN,
		i128::MIN + 1,
		-1i128,
		0,
		1,
		i128::MAX - 1,
		i128::MAX,
	] {
		let pod = PodI128::from_primitive(val);
		let back: i128 = pod.into();
		assert_eq!(back, val);
	}
}

// ---------------------------------------------------------------------------
// bytemuck try_from_bytes never panics for correctly-sized aligned data
// ---------------------------------------------------------------------------

proptest! {
	/// For any arbitrary byte pattern of the correct size, `try_from_bytes`
	/// must return `Ok` (Pod types have no alignment or validity constraints
	/// beyond size) and the resulting value must survive conversion without
	/// panicking.
	#[test]
	fn try_from_bytes_never_panics_u64(bytes: [u8; 8]) {
		let result = try_from_bytes::<PodU64>(&bytes);
		// PodU64 is repr(transparent) over [u8; 8], so this must always
		// succeed for an 8-byte aligned slice.
		prop_assert!(result.is_ok());
		let val: u64 = (*result.unwrap_or_else(|e| {
			panic!("unexpected failure: {e}")
		}))
		.into();
		// Round-trip: encoding back must produce the same bytes.
		let re_encoded = PodU64::from_primitive(val);
		prop_assert_eq!(re_encoded.0, bytes);
	}

	#[test]
	fn try_from_bytes_never_panics_u128(bytes: [u8; 16]) {
		let result = try_from_bytes::<PodU128>(&bytes);
		prop_assert!(result.is_ok());
		let val: u128 = (*result.unwrap_or_else(|e| {
			panic!("unexpected failure: {e}")
		}))
		.into();
		let re_encoded = PodU128::from_primitive(val);
		prop_assert_eq!(re_encoded.0, bytes);
	}
}
