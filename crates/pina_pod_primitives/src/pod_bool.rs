//! The standard `bool` is not a `Pod`, define a replacement that is.

use core::fmt;

use bytemuck::Pod;
use bytemuck::Zeroable;

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

const _: () = assert!(align_of::<PodBool>() == 1);
const _: () = assert!(size_of::<PodBool>() == 1);
