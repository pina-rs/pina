//! Fixed-capacity vector with length prefix.

use core::fmt;
use core::mem::MaybeUninit;
use core::mem::align_of;
use core::mem::size_of;

use bytemuck::Pod;
use bytemuck::Zeroable;

use crate::PodU64;
use crate::error::PodCollectionError;
use crate::error::max_n_for_pfx;

/// A fixed-capacity vector stored inline with a length prefix.
///
/// Default prefix size is `2` bytes (u16), supporting up to 65,535 elements.
/// Use `PodVec<T, N, 1>` for up to 255 elements, etc.
///
/// # Layout
/// - Bytes 0..PFX: element count prefix (little-endian)
/// - Bytes `PFX..PFX+(N*size_of::<T>())`: element data (may be partially uninitialized)
#[repr(C)]
#[derive(Copy, Clone)]
pub struct PodVec<T: Pod, const N: usize, const PFX: usize = 2> {
	len: [u8; PFX],
	data: [MaybeUninit<T>; N],
}

// Compile-time validation of PFX
impl<T: Pod, const N: usize, const PFX: usize> PodVec<T, N, PFX> {
	/// Use this const to trigger the compile-time assertions.
	pub const VALID: () = Self::_CAP_CHECK;
	const _CAP_CHECK: () = {
		assert!(
			PFX == 1 || PFX == 2 || PFX == 4 || PFX == 8,
			"PodVec<T, N, PFX>: PFX must be 1, 2, 4, or 8"
		);
		assert!(
			N <= max_n_for_pfx(PFX),
			"PodVec<T, N, PFX>: N exceeds the maximum value representable by the PFX-byte length \
			 prefix"
		);
	};
}

impl<T: Pod, const N: usize, const PFX: usize> PodVec<T, N, PFX> {
	#[inline]
	fn decode_len(&self) -> usize {
		match PFX {
			1 => self.len[0] as usize,
			2 => u16::from_le_bytes([self.len[0], self.len[1]]) as usize,
			4 => u32::from_le_bytes([self.len[0], self.len[1], self.len[2], self.len[3]]) as usize,
			8 => {
				u64::from_le_bytes([
					self.len[0],
					self.len[1],
					self.len[2],
					self.len[3],
					self.len[4],
					self.len[5],
					self.len[6],
					self.len[7],
				]) as usize
			}
			_ => unreachable!(),
		}
	}

	#[inline]
	fn encode_len(&mut self, n: usize) {
		match PFX {
			1 => self.len[0] = n as u8,
			2 => {
				let bytes = (n as u16).to_le_bytes();
				self.len.copy_from_slice(&bytes);
			}
			4 => {
				let bytes = (n as u32).to_le_bytes();
				self.len.copy_from_slice(&bytes);
			}
			8 => {
				let bytes = (n as u64).to_le_bytes();
				self.len.copy_from_slice(&bytes);
			}
			_ => unreachable!(),
		}
	}

	/// Returns the number of elements (clamped to capacity).
	#[inline]
	pub fn len(&self) -> usize {
		self.decode_len().min(N)
	}

	/// Returns `true` if the vector is empty.
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// Returns the maximum capacity.
	pub const fn capacity(&self) -> usize {
		N
	}

	/// Returns a slice of the initialized elements.
	pub fn as_slice(&self) -> &[T] {
		let len = self.len();
		unsafe { core::slice::from_raw_parts(self.data.as_ptr().cast::<T>(), len) }
	}

	/// Returns a mutable slice of the initialized elements.
	pub fn as_mut_slice(&mut self) -> &mut [T] {
		let len = self.len();
		unsafe { core::slice::from_raw_parts_mut(self.data.as_mut_ptr().cast::<T>(), len) }
	}

	/// Returns the element at the given index.
	pub fn get(&self, index: usize) -> Option<&T> {
		if index < self.len() {
			Some(unsafe { &*self.data.as_ptr().add(index).cast::<T>() })
		} else {
			None
		}
	}

	/// Returns a mutable reference to the element at the given index.
	pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
		if index < self.len() {
			Some(unsafe { &mut *self.data.as_mut_ptr().add(index).cast::<T>() })
		} else {
			None
		}
	}

	/// Pushes an element, returning error if at capacity.
	pub fn try_push(&mut self, value: T) -> Result<(), PodCollectionError> {
		let len = self.len();
		if len >= N {
			return Err(PodCollectionError::Overflow);
		}
		unsafe {
			self.data.as_mut_ptr().add(len).cast::<T>().write(value);
		}
		self.encode_len(len + 1);
		Ok(())
	}

	/// Pushes an element.
	///
	/// Returns `false` if at capacity.
	#[must_use = "returns false if at capacity"]
	pub fn push(&mut self, value: T) -> bool {
		self.try_push(value).is_ok()
	}

	/// Pops the last element.
	pub fn pop(&mut self) -> Option<T> {
		let len = self.len();
		if len == 0 {
			return None;
		}
		let value = unsafe { self.data.as_ptr().add(len - 1).cast::<T>().read() };
		self.encode_len(len - 1);
		Some(value)
	}

	/// Clears the vector (sets length to 0).
	pub fn clear(&mut self) {
		self.len = [0u8; PFX];
	}
}

impl<T: Pod, const N: usize, const PFX: usize> Default for PodVec<T, N, PFX> {
	fn default() -> Self {
		Self {
			len: [0u8; PFX],
			data: [MaybeUninit::uninit(); N],
		}
	}
}

impl<T: Pod, const N: usize, const PFX: usize> core::ops::Deref for PodVec<T, N, PFX> {
	type Target = [T];

	fn deref(&self) -> &[T] {
		self.as_slice()
	}
}

impl<T: Pod, const N: usize, const PFX: usize> core::ops::DerefMut for PodVec<T, N, PFX> {
	fn deref_mut(&mut self) -> &mut [T] {
		self.as_mut_slice()
	}
}

impl<T: Pod, const N: usize, const PFX: usize> AsRef<[T]> for PodVec<T, N, PFX> {
	fn as_ref(&self) -> &[T] {
		self.as_slice()
	}
}

impl<T: Pod + PartialEq, const N: usize, const PFX: usize> PartialEq for PodVec<T, N, PFX> {
	fn eq(&self, other: &Self) -> bool {
		self.as_slice() == other.as_slice()
	}
}

impl<T: Pod + Eq, const N: usize, const PFX: usize> Eq for PodVec<T, N, PFX> {}

impl<T: Pod + fmt::Debug, const N: usize, const PFX: usize> fmt::Debug for PodVec<T, N, PFX> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_list().entries(self.as_slice().iter()).finish()
	}
}

// SAFETY: PodVec is #[repr(C)] with len: [u8; PFX] + data: [MaybeUninit<T>; N] where T: Pod.
// Both have align 1 and any bit pattern is valid.
unsafe impl<T: Pod, const N: usize, const PFX: usize> Zeroable for PodVec<T, N, PFX> {}
unsafe impl<T: Pod, const N: usize, const PFX: usize> Pod for PodVec<T, N, PFX> {}

// Compile-time layout assertions
const _: () = assert!(align_of::<PodVec<u8, 0>>() == 1);
const _: () = assert!(size_of::<PodVec<u8, 10>>() == 2 + 10);
const _: () = assert!(size_of::<PodVec<PodU64, 10>>() == 2 + 80);

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {
	use super::*;

	#[kani::proof]
	fn push_pop_roundtrip() {
		let val: u8 = kani::any();
		let mut v = PodVec::<u8, 4>::default();
		assert!(v.push(val));
		assert_eq!(v.len(), 1);
		assert_eq!(v.pop(), Some(val));
		assert_eq!(v.len(), 0);
	}

	#[kani::proof]
	fn overflow_rejected() {
		let mut v = PodVec::<u8, 2>::default();
		v.push(1);
		v.push(2);
		assert!(!v.push(3)); // at capacity
		assert_eq!(v.len(), 2);
	}

	#[kani::proof]
	fn empty_pop_returns_none() {
		let mut v = PodVec::<u8, 4>::default();
		assert_eq!(v.pop(), None);
	}

	#[kani::proof]
	fn clear_resets_len() {
		let val: u8 = kani::any();
		let mut v = PodVec::<u8, 4>::default();
		v.push(val);
		v.clear();
		assert!(v.is_empty());
		assert_eq!(v.len(), 0);
	}

	#[kani::proof]
	fn get_in_bounds() {
		let val: u8 = kani::any();
		let mut v = PodVec::<u8, 4>::default();
		v.push(val);
		assert_eq!(v.get(0), Some(&val));
		assert_eq!(v.get(1), None);
	}
}
