#![no_std]

//! Alignment-safe primitive wrappers that can be used in `Pod` structs.

use bytemuck::Pod;
use bytemuck::Zeroable;

/// The standard `bool` is not a `Pod`, define a replacement that is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodBool(pub u8);

impl PodBool {
	pub const fn from_bool(b: bool) -> Self {
		Self(if b { 1 } else { 0 })
	}

	/// Returns `true` if the underlying byte is a canonical boolean value
	/// (`0` or `1`).
	///
	/// Non-canonical values (2–255) are accepted by `bytemuck` deserialization
	/// and convert to `true`, but two non-canonical `PodBool` values
	/// representing the same logical boolean may fail `PartialEq` comparison.
	/// Use this method to validate account data at deserialization boundaries.
	pub const fn is_canonical(&self) -> bool {
		self.0 == 0 || self.0 == 1
	}
}

impl From<bool> for PodBool {
	fn from(b: bool) -> Self {
		Self::from_bool(b)
	}
}

impl From<&bool> for PodBool {
	fn from(b: &bool) -> Self {
		Self(u8::from(*b))
	}
}

impl From<&PodBool> for bool {
	fn from(b: &PodBool) -> Self {
		b.0 != 0
	}
}

impl From<PodBool> for bool {
	fn from(b: PodBool) -> Self {
		b.0 != 0
	}
}

/// Implements bidirectional conversion between a `Pod*` wrapper type and its
/// corresponding standard integer.
///
/// For a given pair `($P, $I)`, this generates:
/// - `$P::from_primitive($I) -> $P` (const)
/// - `From<$I> for $P`
/// - `From<$P> for $I`
#[macro_export]
macro_rules! impl_int_conversion {
	($P:ty, $I:ty) => {
		impl $P {
			pub const fn from_primitive(n: $I) -> Self {
				Self(n.to_le_bytes())
			}
		}
		impl From<$I> for $P {
			fn from(n: $I) -> Self {
				Self::from_primitive(n)
			}
		}
		impl From<$P> for $I {
			fn from(pod: $P) -> Self {
				Self::from_le_bytes(pod.0)
			}
		}
	};
}

/// `u16` type that can be used in `Pod`s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodU16(pub [u8; 2]);
impl_int_conversion!(PodU16, u16);

/// `i16` type that can be used in `Pod`s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodI16(pub [u8; 2]);
impl_int_conversion!(PodI16, i16);

/// `u32` type that can be used in `Pod`s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodU32(pub [u8; 4]);
impl_int_conversion!(PodU32, u32);

/// `i32` type that can be used in `Pod`s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodI32(pub [u8; 4]);
impl_int_conversion!(PodI32, i32);

/// `u64` type that can be used in `Pod`s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodU64(pub [u8; 8]);
impl_int_conversion!(PodU64, u64);

/// `i64` type that can be used in `Pod`s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodI64(pub [u8; 8]);
impl_int_conversion!(PodI64, i64);

/// `u128` type that can be used in `Pod`s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodU128(pub [u8; 16]);
impl_int_conversion!(PodU128, u128);

/// `i128` type that can be used in `Pod`s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodI128(pub [u8; 16]);
impl_int_conversion!(PodI128, i128);

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
	use bytemuck::try_from_bytes;

	use super::*;

	#[test]
	fn pod_bool_roundtrip() {
		for i in 0..=u8::MAX {
			let value = *try_from_bytes::<PodBool>(&[i]).unwrap();
			assert_eq!(i != 0, bool::from(value));
		}
	}

	/// Demonstrates that non-canonical PodBool values (2–255) convert to
	/// `true` but fail `PartialEq` against `PodBool(1)`. Programs should
	/// use `is_canonical()` to detect this at deserialization boundaries.
	#[test]
	fn pod_bool_non_canonical_equality_mismatch() {
		let canonical_true = PodBool::from_bool(true);
		let non_canonical_true = *try_from_bytes::<PodBool>(&[2]).unwrap();

		// Both convert to `true`...
		assert!(bool::from(canonical_true));
		assert!(bool::from(non_canonical_true));

		// ...but fail PartialEq because the raw bytes differ.
		assert_ne!(canonical_true, non_canonical_true);

		// `is_canonical` detects the non-standard encoding.
		assert!(canonical_true.is_canonical());
		assert!(!non_canonical_true.is_canonical());
	}

	#[test]
	fn pod_bool_is_canonical_boundary_values() {
		assert!(PodBool(0).is_canonical());
		assert!(PodBool(1).is_canonical());
		assert!(!PodBool(2).is_canonical());
		assert!(!PodBool(127).is_canonical());
		assert!(!PodBool(255).is_canonical());
	}

	#[test]
	fn pod_bool_from_bool_produces_canonical() {
		assert!(PodBool::from_bool(false).is_canonical());
		assert!(PodBool::from_bool(true).is_canonical());
		assert!(PodBool::from(false).is_canonical());
		assert!(PodBool::from(true).is_canonical());
	}

	#[test]
	fn pod_bool_from_ref() {
		let t = true;
		let f = false;
		assert_eq!(PodBool::from(&t), PodBool(1));
		assert_eq!(PodBool::from(&f), PodBool(0));
	}

	#[test]
	fn pod_bool_from_ref_roundtrip() {
		let pod = PodBool(1);
		assert!(bool::from(&pod));
		let pod = PodBool(0);
		assert!(!bool::from(&pod));
	}

	#[test]
	fn pod_bool_default_is_false() {
		let default = PodBool::default();
		assert_eq!(default.0, 0);
		assert!(!bool::from(default));
		assert!(default.is_canonical());
	}

	#[test]
	fn pod_u16_roundtrip() {
		assert_eq!(1u16, u16::from(PodU16::from_primitive(1)));
	}

	#[test]
	fn pod_i16_roundtrip() {
		assert_eq!(-1i16, i16::from(PodI16::from_primitive(-1)));
	}

	#[test]
	fn pod_u32_roundtrip() {
		assert_eq!(7u32, u32::from(PodU32::from_primitive(7)));
	}

	#[test]
	fn pod_i32_roundtrip() {
		assert_eq!(-7i32, i32::from(PodI32::from_primitive(-7)));
	}

	#[test]
	fn pod_u64_roundtrip() {
		assert_eq!(9u64, u64::from(PodU64::from_primitive(9)));
	}

	#[test]
	fn pod_i64_roundtrip() {
		assert_eq!(-9i64, i64::from(PodI64::from_primitive(-9)));
	}

	#[test]
	fn pod_u128_roundtrip() {
		assert_eq!(11u128, u128::from(PodU128::from_primitive(11)));
	}

	#[test]
	fn pod_i128_roundtrip() {
		assert_eq!(-11i128, i128::from(PodI128::from_primitive(-11)));
	}

	#[test]
	fn pod_u16_boundary_values() {
		assert_eq!(0u16, u16::from(PodU16::from_primitive(0)));
		assert_eq!(u16::MAX, u16::from(PodU16::from_primitive(u16::MAX)));
	}

	#[test]
	fn pod_i16_boundary_values() {
		assert_eq!(i16::MIN, i16::from(PodI16::from_primitive(i16::MIN)));
		assert_eq!(i16::MAX, i16::from(PodI16::from_primitive(i16::MAX)));
		assert_eq!(0i16, i16::from(PodI16::from_primitive(0)));
	}

	#[test]
	fn pod_u32_boundary_values() {
		assert_eq!(0u32, u32::from(PodU32::from_primitive(0)));
		assert_eq!(u32::MAX, u32::from(PodU32::from_primitive(u32::MAX)));
	}

	#[test]
	fn pod_i32_boundary_values() {
		assert_eq!(i32::MIN, i32::from(PodI32::from_primitive(i32::MIN)));
		assert_eq!(i32::MAX, i32::from(PodI32::from_primitive(i32::MAX)));
	}

	#[test]
	fn pod_u64_boundary_values() {
		assert_eq!(0u64, u64::from(PodU64::from_primitive(0)));
		assert_eq!(u64::MAX, u64::from(PodU64::from_primitive(u64::MAX)));
	}

	#[test]
	fn pod_i64_boundary_values() {
		assert_eq!(i64::MIN, i64::from(PodI64::from_primitive(i64::MIN)));
		assert_eq!(i64::MAX, i64::from(PodI64::from_primitive(i64::MAX)));
	}

	#[test]
	fn pod_u128_boundary_values() {
		assert_eq!(0u128, u128::from(PodU128::from_primitive(0)));
		assert_eq!(u128::MAX, u128::from(PodU128::from_primitive(u128::MAX)));
	}

	#[test]
	fn pod_i128_boundary_values() {
		assert_eq!(i128::MIN, i128::from(PodI128::from_primitive(i128::MIN)));
		assert_eq!(i128::MAX, i128::from(PodI128::from_primitive(i128::MAX)));
	}

	/// Verify that all Pod types store bytes in little-endian order, which
	/// is the native byte order on Solana's BPF/SBF target.
	#[test]
	fn pod_types_use_little_endian_byte_order() {
		let u16_val = PodU16::from_primitive(0x0102);
		assert_eq!(u16_val.0, [0x02, 0x01]);

		let u32_val = PodU32::from_primitive(0x01020304);
		assert_eq!(u32_val.0, [0x04, 0x03, 0x02, 0x01]);

		let u64_val = PodU64::from_primitive(0x0102030405060708);
		assert_eq!(u64_val.0, [0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]);
	}

	/// Verify that bytemuck deserialization of Pod types works correctly
	/// from raw byte slices, simulating zero-copy account data access.
	#[test]
	fn pod_types_bytemuck_from_bytes() {
		let bytes_u16 = [0x39, 0x05]; // 0x0539 = 1337
		let val = try_from_bytes::<PodU16>(&bytes_u16).unwrap();
		assert_eq!(u16::from(*val), 1337);

		let bytes_u32 = [0xEF, 0xBE, 0xAD, 0xDE]; // 0xDEADBEEF
		let val = try_from_bytes::<PodU32>(&bytes_u32).unwrap();
		assert_eq!(u32::from(*val), 0xDEAD_BEEF);

		let bytes_i16 = [0xFF, 0xFF]; // -1 in two's complement LE
		let val = try_from_bytes::<PodI16>(&bytes_i16).unwrap();
		assert_eq!(i16::from(*val), -1);
	}

	#[test]
	fn pod_default_is_zero() {
		assert_eq!(u16::from(PodU16::default()), 0);
		assert_eq!(i16::from(PodI16::default()), 0);
		assert_eq!(u32::from(PodU32::default()), 0);
		assert_eq!(i32::from(PodI32::default()), 0);
		assert_eq!(u64::from(PodU64::default()), 0);
		assert_eq!(i64::from(PodI64::default()), 0);
		assert_eq!(u128::from(PodU128::default()), 0);
		assert_eq!(i128::from(PodI128::default()), 0);
	}
}
