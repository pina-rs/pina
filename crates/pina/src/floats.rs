//! Alignment-one float storage for zero-copy schemas.
//!
//! These pods live in Pina rather than `pinapod` because `ZcField` is
//! implemented for the schema-facing native types, and Pina's closed grammar
//! maps `f32`/`f64` fields to `PodF32`/`PodF64` storage when the `floats`
//! feature is enabled. Generated clients never need these types: the IDL
//! describes float fields as their backing little-endian integer bit
//! patterns.

use core::fmt;

use crate::pinapod::PinaPodError;
use crate::pinapod::ZcElem;
use crate::pinapod::ZcField;
use crate::pinapod::ZcValidate;

macro_rules! define_pod_float {
	($name:ident, $native:ty, $bits:ty, $size:expr) => {
		#[repr(transparent)]
		#[derive(Copy, Clone, Default)]
		pub struct $name([u8; $size]);

		impl $name {
			/// Zero (`+0.0`) encoded in little-endian form.
			pub const ZERO: Self = Self([0u8; $size]);

			/// Creates a value from its little-endian byte representation.
			#[inline(always)]
			pub const fn new_from_array(array: [u8; $size]) -> Self {
				Self(array)
			}

			/// Decodes the stored little-endian bit pattern.
			#[inline(always)]
			pub fn get(&self) -> $native {
				<$native>::from_bits(<$bits>::from_le_bytes(self.0))
			}

			/// Replaces the stored value with the bit pattern of `value`.
			#[inline(always)]
			pub fn set(&mut self, value: $native) {
				self.0 = value.to_bits().to_le_bytes();
			}

			/// Returns `true` if the stored bit pattern is all zeros (`+0.0`).
			#[inline(always)]
			pub fn is_zero(&self) -> bool {
				self.0 == [0u8; $size]
			}

			/// The stored bit pattern as the backing little-endian integer.
			#[inline(always)]
			pub const fn to_bits(&self) -> $bits {
				<$bits>::from_le_bytes(self.0)
			}

			/// Replaces the stored value with a raw bit pattern.
			#[inline(always)]
			pub const fn set_bits(&mut self, bits: $bits) {
				self.0 = bits.to_le_bytes();
			}
		}

		impl From<$native> for $name {
			#[inline(always)]
			fn from(value: $native) -> Self {
				Self(value.to_bits().to_le_bytes())
			}
		}

		impl From<$name> for $native {
			#[inline(always)]
			fn from(value: $name) -> Self {
				value.get()
			}
		}

		impl PartialEq for $name {
			#[inline(always)]
			fn eq(&self, other: &Self) -> bool {
				// Bitwise equality keeps `Eq` sound in the presence of NaN
				// payloads and preserves the distinction between `+0.0` and
				// `-0.0`. Pod storage is a byte container, not an algebraic
				// float.
				self.0 == other.0
			}
		}

		impl Eq for $name {}

		impl PartialOrd for $name {
			#[inline(always)]
			fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
				self.get().partial_cmp(&other.get())
			}
		}

		impl core::hash::Hash for $name {
			fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
				self.0.hash(state);
			}
		}

		impl fmt::LowerHex for $name {
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				fmt::LowerHex::fmt(&self.to_bits(), f)
			}
		}

		impl fmt::UpperHex for $name {
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				fmt::UpperHex::fmt(&self.to_bits(), f)
			}
		}

		impl fmt::Display for $name {
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				self.get().fmt(f)
			}
		}

		impl fmt::Debug for $name {
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				fmt::Debug::fmt(&self.get(), f)
			}
		}

		impl AsRef<[u8]> for $name {
			#[inline(always)]
			fn as_ref(&self) -> &[u8] {
				&self.0
			}
		}

		impl ZcValidate for $name {
			#[inline(always)]
			fn validate_ref(_: &Self) -> Result<(), PinaPodError> {
				// Every bit pattern is a memory-valid float value; NaN
				// payloads are values, not invalid states.
				Ok(())
			}
		}

		// SAFETY: `$name` is `#[repr(transparent)]` over `[u8; $size]`, so it
		// is align 1, and any bit pattern reinterpreted as `$native` is a
		// value rather than undefined behavior.
		#[allow(unsafe_code)]
		unsafe impl ZcElem for $name {}

		// SAFETY: `$name` is its own alignment-one pod; its size is derived
		// from `size_of` wherever it is used.
		#[allow(unsafe_code)]
		unsafe impl ZcField for $name {
			type Pod = Self;
		}

		const _: () = assert!(core::mem::align_of::<$name>() == 1);
		const _: () = assert!(core::mem::size_of::<$name>() == $size);
	};
}

define_pod_float!(PodF32, f32, u32, 4);
define_pod_float!(PodF64, f64, u64, 8);

#[cfg(test)]
mod tests {
	extern crate std;

	use super::*;

	#[test]
	fn roundtrips_values_through_bits() {
		for value in [
			0.0_f32,
			-1.5,
			3.141_592_7,
			f32::INFINITY,
			f32::MIN,
			f32::from_bits(0x7f80_0001), // signaling NaN payload
		] {
			let pod = PodF32::from(value);
			assert_eq!(pod.get().to_bits(), value.to_bits());
			assert_eq!(f32::from(pod).to_bits(), value.to_bits());
		}

		for value in [0.0_f64, -2.25, f64::MAX, f64::NEG_INFINITY] {
			let pod = PodF64::from(value);
			assert_eq!(pod.get(), value);
		}
	}

	#[test]
	fn wire_format_is_little_endian_bits() {
		let mut pod = PodF32::ZERO;
		pod.set(1.5);
		assert_eq!(pod.as_ref(), &1.5_f32.to_bits().to_le_bytes());
		assert_eq!(pod.to_bits(), 1.5_f32.to_bits());

		pod.set_bits(0x4049_0fdb);
		assert_eq!(pod.get().to_bits(), 0x4049_0fdb);

		let mut wide = PodF64::ZERO;
		wide.set(1.5);
		assert_eq!(wide.as_ref(), &1.5_f64.to_bits().to_le_bytes());
	}

	#[test]
	fn equality_is_bitwise() {
		assert_eq!(PodF32::from(1.5), PodF32::from(1.5));
		assert_ne!(PodF32::from(0.0), PodF32::from(-0.0));

		let quiet_nan = PodF32::from(f32::NAN);
		assert_eq!(quiet_nan, quiet_nan, "NaN payloads compare bitwise");

		assert!(PodF32::ZERO.is_zero());
		assert!(!PodF32::from(-0.0).is_zero());

		let left = PodF32::from(1.0);
		let right = PodF32::from(2.0);
		assert!(left < right);
		assert!(PodF32::from(f32::NAN).partial_cmp(&PodF32::ZERO).is_none());
	}

	#[test]
	fn display_and_debug_show_the_decoded_value() {
		let pod = PodF32::from(1.5);
		assert_eq!(std::format!("{pod}"), "1.5");
		assert_eq!(std::format!("{pod:?}"), "1.5");
		assert_eq!(std::format!("{pod:x}"), "3fc00000");
	}
}
