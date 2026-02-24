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
}
