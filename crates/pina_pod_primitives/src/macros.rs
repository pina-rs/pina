//! Internal macros for Pod integer type code generation.
//!
//! These macros are `#[macro_export]` so they are available at the crate root
//! for use by other modules (e.g. `numeric.rs`).

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
#[macro_export]
macro_rules! impl_pod_common {
	($name:ident, $native:ty, $size:expr) => {
		impl $name {
			/// The largest value representable by the underlying integer type.
			pub const MAX: Self = Self(<$native>::MAX.to_le_bytes());
			/// The smallest value representable by the underlying integer type.
			pub const MIN: Self = Self(<$native>::MIN.to_le_bytes());
			/// The zero value.
			pub const ZERO: Self = Self([0u8; $size]);

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

		impl core::fmt::Display for $name {
			fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
				self.get().fmt(f)
			}
		}
	};
}

/// Implements arithmetic operators for a Pod type.
///
/// In debug builds, operators panic on overflow via `checked_*`. In release
/// builds, they use `wrapping_*` for CU efficiency on Solana.
#[macro_export]
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
#[macro_export]
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
#[macro_export]
macro_rules! define_pod_unsigned {
	($name:ident, $native:ty, $size:expr, $doc:expr) => {
		#[doc = $doc]
		#[derive(Clone, Copy, Default, PartialEq, Eq, Pod, Zeroable)]
		#[repr(transparent)]
		pub struct $name(pub [u8; $size]);

		$crate::impl_int_conversion!($name, $native);
		$crate::impl_pod_common!($name, $native, $size);
		$crate::impl_pod_arithmetic!($name, $native);

		impl core::fmt::Debug for $name {
			fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
				write!(f, "{}({})", stringify!($name), self.get())
			}
		}
	};
}

/// Defines a signed Pod integer type with full operator support.
#[macro_export]
macro_rules! define_pod_signed {
	($name:ident, $native:ty, $size:expr, $doc:expr) => {
		#[doc = $doc]
		#[derive(Clone, Copy, Default, PartialEq, Eq, Pod, Zeroable)]
		#[repr(transparent)]
		pub struct $name(pub [u8; $size]);

		$crate::impl_int_conversion!($name, $native);
		$crate::impl_pod_common!($name, $native, $size);
		$crate::impl_pod_arithmetic!($name, $native);
		$crate::impl_pod_neg!($name, $native);

		impl core::fmt::Debug for $name {
			fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
				write!(f, "{}({})", stringify!($name), self.get())
			}
		}
	};
}
