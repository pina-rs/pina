//! Property-based fuzz tests for discriminator parsing safety and round-trip
//! correctness across all supported primitive discriminator widths (u8, u16,
//! u32, u64).

use pina::IntoDiscriminator;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Arbitrary byte slices never cause panics when parsed as discriminators
// ---------------------------------------------------------------------------

proptest! {
	#[test]
	fn arbitrary_bytes_no_panic_u8(ref bytes in prop::collection::vec(any::<u8>(), 0..64)) {
		// Must not panic — either Ok or Err.
		let _ = u8::discriminator_from_bytes(bytes);
	}

	#[test]
	fn arbitrary_bytes_no_panic_u16(ref bytes in prop::collection::vec(any::<u8>(), 0..64)) {
		let _ = u16::discriminator_from_bytes(bytes);
	}

	#[test]
	fn arbitrary_bytes_no_panic_u32(ref bytes in prop::collection::vec(any::<u8>(), 0..64)) {
		let _ = u32::discriminator_from_bytes(bytes);
	}

	#[test]
	fn arbitrary_bytes_no_panic_u64(ref bytes in prop::collection::vec(any::<u8>(), 0..64)) {
		let _ = u64::discriminator_from_bytes(bytes);
	}
}

// ---------------------------------------------------------------------------
// Valid discriminator values round-trip correctly (write -> from_bytes)
// ---------------------------------------------------------------------------

proptest! {
	#[test]
	fn roundtrip_u8(val: u8) {
		let mut bytes = [0u8; 1];
		val.write_discriminator(&mut bytes);
		let decoded = u8::discriminator_from_bytes(&bytes)
			.unwrap_or_else(|_| panic!("discriminator_from_bytes failed for u8 value {val}"));
		prop_assert_eq!(decoded, val);
	}

	#[test]
	fn roundtrip_u16(val: u16) {
		let mut bytes = [0u8; 2];
		val.write_discriminator(&mut bytes);
		let decoded = u16::discriminator_from_bytes(&bytes)
			.unwrap_or_else(|_| panic!("discriminator_from_bytes failed for u16 value {val}"));
		prop_assert_eq!(decoded, val);
	}

	#[test]
	fn roundtrip_u32(val: u32) {
		let mut bytes = [0u8; 4];
		val.write_discriminator(&mut bytes);
		let decoded = u32::discriminator_from_bytes(&bytes)
			.unwrap_or_else(|_| panic!("discriminator_from_bytes failed for u32 value {val}"));
		prop_assert_eq!(decoded, val);
	}

	#[test]
	fn roundtrip_u64(val: u64) {
		let mut bytes = [0u8; 8];
		val.write_discriminator(&mut bytes);
		let decoded = u64::discriminator_from_bytes(&bytes)
			.unwrap_or_else(|_| panic!("discriminator_from_bytes failed for u64 value {val}"));
		prop_assert_eq!(decoded, val);
	}
}

// ---------------------------------------------------------------------------
// matches_discriminator is consistent with write_discriminator
// ---------------------------------------------------------------------------

proptest! {
	#[test]
	fn matches_after_write_u8(val: u8) {
		let mut bytes = [0u8; 1];
		val.write_discriminator(&mut bytes);
		prop_assert!(val.matches_discriminator(&bytes));
	}

	#[test]
	fn matches_after_write_u16(val: u16) {
		let mut bytes = [0u8; 2];
		val.write_discriminator(&mut bytes);
		prop_assert!(val.matches_discriminator(&bytes));
	}

	#[test]
	fn matches_after_write_u32(val: u32) {
		let mut bytes = [0u8; 4];
		val.write_discriminator(&mut bytes);
		prop_assert!(val.matches_discriminator(&bytes));
	}

	#[test]
	fn matches_after_write_u64(val: u64) {
		let mut bytes = [0u8; 8];
		val.write_discriminator(&mut bytes);
		prop_assert!(val.matches_discriminator(&bytes));
	}
}

// ---------------------------------------------------------------------------
// Short slices always return Err or false, never panic
// ---------------------------------------------------------------------------

proptest! {
	#[test]
	fn short_slice_u16_no_panic(ref bytes in prop::collection::vec(any::<u8>(), 0..2)) {
		// Fewer than 2 bytes — must be Err.
		let result = u16::discriminator_from_bytes(bytes);
		prop_assert!(result.is_err());
	}

	#[test]
	fn short_slice_u32_no_panic(ref bytes in prop::collection::vec(any::<u8>(), 0..4)) {
		let result = u32::discriminator_from_bytes(bytes);
		prop_assert!(result.is_err());
	}

	#[test]
	fn short_slice_u64_no_panic(ref bytes in prop::collection::vec(any::<u8>(), 0..8)) {
		let result = u64::discriminator_from_bytes(bytes);
		prop_assert!(result.is_err());
	}

	#[test]
	fn matches_short_slice_u16_no_panic(
		val: u16,
		ref bytes in prop::collection::vec(any::<u8>(), 0..2),
	) {
		// Short data — must return false, never panic.
		prop_assert!(!val.matches_discriminator(bytes));
	}

	#[test]
	fn matches_short_slice_u32_no_panic(
		val: u32,
		ref bytes in prop::collection::vec(any::<u8>(), 0..4),
	) {
		prop_assert!(!val.matches_discriminator(bytes));
	}

	#[test]
	fn matches_short_slice_u64_no_panic(
		val: u64,
		ref bytes in prop::collection::vec(any::<u8>(), 0..8),
	) {
		prop_assert!(!val.matches_discriminator(bytes));
	}
}

// ---------------------------------------------------------------------------
// Boundary values for all discriminator widths
// ---------------------------------------------------------------------------

#[test]
fn boundary_values_u8() {
	for val in [0u8, 1, 127, 128, 254, 255] {
		let mut bytes = [0u8; 1];
		val.write_discriminator(&mut bytes);
		let decoded = u8::discriminator_from_bytes(&bytes)
			.unwrap_or_else(|_| panic!("failed for u8 boundary value {val}"));
		assert_eq!(decoded, val);
		assert!(val.matches_discriminator(&bytes));
	}
}

#[test]
fn boundary_values_u16() {
	for val in [0u16, 1, 255, 256, u16::MAX - 1, u16::MAX] {
		let mut bytes = [0u8; 2];
		val.write_discriminator(&mut bytes);
		let decoded = u16::discriminator_from_bytes(&bytes)
			.unwrap_or_else(|_| panic!("failed for u16 boundary value {val}"));
		assert_eq!(decoded, val);
		assert!(val.matches_discriminator(&bytes));
	}
}

#[test]
fn boundary_values_u32() {
	for val in [0u32, 1, 255, 256, 65535, 65536, u32::MAX - 1, u32::MAX] {
		let mut bytes = [0u8; 4];
		val.write_discriminator(&mut bytes);
		let decoded = u32::discriminator_from_bytes(&bytes)
			.unwrap_or_else(|_| panic!("failed for u32 boundary value {val}"));
		assert_eq!(decoded, val);
		assert!(val.matches_discriminator(&bytes));
	}
}

#[test]
fn boundary_values_u64() {
	for val in [
		0u64,
		1,
		255,
		256,
		65535,
		65536,
		u32::MAX as u64,
		(u32::MAX as u64) + 1,
		u64::MAX - 1,
		u64::MAX,
	] {
		let mut bytes = [0u8; 8];
		val.write_discriminator(&mut bytes);
		let decoded = u64::discriminator_from_bytes(&bytes)
			.unwrap_or_else(|_| panic!("failed for u64 boundary value {val}"));
		assert_eq!(decoded, val);
		assert!(val.matches_discriminator(&bytes));
	}
}

// ---------------------------------------------------------------------------
// Extra-long slices: discriminator_from_bytes reads only the first N bytes
// ---------------------------------------------------------------------------

proptest! {
	#[test]
	fn extra_long_slice_u8(
		val: u8,
		ref tail in prop::collection::vec(any::<u8>(), 1..32),
	) {
		let mut bytes = vec![0u8; 1];
		val.write_discriminator(&mut bytes);
		bytes.extend_from_slice(tail);
		let decoded = u8::discriminator_from_bytes(&bytes)
			.unwrap_or_else(|_| panic!("failed for u8 with extra bytes"));
		prop_assert_eq!(decoded, val);
	}

	#[test]
	fn extra_long_slice_u32(
		val: u32,
		ref tail in prop::collection::vec(any::<u8>(), 1..32),
	) {
		let mut bytes = vec![0u8; 4];
		val.write_discriminator(&mut bytes[..4]);
		bytes.extend_from_slice(tail);
		let decoded = u32::discriminator_from_bytes(&bytes)
			.unwrap_or_else(|_| panic!("failed for u32 with extra bytes"));
		prop_assert_eq!(decoded, val);
	}
}
