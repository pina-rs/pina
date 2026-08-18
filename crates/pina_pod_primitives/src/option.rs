//! Fixed-size optional value with 1-byte discriminant.

use core::fmt;
use core::mem::MaybeUninit;
use core::mem::align_of;
use core::mem::size_of;

use bytemuck::Pod;
use bytemuck::Zeroable;

/// A fixed-size optional value with `1` byte discriminant (`0=None`, `1=Some`).
///
/// # Layout
/// - Byte 0: discriminant (`0` or `1`)
/// - Bytes `1..1+size_of::<T>()`: value (uninitialized if `None`)
#[repr(C)]
#[derive(Copy, Clone)]
pub struct PodOption<T: Pod> {
	tag: u8,
	value: MaybeUninit<T>,
}

impl<T: Pod> PodOption<T> {
	/// Creates a `None` value.
	pub const fn none() -> Self {
		Self {
			tag: 0,
			value: MaybeUninit::uninit(),
		}
	}

	/// Creates a `Some` value.
	pub const fn some(value: T) -> Self {
		Self {
			tag: 1,
			value: MaybeUninit::new(value),
		}
	}

	/// Returns `true` if the option is `None`.
	pub const fn is_none(&self) -> bool {
		self.tag == 0
	}

	/// Returns `true` if the option is `Some`.
	pub const fn is_some(&self) -> bool {
		self.tag == 1
	}

	/// Returns the value if `Some`, otherwise `None`.
	pub fn get(&self) -> Option<T> {
		if self.tag == 1 {
			// SAFETY: tag == 1 means value was initialized
			Some(unsafe { self.value.assume_init() })
		} else {
			None
		}
	}

	/// Returns a reference to the value if `Some`.
	pub fn as_ref(&self) -> Option<&T> {
		if self.tag == 1 {
			// SAFETY: tag == 1 means value was initialized
			Some(unsafe { &*self.value.as_ptr() })
		} else {
			None
		}
	}

	/// Returns a mutable reference to the value if `Some`.
	pub fn as_mut(&mut self) -> Option<&mut T> {
		if self.tag == 1 {
			// SAFETY: tag == 1 means value was initialized
			Some(unsafe { &mut *self.value.as_mut_ptr() })
		} else {
			None
		}
	}

	/// Sets the value to `Some`.
	pub fn set(&mut self, value: T) {
		self.value = MaybeUninit::new(value);
		self.tag = 1;
	}

	/// Sets the value to `None`.
	pub fn clear(&mut self) {
		self.tag = 0;
	}

	/// Returns the raw tag byte.
	pub const fn raw_tag(&self) -> u8 {
		self.tag
	}

	/// # Safety
	/// Caller must ensure this is `Some`, otherwise returns uninitialized data.
	pub unsafe fn assume_init(&self) -> &T {
		unsafe { &*self.value.as_ptr() }
	}
}

impl<T: Pod> Default for PodOption<T> {
	fn default() -> Self {
		Self::none()
	}
}

impl<T: Pod + PartialEq> PartialEq for PodOption<T> {
	fn eq(&self, other: &Self) -> bool {
		match (self.tag, other.tag) {
			(0, 0) => true,
			(1, 1) => unsafe { self.value.assume_init() == other.value.assume_init() },
			_ => false,
		}
	}
}

impl<T: Pod + Eq> Eq for PodOption<T> {}

impl<T: Pod> fmt::Debug for PodOption<T>
where
	T: fmt::Debug,
{
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self.get() {
			Some(v) => f.debug_tuple("PodOption::Some").field(&v).finish(),
			None => write!(f, "PodOption::None"),
		}
	}
}

// SAFETY: PodOption is #[repr(C)] with tag: u8 + MaybeUninit<T> where T: Pod.
// T: Pod guarantees T is align-1 and valid for any bit pattern, so the
// MaybeUninit doesn't violate Pod requirements.
unsafe impl<T: Pod> Zeroable for PodOption<T> {}
unsafe impl<T: Pod> Pod for PodOption<T> {}

// Compile-time layout assertions for PodOption
const _: () = assert!(align_of::<PodOption<u8>>() == 1);
const _: () = assert!(size_of::<PodOption<u8>>() == 2); // 1 tag + 1 value

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {
	use super::*;

	#[kani::proof]
	fn none_is_none() {
		let opt: PodOption<u8> = PodOption::none();
		assert!(opt.is_none());
		assert!(!opt.is_some());
		assert_eq!(opt.get(), None);
	}

	#[kani::proof]
	fn some_roundtrip() {
		let val: u8 = kani::any();
		let opt = PodOption::some(val);
		assert!(opt.is_some());
		assert_eq!(opt.get(), Some(val));
	}

	#[kani::proof]
	fn set_then_get() {
		let val: u8 = kani::any();
		let mut opt = PodOption::<u8>::none();
		opt.set(val);
		assert_eq!(opt.get(), Some(val));
	}

	#[kani::proof]
	fn clear_after_some() {
		let val: u8 = kani::any();
		let mut opt = PodOption::some(val);
		opt.clear();
		assert!(opt.is_none());
		assert_eq!(opt.get(), None);
	}

	#[kani::proof]
	fn tag_byte_matches_state() {
		let val: u8 = kani::any();
		let some = PodOption::some(val);
		assert_eq!(some.raw_tag(), 1);

		let none: PodOption<u8> = PodOption::none();
		assert_eq!(none.raw_tag(), 0);
	}
}
