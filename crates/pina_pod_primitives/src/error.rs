//! Error type for Pod collection operations.

use core::fmt;

/// Error type for collection operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PodCollectionError {
	/// Value exceeds capacity.
	Overflow,
	/// Invalid UTF-8 in string data.
	InvalidUtf8,
	/// Index out of bounds.
	OutOfBounds,
}

impl fmt::Display for PodCollectionError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Overflow => write!(f, "value exceeds capacity"),
			Self::InvalidUtf8 => write!(f, "invalid UTF-8"),
			Self::OutOfBounds => write!(f, "index out of bounds"),
		}
	}
}

/// Returns the maximum `N` value representable by a `PFX`-byte length prefix.
pub(crate) const fn max_n_for_pfx(pfx: usize) -> usize {
	match pfx {
		1 => u8::MAX as usize,
		2 => u16::MAX as usize,
		4 => u32::MAX as usize,
		8 => usize::MAX,
		_ => 0,
	}
}
