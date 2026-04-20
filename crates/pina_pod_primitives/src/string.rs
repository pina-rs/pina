//! Fixed-capacity string with length prefix.

use core::fmt;
use core::mem::MaybeUninit;
use core::mem::align_of;
use core::mem::size_of;

use bytemuck::Pod;
use bytemuck::Zeroable;

use crate::error::PodCollectionError;
use crate::error::max_n_for_pfx;

/// A fixed-capacity string stored inline with a length prefix.
///
/// Default prefix size is `1` byte (u8), supporting strings up to 255 bytes.
/// Use `PodString<N, 2>` for up to 65,535 bytes, etc.
///
/// # Layout
/// - Bytes 0..PFX: length prefix (little-endian)
/// - Bytes PFX..PFX+N: UTF-8 data (may be partially uninitialized)
#[repr(C)]
#[derive(Copy, Clone)]
pub struct PodString<const N: usize, const PFX: usize = 1> {
	len: [u8; PFX],
	data: [MaybeUninit<u8>; N],
}

// Compile-time validation of PFX
impl<const N: usize, const PFX: usize> PodString<N, PFX> {
	/// Use this const to trigger the compile-time assertions.
	pub const VALID: () = Self::_CAP_CHECK;
	const _CAP_CHECK: () = {
		assert!(
			PFX == 1 || PFX == 2 || PFX == 4 || PFX == 8,
			"PodString<N, PFX>: PFX must be 1, 2, 4, or 8"
		);
		assert!(
			N <= max_n_for_pfx(PFX),
			"PodString<N, PFX>: N exceeds the maximum value representable by the PFX-byte length \
			 prefix"
		);
	};
}

impl<const N: usize, const PFX: usize> PodString<N, PFX> {
	#[inline(always)]
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

	#[inline(always)]
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

	/// Returns the logical length of the string (clamped to capacity).
	#[inline(always)]
	pub fn len(&self) -> usize {
		self.decode_len().min(N)
	}

	/// Returns `true` if the string is empty.
	#[inline(always)]
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// Returns the maximum capacity.
	pub const fn capacity(&self) -> usize {
		N
	}

	/// Returns the string as a `&str`.
	///
	/// # Safety
	/// This assumes the stored bytes are valid UTF-8. For untrusted account
	/// data, use `try_as_str()` instead.
	#[inline(always)]
	pub unsafe fn as_str_unchecked(&self) -> &str {
		let len = self.len();
		let bytes = core::slice::from_raw_parts(self.data.as_ptr() as *const u8, len);
		core::str::from_utf8_unchecked(bytes)
	}

	/// Returns the string as a `&str`, validating UTF-8.
	pub fn try_as_str(&self) -> Result<&str, PodCollectionError> {
		let len = self.len();
		let bytes = unsafe { core::slice::from_raw_parts(self.data.as_ptr() as *const u8, len) };
		core::str::from_utf8(bytes).map_err(|_| PodCollectionError::InvalidUtf8)
	}

	/// Returns the raw bytes (may include trailing garbage — use `len()` for valid slice).
	pub fn as_bytes(&self) -> &[u8] {
		let len = self.len();
		unsafe { core::slice::from_raw_parts(self.data.as_ptr() as *const u8, len) }
	}

	/// Sets the string to a new value, returning error if too long.
	pub fn try_set(&mut self, value: &str) -> Result<(), PodCollectionError> {
		let vlen = value.len();
		if vlen > N {
			return Err(PodCollectionError::Overflow);
		}
		unsafe {
			core::ptr::copy_nonoverlapping(value.as_ptr(), self.data.as_mut_ptr() as *mut u8, vlen);
		}
		self.encode_len(vlen);
		Ok(())
	}

	/// Sets the string to a new value.
	///
	/// Returns `false` if the value was truncated due to exceeding capacity.
	#[must_use = "returns false if value exceeds capacity"]
	pub fn set(&mut self, value: &str) -> bool {
		self.try_set(value).is_ok()
	}

	/// Appends a string slice, returning error if capacity exceeded.
	pub fn try_push_str(&mut self, value: &str) -> Result<(), PodCollectionError> {
		let cur = self.len();
		let vlen = value.len();
		let new_len = cur + vlen;
		if new_len > N {
			return Err(PodCollectionError::Overflow);
		}
		unsafe {
			core::ptr::copy_nonoverlapping(
				value.as_ptr(),
				(self.data.as_mut_ptr() as *mut u8).add(cur),
				vlen,
			);
		}
		self.encode_len(new_len);
		Ok(())
	}

	/// Appends a string slice.
	///
	/// Returns `false` if appending would exceed capacity.
	#[must_use = "returns false if append would exceed capacity"]
	pub fn push_str(&mut self, value: &str) -> bool {
		self.try_push_str(value).is_ok()
	}

	/// Clears the string (sets length to 0).
	pub fn clear(&mut self) {
		self.len = [0u8; PFX];
	}
}

impl<const N: usize, const PFX: usize> Default for PodString<N, PFX> {
	fn default() -> Self {
		Self {
			len: [0u8; PFX],
			data: [MaybeUninit::uninit(); N],
		}
	}
}

impl<const N: usize, const PFX: usize> core::ops::Deref for PodString<N, PFX> {
	type Target = str;

	fn deref(&self) -> &str {
		unsafe { self.as_str_unchecked() }
	}
}

impl<const N: usize, const PFX: usize> AsRef<str> for PodString<N, PFX> {
	fn as_ref(&self) -> &str {
		unsafe { self.as_str_unchecked() }
	}
}

impl<const N: usize, const PFX: usize> AsRef<[u8]> for PodString<N, PFX> {
	fn as_ref(&self) -> &[u8] {
		self.as_bytes()
	}
}

impl<const N: usize, const PFX: usize> PartialEq for PodString<N, PFX> {
	fn eq(&self, other: &Self) -> bool {
		self.as_bytes() == other.as_bytes()
	}
}

impl<const N: usize, const PFX: usize> Eq for PodString<N, PFX> {}

impl<const N: usize, const PFX: usize> PartialEq<str> for PodString<N, PFX> {
	fn eq(&self, other: &str) -> bool {
		self.as_bytes() == other.as_bytes()
	}
}

impl<const N: usize, const PFX: usize> fmt::Debug for PodString<N, PFX> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self.try_as_str() {
			Ok(s) => fmt::Debug::fmt(s, f),
			Err(_) => {
				f.debug_struct("PodString")
					.field("len", &self.len())
					.finish()
			}
		}
	}
}

impl<const N: usize, const PFX: usize> fmt::Display for PodString<N, PFX> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self.try_as_str() {
			Ok(s) => f.write_str(s),
			Err(_) => write!(f, "<invalid utf8>"),
		}
	}
}

// SAFETY: PodString is #[repr(C)] with len: [u8; PFX] + data: [MaybeUninit<u8>; N].
// Both have align 1 and any bit pattern is valid.
unsafe impl<const N: usize, const PFX: usize> Zeroable for PodString<N, PFX> {}
unsafe impl<const N: usize, const PFX: usize> Pod for PodString<N, PFX> {}

// Compile-time layout assertions
const _: () = assert!(align_of::<PodString<0>>() == 1);
const _: () = assert!(size_of::<PodString<0>>() == 1);
const _: () = assert!(size_of::<PodString<32>>() == 33);
const _: () = assert!(size_of::<PodString<255>>() == 256);
const _: () = assert!(size_of::<PodString<0, 2>>() == 2);
const _: () = assert!(size_of::<PodString<100, 2>>() == 102);

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {
	use super::*;

	#[kani::proof]
	fn encode_decode_roundtrip_pfx1() {
		let n: usize = kani::any();
		kani::assume(n <= u8::MAX as usize);
		let mut s = PodString::<255, 1>::default();
		s.encode_len(n);
		assert!(s.decode_len() == n);
	}

	#[kani::proof]
	fn encode_decode_roundtrip_pfx2() {
		let n: usize = kani::any();
		kani::assume(n <= u16::MAX as usize);
		let mut s = PodString::<255, 2>::default();
		s.encode_len(n);
		assert!(s.decode_len() == n);
	}

	#[kani::proof]
	fn len_clamp_pfx1() {
		let raw: [u8; 1] = kani::any();
		let s = PodString::<8, 1> {
			len: raw,
			data: [MaybeUninit::uninit(); 8],
		};
		assert!(s.len() <= 8);
	}

	#[kani::proof]
	fn len_clamp_pfx2() {
		let raw: [u8; 2] = kani::any();
		let s = PodString::<8, 2> {
			len: raw,
			data: [MaybeUninit::uninit(); 8],
		};
		assert!(s.len() <= 8);
	}

	#[kani::proof]
	#[kani::unwind(10)]
	fn set_then_as_bytes_len() {
		let vlen: usize = kani::any();
		kani::assume(vlen <= 8);
		let content = [0x41u8; 8];
		let mut s = PodString::<8>::default();
		let ok = s.set(unsafe { core::str::from_utf8_unchecked(&content[..vlen]) });
		assert!(ok);
		assert!(s.len() == vlen);
		assert!(s.as_bytes().len() == vlen);
	}

	#[kani::proof]
	fn set_rejects_over_capacity() {
		let vlen: usize = kani::any();
		kani::assume(vlen > 4);
		kani::assume(vlen <= 8);
		let content = [0x41u8; 8];
		let mut s = PodString::<4>::default();
		assert!(!s.set(unsafe { core::str::from_utf8_unchecked(&content[..vlen]) }));
	}

	#[kani::proof]
	#[kani::unwind(10)]
	fn push_str_len_accounting() {
		let a_len: usize = kani::any();
		let b_len: usize = kani::any();
		kani::assume(a_len <= 4);
		kani::assume(b_len <= 4);
		kani::assume(a_len + b_len <= 8);

		let buf = [0x41u8; 8];
		let mut s = PodString::<8>::default();
		assert!(s.set(unsafe { core::str::from_utf8_unchecked(&buf[..a_len]) }));
		assert!(s.push_str(unsafe { core::str::from_utf8_unchecked(&buf[..b_len]) }));
		assert!(s.len() == a_len + b_len);
	}

	#[kani::proof]
	fn push_str_rejects_overflow() {
		let a_len: usize = kani::any();
		let b_len: usize = kani::any();
		kani::assume(a_len <= 4);
		kani::assume(b_len <= 8);
		kani::assume(a_len + b_len > 4);

		let buf = [0x41u8; 8];
		let mut s = PodString::<4>::default();
		assert!(s.set(unsafe { core::str::from_utf8_unchecked(&buf[..a_len]) }));
		assert!(!s.push_str(unsafe { core::str::from_utf8_unchecked(&buf[..b_len]) }));
		assert!(s.len() == a_len);
	}
}
