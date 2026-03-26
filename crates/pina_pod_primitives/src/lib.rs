#![no_std]

//! Alignment-safe primitive wrappers that can be used in `Pod` structs.
//!
//! Pod types (`PodU64`, `PodU32`, etc.) wrap native integers in `[u8; N]`
//! arrays, guaranteeing alignment 1. This allows direct pointer casts from
//! account data without alignment concerns — critical for `#[repr(C)]`
//! zero-copy structs on Solana.
//!
//! # Arithmetic
//!
//! Arithmetic operators (`+`, `-`, `*`) use **wrapping** semantics in release
//! builds for CU efficiency and **panic on overflow** in debug builds. Use
//! `checked_add`, `checked_sub`, `checked_mul`, `checked_div` where overflow
//! must be detected in all build profiles.
//!
//! # Constants
//!
//! Each Pod integer type provides `ZERO`, `MIN`, and `MAX` constants.

use bytemuck::Pod;
use bytemuck::Zeroable;
use core::fmt;
use core::mem::align_of;
use core::mem::size_of;

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

impl core::ops::Not for PodBool {
	type Output = Self;

	#[inline]
	fn not(self) -> Self {
		Self::from_bool(!bool::from(self))
	}
}

impl fmt::Display for PodBool {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		bool::from(*self).fmt(f)
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

			/// Returns the contained native value, converting from
			/// little-endian bytes.
			#[inline]
			pub const fn get(&self) -> $I {
				<$I>::from_le_bytes(self.0)
			}
		}

		impl From<$I> for $P {
			fn from(n: $I) -> Self {
				Self::from_primitive(n)
			}
		}

		impl From<$P> for $I {
			fn from(pod: $P) -> Self {
				pod.get()
			}
		}
	};
}

/// Implements constants, ordering, display, checked/saturating arithmetic, and
/// helper methods for a Pod integer type.
macro_rules! impl_pod_common {
	($name:ident, $native:ty, $size:expr) => {
		impl $name {
			/// The zero value.
			pub const ZERO: Self = Self([0u8; $size]);

			#[doc = concat!(
				"The largest value representable by [`",
				stringify!($native),
				"`]."
			)]
			pub const MAX: Self = Self(<$native>::MAX.to_le_bytes());

			#[doc = concat!(
				"The smallest value representable by [`",
				stringify!($native),
				"`]."
			)]
			pub const MIN: Self = Self(<$native>::MIN.to_le_bytes());

			/// Returns `true` if the value is zero.
			#[inline]
			#[must_use]
			pub fn is_zero(&self) -> bool {
				self.0 == [0u8; $size]
			}

			/// Checked addition. Returns `None` on overflow.
			#[inline]
			#[must_use]
			pub fn checked_add(self, rhs: impl Into<$name>) -> Option<Self> {
				self.get().checked_add(rhs.into().get()).map(Self::from)
			}

			/// Checked subtraction. Returns `None` on underflow.
			#[inline]
			#[must_use]
			pub fn checked_sub(self, rhs: impl Into<$name>) -> Option<Self> {
				self.get().checked_sub(rhs.into().get()).map(Self::from)
			}

			/// Checked multiplication. Returns `None` on overflow.
			#[inline]
			#[must_use]
			pub fn checked_mul(self, rhs: impl Into<$name>) -> Option<Self> {
				self.get().checked_mul(rhs.into().get()).map(Self::from)
			}

			/// Checked division. Returns `None` if `rhs` is zero.
			#[inline]
			#[must_use]
			pub fn checked_div(self, rhs: impl Into<$name>) -> Option<Self> {
				self.get().checked_div(rhs.into().get()).map(Self::from)
			}

			/// Saturating addition. Clamps at the numeric bounds instead of
			/// overflowing.
			#[inline]
			#[must_use]
			pub fn saturating_add(self, rhs: impl Into<$name>) -> Self {
				Self::from(self.get().saturating_add(rhs.into().get()))
			}

			/// Saturating subtraction. Clamps at the numeric bound instead of
			/// underflowing.
			#[inline]
			#[must_use]
			pub fn saturating_sub(self, rhs: impl Into<$name>) -> Self {
				Self::from(self.get().saturating_sub(rhs.into().get()))
			}

			/// Saturating multiplication. Clamps at the numeric bounds instead
			/// of overflowing.
			#[inline]
			#[must_use]
			pub fn saturating_mul(self, rhs: impl Into<$name>) -> Self {
				Self::from(self.get().saturating_mul(rhs.into().get()))
			}
		}

		impl PartialOrd for $name {
			#[inline]
			fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
				Some(self.cmp(other))
			}
		}

		impl Ord for $name {
			#[inline]
			fn cmp(&self, other: &Self) -> core::cmp::Ordering {
				self.get().cmp(&other.get())
			}
		}

		impl PartialEq<$native> for $name {
			#[inline]
			fn eq(&self, other: &$native) -> bool {
				self.get() == *other
			}
		}

		impl PartialOrd<$native> for $name {
			#[inline]
			fn partial_cmp(&self, other: &$native) -> Option<core::cmp::Ordering> {
				self.get().partial_cmp(other)
			}
		}

		impl fmt::Display for $name {
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				self.get().fmt(f)
			}
		}
	};
}

/// Implements arithmetic operators for a Pod type.
///
/// In debug builds, operators panic on overflow via `checked_*`. In release
/// builds, they use `wrapping_*` for CU efficiency on Solana.
macro_rules! impl_pod_arithmetic {
	($name:ident, $native:ty) => {
		// --- Pod + native ---

		impl core::ops::Add<$native> for $name {
			type Output = Self;

			#[inline]
			fn add(self, rhs: $native) -> Self {
				#[cfg(debug_assertions)]
				{
					Self::from(
						self.get()
							.checked_add(rhs)
							.unwrap_or_else(|| panic!("attempt to add with overflow")),
					)
				}
				#[cfg(not(debug_assertions))]
				{
					Self::from(self.get().wrapping_add(rhs))
				}
			}
		}

		impl core::ops::Sub<$native> for $name {
			type Output = Self;

			#[inline]
			fn sub(self, rhs: $native) -> Self {
				#[cfg(debug_assertions)]
				{
					Self::from(
						self.get()
							.checked_sub(rhs)
							.unwrap_or_else(|| panic!("attempt to subtract with overflow")),
					)
				}
				#[cfg(not(debug_assertions))]
				{
					Self::from(self.get().wrapping_sub(rhs))
				}
			}
		}

		impl core::ops::Mul<$native> for $name {
			type Output = Self;

			#[inline]
			fn mul(self, rhs: $native) -> Self {
				#[cfg(debug_assertions)]
				{
					Self::from(
						self.get()
							.checked_mul(rhs)
							.unwrap_or_else(|| panic!("attempt to multiply with overflow")),
					)
				}
				#[cfg(not(debug_assertions))]
				{
					Self::from(self.get().wrapping_mul(rhs))
				}
			}
		}

		impl core::ops::Div<$native> for $name {
			type Output = Self;

			#[inline]
			fn div(self, rhs: $native) -> Self {
				Self::from(self.get() / rhs)
			}
		}

		impl core::ops::Rem<$native> for $name {
			type Output = Self;

			#[inline]
			fn rem(self, rhs: $native) -> Self {
				Self::from(self.get() % rhs)
			}
		}

		// --- Pod + Pod ---

		impl core::ops::Add for $name {
			type Output = Self;

			#[inline]
			fn add(self, rhs: Self) -> Self {
				self + rhs.get()
			}
		}

		impl core::ops::Sub for $name {
			type Output = Self;

			#[inline]
			fn sub(self, rhs: Self) -> Self {
				self - rhs.get()
			}
		}

		impl core::ops::Mul for $name {
			type Output = Self;

			#[inline]
			fn mul(self, rhs: Self) -> Self {
				self * rhs.get()
			}
		}

		impl core::ops::Div for $name {
			type Output = Self;

			#[inline]
			fn div(self, rhs: Self) -> Self {
				self / rhs.get()
			}
		}

		impl core::ops::Rem for $name {
			type Output = Self;

			#[inline]
			fn rem(self, rhs: Self) -> Self {
				self % rhs.get()
			}
		}

		// --- Assign with native ---

		impl core::ops::AddAssign<$native> for $name {
			#[inline]
			fn add_assign(&mut self, rhs: $native) {
				*self = *self + rhs;
			}
		}

		impl core::ops::SubAssign<$native> for $name {
			#[inline]
			fn sub_assign(&mut self, rhs: $native) {
				*self = *self - rhs;
			}
		}

		impl core::ops::MulAssign<$native> for $name {
			#[inline]
			fn mul_assign(&mut self, rhs: $native) {
				*self = *self * rhs;
			}
		}

		impl core::ops::DivAssign<$native> for $name {
			#[inline]
			fn div_assign(&mut self, rhs: $native) {
				*self = *self / rhs;
			}
		}

		impl core::ops::RemAssign<$native> for $name {
			#[inline]
			fn rem_assign(&mut self, rhs: $native) {
				*self = *self % rhs;
			}
		}

		// --- Assign with Pod ---

		impl core::ops::AddAssign for $name {
			#[inline]
			fn add_assign(&mut self, rhs: Self) {
				*self = *self + rhs;
			}
		}

		impl core::ops::SubAssign for $name {
			#[inline]
			fn sub_assign(&mut self, rhs: Self) {
				*self = *self - rhs;
			}
		}

		impl core::ops::MulAssign for $name {
			#[inline]
			fn mul_assign(&mut self, rhs: Self) {
				*self = *self * rhs;
			}
		}

		impl core::ops::DivAssign for $name {
			#[inline]
			fn div_assign(&mut self, rhs: Self) {
				*self = *self / rhs;
			}
		}

		impl core::ops::RemAssign for $name {
			#[inline]
			fn rem_assign(&mut self, rhs: Self) {
				*self = *self % rhs;
			}
		}

		// --- Bitwise ---

		impl core::ops::BitAnd<$native> for $name {
			type Output = Self;

			#[inline]
			fn bitand(self, rhs: $native) -> Self {
				Self::from(self.get() & rhs)
			}
		}

		impl core::ops::BitOr<$native> for $name {
			type Output = Self;

			#[inline]
			fn bitor(self, rhs: $native) -> Self {
				Self::from(self.get() | rhs)
			}
		}

		impl core::ops::BitXor<$native> for $name {
			type Output = Self;

			#[inline]
			fn bitxor(self, rhs: $native) -> Self {
				Self::from(self.get() ^ rhs)
			}
		}

		impl core::ops::BitAnd for $name {
			type Output = Self;

			#[inline]
			fn bitand(self, rhs: Self) -> Self {
				self & rhs.get()
			}
		}

		impl core::ops::BitOr for $name {
			type Output = Self;

			#[inline]
			fn bitor(self, rhs: Self) -> Self {
				self | rhs.get()
			}
		}

		impl core::ops::BitXor for $name {
			type Output = Self;

			#[inline]
			fn bitxor(self, rhs: Self) -> Self {
				self ^ rhs.get()
			}
		}

		impl core::ops::Shl<u32> for $name {
			type Output = Self;

			#[inline]
			fn shl(self, rhs: u32) -> Self {
				Self::from(self.get() << rhs)
			}
		}

		impl core::ops::Shr<u32> for $name {
			type Output = Self;

			#[inline]
			fn shr(self, rhs: u32) -> Self {
				Self::from(self.get() >> rhs)
			}
		}

		impl core::ops::Not for $name {
			type Output = Self;

			#[inline]
			fn not(self) -> Self {
				Self::from(!self.get())
			}
		}

		// --- Bitwise assign with native ---

		impl core::ops::BitAndAssign<$native> for $name {
			#[inline]
			fn bitand_assign(&mut self, rhs: $native) {
				*self = *self & rhs;
			}
		}

		impl core::ops::BitOrAssign<$native> for $name {
			#[inline]
			fn bitor_assign(&mut self, rhs: $native) {
				*self = *self | rhs;
			}
		}

		impl core::ops::BitXorAssign<$native> for $name {
			#[inline]
			fn bitxor_assign(&mut self, rhs: $native) {
				*self = *self ^ rhs;
			}
		}

		// --- Bitwise assign with Pod ---

		impl core::ops::BitAndAssign for $name {
			#[inline]
			fn bitand_assign(&mut self, rhs: Self) {
				*self = *self & rhs;
			}
		}

		impl core::ops::BitOrAssign for $name {
			#[inline]
			fn bitor_assign(&mut self, rhs: Self) {
				*self = *self | rhs;
			}
		}

		impl core::ops::BitXorAssign for $name {
			#[inline]
			fn bitxor_assign(&mut self, rhs: Self) {
				*self = *self ^ rhs;
			}
		}

		impl core::ops::ShlAssign<u32> for $name {
			#[inline]
			fn shl_assign(&mut self, rhs: u32) {
				*self = *self << rhs;
			}
		}

		impl core::ops::ShrAssign<u32> for $name {
			#[inline]
			fn shr_assign(&mut self, rhs: u32) {
				*self = *self >> rhs;
			}
		}
	};
}

/// Implements `Neg` for signed Pod types.
macro_rules! impl_pod_neg {
	($name:ident, $native:ty) => {
		impl core::ops::Neg for $name {
			type Output = Self;

			#[inline]
			fn neg(self) -> Self {
				#[cfg(debug_assertions)]
				{
					Self::from(
						self.get()
							.checked_neg()
							.unwrap_or_else(|| panic!("attempt to negate with overflow")),
					)
				}
				#[cfg(not(debug_assertions))]
				{
					Self::from(self.get().wrapping_neg())
				}
			}
		}
	};
}

/// Defines an unsigned Pod integer type with full operator support.
macro_rules! define_pod_unsigned {
	($name:ident, $native:ty, $size:expr, $doc:expr) => {
		#[doc = $doc]
		#[derive(Clone, Copy, Default, PartialEq, Eq, Pod, Zeroable)]
		#[repr(transparent)]
		pub struct $name(pub [u8; $size]);

		impl_int_conversion!($name, $native);
		impl_pod_common!($name, $native, $size);
		impl_pod_arithmetic!($name, $native);

		impl fmt::Debug for $name {
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				write!(f, "{}({})", stringify!($name), self.get())
			}
		}
	};
}

/// Defines a signed Pod integer type with full operator support.
macro_rules! define_pod_signed {
	($name:ident, $native:ty, $size:expr, $doc:expr) => {
		#[doc = $doc]
		#[derive(Clone, Copy, Default, PartialEq, Eq, Pod, Zeroable)]
		#[repr(transparent)]
		pub struct $name(pub [u8; $size]);

		impl_int_conversion!($name, $native);
		impl_pod_common!($name, $native, $size);
		impl_pod_arithmetic!($name, $native);
		impl_pod_neg!($name, $native);

		impl fmt::Debug for $name {
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				write!(f, "{}({})", stringify!($name), self.get())
			}
		}
	};
}

define_pod_unsigned!(
	PodU16,
	u16,
	2,
	"An alignment-1 wrapper around `u16` stored as `[u8; 2]`.\n\n\
	 Enables safe zero-copy access inside `#[repr(C)]` account structs."
);

define_pod_signed!(
	PodI16,
	i16,
	2,
	"An alignment-1 wrapper around `i16` stored as `[u8; 2]`.\n\n\
	 Enables safe zero-copy access inside `#[repr(C)]` account structs."
);

define_pod_unsigned!(
	PodU32,
	u32,
	4,
	"An alignment-1 wrapper around `u32` stored as `[u8; 4]`.\n\n\
	 Enables safe zero-copy access inside `#[repr(C)]` account structs."
);

define_pod_signed!(
	PodI32,
	i32,
	4,
	"An alignment-1 wrapper around `i32` stored as `[u8; 4]`.\n\n\
	 Enables safe zero-copy access inside `#[repr(C)]` account structs."
);

define_pod_unsigned!(
	PodU64,
	u64,
	8,
	"An alignment-1 wrapper around `u64` stored as `[u8; 8]`.\n\n\
	 Enables safe zero-copy access inside `#[repr(C)]` account structs."
);

define_pod_signed!(
	PodI64,
	i64,
	8,
	"An alignment-1 wrapper around `i64` stored as `[u8; 8]`.\n\n\
	 Enables safe zero-copy access inside `#[repr(C)]` account structs."
);

define_pod_unsigned!(
	PodU128,
	u128,
	16,
	"An alignment-1 wrapper around `u128` stored as `[u8; 16]`.\n\n\
	 Enables safe zero-copy access inside `#[repr(C)]` account structs."
);

define_pod_signed!(
	PodI128,
	i128,
	16,
	"An alignment-1 wrapper around `i128` stored as `[u8; 16]`.\n\n\
	 Enables safe zero-copy access inside `#[repr(C)]` account structs."
);

// Compile-time invariant: all Pod types must have alignment 1 and correct
// size. These assertions guard against future changes that could break
// zero-copy access.
const _: () = assert!(align_of::<PodU16>() == 1);
const _: () = assert!(size_of::<PodU16>() == 2);
const _: () = assert!(align_of::<PodI16>() == 1);
const _: () = assert!(size_of::<PodI16>() == 2);
const _: () = assert!(align_of::<PodU32>() == 1);
const _: () = assert!(size_of::<PodU32>() == 4);
const _: () = assert!(align_of::<PodI32>() == 1);
const _: () = assert!(size_of::<PodI32>() == 4);
const _: () = assert!(align_of::<PodU64>() == 1);
const _: () = assert!(size_of::<PodU64>() == 8);
const _: () = assert!(align_of::<PodI64>() == 1);
const _: () = assert!(size_of::<PodI64>() == 8);
const _: () = assert!(align_of::<PodU128>() == 1);
const _: () = assert!(size_of::<PodU128>() == 16);
const _: () = assert!(align_of::<PodI128>() == 1);
const _: () = assert!(size_of::<PodI128>() == 16);
const _: () = assert!(align_of::<PodBool>() == 1);
const _: () = assert!(size_of::<PodBool>() == 1);

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
	use bytemuck::try_from_bytes;

	use super::*;

	// =======================================================================
	// PodBool tests
	// =======================================================================

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
	fn pod_bool_not() {
		assert_eq!(!PodBool::from_bool(true), PodBool::from_bool(false));
		assert_eq!(!PodBool::from_bool(false), PodBool::from_bool(true));
		// Non-canonical values treated as true
		assert_eq!(!PodBool(42), PodBool::from_bool(false));
	}

	#[test]
	fn pod_bool_display() {
		assert_eq!(std::format!("{}", PodBool::from_bool(true)), "true");
		assert_eq!(std::format!("{}", PodBool::from_bool(false)), "false");
	}

	// =======================================================================
	// Conversion roundtrip tests
	// =======================================================================

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

	// =======================================================================
	// Boundary value tests
	// =======================================================================

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

	// =======================================================================
	// Constants tests
	// =======================================================================

	#[test]
	fn pod_constants_zero() {
		assert!(PodU16::ZERO.is_zero());
		assert!(PodU32::ZERO.is_zero());
		assert!(PodU64::ZERO.is_zero());
		assert!(PodU128::ZERO.is_zero());
		assert!(PodI16::ZERO.is_zero());
		assert!(PodI32::ZERO.is_zero());
		assert!(PodI64::ZERO.is_zero());
		assert!(PodI128::ZERO.is_zero());
	}

	#[test]
	fn pod_constants_min_max() {
		assert_eq!(PodU16::MIN.get(), u16::MIN);
		assert_eq!(PodU16::MAX.get(), u16::MAX);
		assert_eq!(PodU32::MIN.get(), u32::MIN);
		assert_eq!(PodU32::MAX.get(), u32::MAX);
		assert_eq!(PodU64::MIN.get(), u64::MIN);
		assert_eq!(PodU64::MAX.get(), u64::MAX);
		assert_eq!(PodU128::MIN.get(), u128::MIN);
		assert_eq!(PodU128::MAX.get(), u128::MAX);
		assert_eq!(PodI16::MIN.get(), i16::MIN);
		assert_eq!(PodI16::MAX.get(), i16::MAX);
		assert_eq!(PodI32::MIN.get(), i32::MIN);
		assert_eq!(PodI32::MAX.get(), i32::MAX);
		assert_eq!(PodI64::MIN.get(), i64::MIN);
		assert_eq!(PodI64::MAX.get(), i64::MAX);
		assert_eq!(PodI128::MIN.get(), i128::MIN);
		assert_eq!(PodI128::MAX.get(), i128::MAX);
	}

	#[test]
	fn pod_is_zero_false_for_nonzero() {
		assert!(!PodU64::from_primitive(1).is_zero());
		assert!(!PodI64::from_primitive(-1).is_zero());
		assert!(!PodU128::MAX.is_zero());
	}

	// =======================================================================
	// Arithmetic tests (Add, Sub, Mul, Div, Rem)
	// =======================================================================

	#[test]
	fn pod_add_native() {
		assert_eq!((PodU64::from(10u64) + 5u64).get(), 15);
		assert_eq!((PodI32::from(10i32) + 5i32).get(), 15);
		assert_eq!((PodI32::from(-10i32) + 5i32).get(), -5);
	}

	#[test]
	fn pod_add_pod() {
		let a = PodU64::from(10u64);
		let b = PodU64::from(20u64);
		assert_eq!((a + b).get(), 30);
	}

	#[test]
	fn pod_sub_native() {
		assert_eq!((PodU64::from(10u64) - 5u64).get(), 5);
		assert_eq!((PodI32::from(-10i32) - 5i32).get(), -15);
	}

	#[test]
	fn pod_sub_pod() {
		let a = PodU64::from(20u64);
		let b = PodU64::from(5u64);
		assert_eq!((a - b).get(), 15);
	}

	#[test]
	fn pod_mul_native() {
		assert_eq!((PodU64::from(6u64) * 7u64).get(), 42);
		assert_eq!((PodI32::from(-3i32) * 4i32).get(), -12);
	}

	#[test]
	fn pod_mul_pod() {
		let a = PodU32::from(6u32);
		let b = PodU32::from(7u32);
		assert_eq!((a * b).get(), 42);
	}

	#[test]
	fn pod_div_native() {
		assert_eq!((PodU64::from(42u64) / 7u64).get(), 6);
		assert_eq!((PodI32::from(-12i32) / 4i32).get(), -3);
	}

	#[test]
	fn pod_div_pod() {
		let a = PodU64::from(42u64);
		let b = PodU64::from(7u64);
		assert_eq!((a / b).get(), 6);
	}

	#[test]
	fn pod_rem_native() {
		assert_eq!((PodU64::from(10u64) % 3u64).get(), 1);
		assert_eq!((PodI32::from(-10i32) % 3i32).get(), -1);
	}

	#[test]
	fn pod_rem_pod() {
		let a = PodU64::from(10u64);
		let b = PodU64::from(3u64);
		assert_eq!((a % b).get(), 1);
	}

	// =======================================================================
	// Assign operators
	// =======================================================================

	#[test]
	fn pod_add_assign_native() {
		let mut v = PodU64::from(10u64);
		v += 5u64;
		assert_eq!(v.get(), 15);
	}

	#[test]
	fn pod_add_assign_pod() {
		let mut v = PodU64::from(10u64);
		v += PodU64::from(5u64);
		assert_eq!(v.get(), 15);
	}

	#[test]
	fn pod_sub_assign_native() {
		let mut v = PodU64::from(10u64);
		v -= 3u64;
		assert_eq!(v.get(), 7);
	}

	#[test]
	fn pod_sub_assign_pod() {
		let mut v = PodU64::from(10u64);
		v -= PodU64::from(3u64);
		assert_eq!(v.get(), 7);
	}

	#[test]
	fn pod_mul_assign_native() {
		let mut v = PodU32::from(5u32);
		v *= 4u32;
		assert_eq!(v.get(), 20);
	}

	#[test]
	fn pod_mul_assign_pod() {
		let mut v = PodU32::from(5u32);
		v *= PodU32::from(4u32);
		assert_eq!(v.get(), 20);
	}

	#[test]
	fn pod_div_assign_native() {
		let mut v = PodU64::from(20u64);
		v /= 5u64;
		assert_eq!(v.get(), 4);
	}

	#[test]
	fn pod_div_assign_pod() {
		let mut v = PodU64::from(20u64);
		v /= PodU64::from(5u64);
		assert_eq!(v.get(), 4);
	}

	#[test]
	fn pod_rem_assign_native() {
		let mut v = PodU64::from(10u64);
		v %= 3u64;
		assert_eq!(v.get(), 1);
	}

	#[test]
	fn pod_rem_assign_pod() {
		let mut v = PodU64::from(10u64);
		v %= PodU64::from(3u64);
		assert_eq!(v.get(), 1);
	}

	// =======================================================================
	// Bitwise tests
	// =======================================================================

	#[test]
	fn pod_bitand_native() {
		assert_eq!((PodU32::from(0xFF00u32) & 0x0FF0u32).get(), 0x0F00);
	}

	#[test]
	fn pod_bitand_pod() {
		let a = PodU32::from(0xFF00u32);
		let b = PodU32::from(0x0FF0u32);
		assert_eq!((a & b).get(), 0x0F00);
	}

	#[test]
	fn pod_bitor_native() {
		assert_eq!((PodU32::from(0xFF00u32) | 0x00FFu32).get(), 0xFFFF);
	}

	#[test]
	fn pod_bitor_pod() {
		let a = PodU32::from(0xFF00u32);
		let b = PodU32::from(0x00FFu32);
		assert_eq!((a | b).get(), 0xFFFF);
	}

	#[test]
	fn pod_bitxor_native() {
		assert_eq!((PodU32::from(0xFFFFu32) ^ 0xFF00u32).get(), 0x00FF);
	}

	#[test]
	fn pod_bitxor_pod() {
		let a = PodU32::from(0xFFFFu32);
		let b = PodU32::from(0xFF00u32);
		assert_eq!((a ^ b).get(), 0x00FF);
	}

	#[test]
	fn pod_shl() {
		assert_eq!((PodU32::from(1u32) << 4).get(), 16);
	}

	#[test]
	fn pod_shr() {
		assert_eq!((PodU32::from(16u32) >> 4).get(), 1);
	}

	#[test]
	fn pod_not() {
		assert_eq!((!PodU16::from(0u16)).get(), u16::MAX);
		assert_eq!((!PodI16::from(0i16)).get(), -1i16);
	}

	// --- Bitwise assign ---

	#[test]
	fn pod_bitand_assign_native() {
		let mut v = PodU32::from(0xFF00u32);
		v &= 0x0FF0u32;
		assert_eq!(v.get(), 0x0F00);
	}

	#[test]
	fn pod_bitand_assign_pod() {
		let mut v = PodU32::from(0xFF00u32);
		v &= PodU32::from(0x0FF0u32);
		assert_eq!(v.get(), 0x0F00);
	}

	#[test]
	fn pod_bitor_assign_native() {
		let mut v = PodU32::from(0xFF00u32);
		v |= 0x00FFu32;
		assert_eq!(v.get(), 0xFFFF);
	}

	#[test]
	fn pod_bitor_assign_pod() {
		let mut v = PodU32::from(0xFF00u32);
		v |= PodU32::from(0x00FFu32);
		assert_eq!(v.get(), 0xFFFF);
	}

	#[test]
	fn pod_bitxor_assign_native() {
		let mut v = PodU32::from(0xFFFFu32);
		v ^= 0xFF00u32;
		assert_eq!(v.get(), 0x00FF);
	}

	#[test]
	fn pod_bitxor_assign_pod() {
		let mut v = PodU32::from(0xFFFFu32);
		v ^= PodU32::from(0xFF00u32);
		assert_eq!(v.get(), 0x00FF);
	}

	#[test]
	fn pod_shl_assign() {
		let mut v = PodU32::from(1u32);
		v <<= 4;
		assert_eq!(v.get(), 16);
	}

	#[test]
	fn pod_shr_assign() {
		let mut v = PodU32::from(16u32);
		v >>= 4;
		assert_eq!(v.get(), 1);
	}

	// =======================================================================
	// Neg for signed types
	// =======================================================================

	#[test]
	fn pod_neg_i16() {
		assert_eq!((-PodI16::from(5i16)).get(), -5);
		assert_eq!((-PodI16::from(-5i16)).get(), 5);
		assert_eq!((-PodI16::from(0i16)).get(), 0);
	}

	#[test]
	fn pod_neg_i32() {
		assert_eq!((-PodI32::from(42i32)).get(), -42);
	}

	#[test]
	fn pod_neg_i64() {
		assert_eq!((-PodI64::from(100i64)).get(), -100);
	}

	#[test]
	fn pod_neg_i128() {
		assert_eq!((-PodI128::from(999i128)).get(), -999);
	}

	// =======================================================================
	// Checked arithmetic
	// =======================================================================

	#[test]
	fn pod_checked_add_ok() {
		assert_eq!(PodU64::from(10u64).checked_add(5u64), Some(PodU64::from(15u64)));
	}

	#[test]
	fn pod_checked_add_overflow() {
		assert_eq!(PodU64::MAX.checked_add(1u64), None);
	}

	#[test]
	fn pod_checked_add_pod() {
		assert_eq!(
			PodU32::from(10u32).checked_add(PodU32::from(5u32)),
			Some(PodU32::from(15u32))
		);
	}

	#[test]
	fn pod_checked_sub_ok() {
		assert_eq!(PodU64::from(10u64).checked_sub(5u64), Some(PodU64::from(5u64)));
	}

	#[test]
	fn pod_checked_sub_underflow() {
		assert_eq!(PodU64::from(5u64).checked_sub(10u64), None);
	}

	#[test]
	fn pod_checked_mul_ok() {
		assert_eq!(PodU64::from(6u64).checked_mul(7u64), Some(PodU64::from(42u64)));
	}

	#[test]
	fn pod_checked_mul_overflow() {
		assert_eq!(PodU64::MAX.checked_mul(2u64), None);
	}

	#[test]
	fn pod_checked_div_ok() {
		assert_eq!(PodU64::from(42u64).checked_div(7u64), Some(PodU64::from(6u64)));
	}

	#[test]
	fn pod_checked_div_by_zero() {
		assert_eq!(PodU64::from(42u64).checked_div(0u64), None);
	}

	#[test]
	fn pod_checked_signed_overflow() {
		assert_eq!(PodI64::MIN.checked_sub(1i64), None);
		assert_eq!(PodI64::MAX.checked_add(1i64), None);
	}

	// =======================================================================
	// Saturating arithmetic
	// =======================================================================

	#[test]
	fn pod_saturating_add() {
		assert_eq!(PodU64::MAX.saturating_add(100u64), PodU64::MAX);
		assert_eq!(PodU64::from(10u64).saturating_add(5u64), PodU64::from(15u64));
	}

	#[test]
	fn pod_saturating_sub() {
		assert_eq!(PodU64::from(5u64).saturating_sub(10u64), PodU64::ZERO);
		assert_eq!(PodU64::from(10u64).saturating_sub(5u64), PodU64::from(5u64));
	}

	#[test]
	fn pod_saturating_mul() {
		assert_eq!(PodU64::MAX.saturating_mul(2u64), PodU64::MAX);
		assert_eq!(PodU64::from(6u64).saturating_mul(7u64), PodU64::from(42u64));
	}

	#[test]
	fn pod_saturating_signed() {
		assert_eq!(PodI64::MAX.saturating_add(100i64), PodI64::MAX);
		assert_eq!(PodI64::MIN.saturating_sub(100i64), PodI64::MIN);
		assert_eq!(PodI64::MAX.saturating_mul(2i64), PodI64::MAX);
		assert_eq!(PodI64::MIN.saturating_mul(2i64), PodI64::MIN);
	}

	// =======================================================================
	// Ordering tests
	// =======================================================================

	#[test]
	fn pod_ordering() {
		assert!(PodU64::from(10u64) > PodU64::from(5u64));
		assert!(PodU64::from(5u64) < PodU64::from(10u64));
		assert!(PodU64::from(5u64) == PodU64::from(5u64));

		assert!(PodI64::from(-10i64) < PodI64::from(5i64));
		assert!(PodI64::from(5i64) > PodI64::from(-10i64));
	}

	#[test]
	fn pod_partial_eq_native() {
		assert!(PodU64::from(42u64) == 42u64);
		assert!(PodI32::from(-5i32) == -5i32);
		assert!(PodU64::from(42u64) != 43u64);
	}

	#[test]
	fn pod_partial_ord_native() {
		assert!(PodU64::from(10u64) > 5u64);
		assert!(PodU64::from(5u64) < 10u64);
		assert!(PodI32::from(-10i32) < 0i32);
	}

	// =======================================================================
	// Display / Debug tests
	// =======================================================================

	#[test]
	fn pod_display() {
		assert_eq!(std::format!("{}", PodU64::from(42u64)), "42");
		assert_eq!(std::format!("{}", PodI32::from(-7i32)), "-7");
		assert_eq!(std::format!("{}", PodU128::from(0u128)), "0");
	}

	#[test]
	fn pod_debug() {
		assert_eq!(std::format!("{:?}", PodU64::from(42u64)), "PodU64(42)");
		assert_eq!(std::format!("{:?}", PodI32::from(-7i32)), "PodI32(-7)");
	}

	// =======================================================================
	// Get method tests
	// =======================================================================

	#[test]
	fn pod_get_method() {
		assert_eq!(PodU16::from(1337u16).get(), 1337);
		assert_eq!(PodI16::from(-42i16).get(), -42);
		assert_eq!(PodU32::from(0xDEAD_BEEFu32).get(), 0xDEAD_BEEF);
		assert_eq!(PodI32::from(i32::MIN).get(), i32::MIN);
		assert_eq!(PodU64::from(u64::MAX).get(), u64::MAX);
		assert_eq!(PodI64::from(i64::MAX).get(), i64::MAX);
		assert_eq!(PodU128::from(u128::MAX).get(), u128::MAX);
		assert_eq!(PodI128::from(i128::MIN).get(), i128::MIN);
	}

	// =======================================================================
	// Ergonomic usage pattern: counter increment (the motivating use case)
	// =======================================================================

	#[test]
	fn ergonomic_counter_increment() {
		// Simulates struct field usage: my_account.count += 1;
		let mut count = PodU64::from(0u64);
		count += 1u64;
		assert_eq!(count.get(), 1);
		count += 1u64;
		assert_eq!(count.get(), 2);
	}

	#[test]
	fn ergonomic_balance_arithmetic() {
		let mut balance = PodU64::from(1000u64);
		let fee = PodU64::from(25u64);
		balance -= fee;
		assert_eq!(balance.get(), 975);
	}
}
