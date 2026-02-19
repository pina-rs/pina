//! Alignment-safe primitive wrappers that can be used in `Pod` structs.
use bytemuck::Pod;
use bytemuck::Zeroable;
use pinocchio::error::ProgramError;

/// The standard `bool` is not a `Pod`, define a replacement that is
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

/// `u16` type that can be used in `Pod`s
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodU16(pub [u8; 2]);
impl_int_conversion!(PodU16, u16);

/// `i16` type that can be used in Pods
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodI16(pub [u8; 2]);
impl_int_conversion!(PodI16, i16);

/// `u32` type that can be used in `Pod`s
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodU32(pub [u8; 4]);
impl_int_conversion!(PodU32, u32);

/// `i32` type that can be used in `Pod`s
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodI32(pub [u8; 4]);
impl_int_conversion!(PodI32, i32);

/// `u64` type that can be used in Pods
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodU64(pub [u8; 8]);
impl_int_conversion!(PodU64, u64);

/// `i64` type that can be used in Pods
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodI64(pub [u8; 8]);
impl_int_conversion!(PodI64, i64);

/// `u128` type that can be used in Pods
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodU128(pub [u8; 16]);
impl_int_conversion!(PodU128, u128);

/// `i128` type that can be used in Pods
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodI128(pub [u8; 16]);
impl_int_conversion!(PodI128, i128);

/// Reinterprets a byte slice as `&T` (zero-copy). Returns an error if the
/// slice has incorrect length or alignment.
pub fn pod_from_bytes<T: Pod>(bytes: &[u8]) -> Result<&T, ProgramError> {
	bytemuck::try_from_bytes(bytes).map_err(|_| ProgramError::InvalidArgument)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_pod_bool() {
		assert!(pod_from_bytes::<PodBool>(&[]).is_err());
		assert!(pod_from_bytes::<PodBool>(&[0, 0]).is_err());

		for i in 0..=u8::MAX {
			assert_eq!(i != 0, bool::from(pod_from_bytes::<PodBool>(&[i]).unwrap()));
		}
	}

	#[test]
	fn test_pod_u16() {
		assert!(pod_from_bytes::<PodU16>(&[]).is_err());
		assert_eq!(1u16, u16::from(*pod_from_bytes::<PodU16>(&[1, 0]).unwrap()));
	}

	#[test]
	fn test_pod_i16() {
		assert!(pod_from_bytes::<PodI16>(&[]).is_err());
		assert_eq!(
			-1i16,
			i16::from(*pod_from_bytes::<PodI16>(&[255, 255]).unwrap())
		);
	}

	#[test]
	fn test_pod_u64() {
		assert!(pod_from_bytes::<PodU64>(&[]).is_err());
		assert_eq!(
			1u64,
			u64::from(*pod_from_bytes::<PodU64>(&[1, 0, 0, 0, 0, 0, 0, 0]).unwrap())
		);
	}

	#[test]
	fn test_pod_i64() {
		assert!(pod_from_bytes::<PodI64>(&[]).is_err());
		assert_eq!(
			-1i64,
			i64::from(
				*pod_from_bytes::<PodI64>(&[255, 255, 255, 255, 255, 255, 255, 255]).unwrap()
			)
		);
	}

	#[test]
	fn test_pod_i32() {
		assert!(pod_from_bytes::<PodI32>(&[]).is_err());
		assert_eq!(
			-1i32,
			i32::from(*pod_from_bytes::<PodI32>(&[255, 255, 255, 255]).unwrap())
		);
		assert_eq!(
			1i32,
			i32::from(*pod_from_bytes::<PodI32>(&[1, 0, 0, 0]).unwrap())
		);
	}

	#[test]
	fn test_pod_u128() {
		assert!(pod_from_bytes::<PodU128>(&[]).is_err());
		assert_eq!(
			1u128,
			u128::from(
				*pod_from_bytes::<PodU128>(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
					.unwrap()
			)
		);
	}

	#[test]
	fn test_pod_i128() {
		assert!(pod_from_bytes::<PodI128>(&[]).is_err());
		assert_eq!(
			-1i128,
			i128::from(
				*pod_from_bytes::<PodI128>(&[
					255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255
				])
				.unwrap()
			)
		);
		assert_eq!(
			1i128,
			i128::from(
				*pod_from_bytes::<PodI128>(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
					.unwrap()
			)
		);
	}
}
