//! Deterministic serialization for fixed-layout Pina values.
//!
//! Zeropod collections may keep unused capacity in `MaybeUninit`. Serializing
//! an entire object representation with a raw byte-slice cast would therefore
//! read bytes that were never initialized. These traits write fields into a
//! zeroed destination instead, preserving the fixed on-chain layout without
//! observing inactive collection storage.

use core::mem::size_of;

use crate::Address;
use crate::PodBool;
use crate::PodI16;
use crate::PodI32;
use crate::PodI64;
use crate::PodI128;
use crate::PodOption;
use crate::PodString;
use crate::PodU16;
use crate::PodU32;
use crate::PodU64;
use crate::PodU128;
use crate::PodVec;
use crate::ZcElem;

/// Writes a value's fixed-layout representation into initialized bytes.
///
/// Implementations must write exactly `size_of::<Self>()` bytes and must not
/// read uninitialized storage from `self`.
pub trait PinaSerialize {
	/// Writes `self` into `output` using its on-chain fixed layout.
	fn write_bytes(&self, output: &mut [u8]);
}

/// Produces an owned, fully initialized byte representation.
///
/// Macro-generated account, instruction, and event types implement this trait.
pub trait PinaToBytes: PinaSerialize {
	/// The fixed-size byte array returned by [`PinaToBytes::to_bytes`].
	type Bytes: AsRef<[u8]> + AsMut<[u8]>;

	/// Creates the zero-initialized output buffer for this type.
	fn zeroed_bytes() -> Self::Bytes;

	/// Serializes `self` into a zero-initialized fixed-size byte array.
	fn to_bytes(&self) -> Self::Bytes {
		let mut output = Self::zeroed_bytes();
		self.write_bytes(output.as_mut());
		output
	}
}

fn assert_output_size<T>(output: &[u8]) {
	assert_eq!(output.len(), size_of::<T>());
}

fn write_prefix(output: &mut [u8], value: usize) {
	let bytes = (value as u64).to_le_bytes();
	output.copy_from_slice(&bytes[..output.len()]);
}

impl PinaSerialize for u8 {
	fn write_bytes(&self, output: &mut [u8]) {
		assert_output_size::<Self>(output);
		output[0] = *self;
	}
}

impl PinaSerialize for i8 {
	fn write_bytes(&self, output: &mut [u8]) {
		assert_output_size::<Self>(output);
		output[0] = *self as u8;
	}
}

impl<T: PinaSerialize, const N: usize> PinaSerialize for [T; N] {
	fn write_bytes(&self, output: &mut [u8]) {
		assert_output_size::<Self>(output);
		output.fill(0);
		let item_size = size_of::<T>();
		for (index, item) in self.iter().enumerate() {
			let start = index * item_size;
			item.write_bytes(&mut output[start..start + item_size]);
		}
	}
}

macro_rules! impl_serialize_as_ref {
	($($ty:ty),+ $(,)?) => {
		$(
			impl PinaSerialize for $ty {
				fn write_bytes(&self, output: &mut [u8]) {
					assert_output_size::<Self>(output);
					output.copy_from_slice(self.as_ref());
				}
			}
		)+
	};
}

impl_serialize_as_ref!(
	PodBool, PodU16, PodU32, PodU64, PodU128, PodI16, PodI32, PodI64, PodI128,
);

impl PinaSerialize for Address {
	fn write_bytes(&self, output: &mut [u8]) {
		assert_output_size::<Self>(output);
		output.copy_from_slice(self.as_ref());
	}
}

impl<const N: usize, const PFX: usize> PinaSerialize for PodString<N, PFX> {
	fn write_bytes(&self, output: &mut [u8]) {
		assert_output_size::<Self>(output);
		output.fill(0);
		write_prefix(&mut output[..PFX], self.len());
		output[PFX..PFX + self.len()].copy_from_slice(self.as_bytes());
	}
}

impl<T: ZcElem + PinaSerialize, const N: usize, const PFX: usize> PinaSerialize
	for PodVec<T, N, PFX>
{
	fn write_bytes(&self, output: &mut [u8]) {
		assert_output_size::<Self>(output);
		output.fill(0);
		write_prefix(&mut output[..PFX], self.len());

		let item_size = size_of::<T>();
		for (index, item) in self.as_slice().iter().enumerate() {
			let start = PFX + index * item_size;
			item.write_bytes(&mut output[start..start + item_size]);
		}
	}
}

impl<T: ZcElem + PinaSerialize, const PFX: usize> PinaSerialize for PodOption<T, PFX> {
	fn write_bytes(&self, output: &mut [u8]) {
		assert_output_size::<Self>(output);
		output.fill(0);
		write_prefix(&mut output[..PFX], self.raw_tag() as usize);
		if let Some(value) = self.get_ref() {
			value.write_bytes(&mut output[PFX..]);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn partial_collections_serialize_with_zeroed_capacity() {
		let mut string = PodString::<8>::default();
		assert!(string.set("hi"));
		let mut string_bytes = [0xff; 9];
		string.write_bytes(&mut string_bytes);
		assert_eq!(string_bytes, [2, b'h', b'i', 0, 0, 0, 0, 0, 0]);

		let mut values = PodVec::<PodU16, 4>::default();
		assert!(values.push(PodU16::from(5)));
		let mut value_bytes = [0xff; 10];
		values.write_bytes(&mut value_bytes);
		assert_eq!(value_bytes, [1, 0, 5, 0, 0, 0, 0, 0, 0, 0]);
	}
}
