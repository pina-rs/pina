//! Typed instruction construction helpers for client-side usage.
//!
//! This module provides lightweight utilities for building instruction data and
//! account metadata tuples from pina types. The helpers are `no_std` compatible
//! and work with any `HasDiscriminator + Pod` type.
//!
//! # Account metadata helpers
//!
//! The free functions [`writable_signer`], [`writable`], [`readonly_signer`],
//! and [`readonly`] produce `(Address, bool, bool)` tuples suitable for passing
//! to instruction builders in downstream SDKs.
//!
//! # Instruction data
//!
//! [`InstructionBuilder::data`] returns the raw byte representation of a `Pod`
//! instruction struct (which already includes the discriminator as its first
//! field thanks to the `#[instruction]` macro).

use bytemuck::Pod;

use crate::Address;
use crate::HasDiscriminator;

/// Builds instruction data for types that implement [`HasDiscriminator`] and
/// [`Pod`].
///
/// Because `#[instruction]` types already include the discriminator as their
/// first field, `data` simply returns the `bytemuck::bytes_of` representation.
///
/// # Examples
///
/// ```
/// use bytemuck::Pod;
/// use bytemuck::Zeroable;
/// use pina::HasDiscriminator;
/// use pina::IntoDiscriminator;
/// use pina::transaction::InstructionBuilder;
///
/// #[repr(C)]
/// #[derive(Copy, Clone, Pod, Zeroable)]
/// struct MyInstruction {
/// 	discriminator: u8,
/// 	amount: [u8; 8],
/// }
///
/// impl HasDiscriminator for MyInstruction {
/// 	type Type = u8;
///
/// 	const VALUE: u8 = 1;
/// }
///
/// let ix = MyInstruction {
/// 	discriminator: 1,
/// 	amount: 42u64.to_le_bytes(),
/// };
/// let bytes = InstructionBuilder::<MyInstruction>::data(&ix);
/// assert_eq!(bytes[0], 1); // discriminator
/// assert_eq!(bytes.len(), core::mem::size_of::<MyInstruction>());
/// ```
pub struct InstructionBuilder<T: HasDiscriminator> {
	_marker: core::marker::PhantomData<T>,
}

impl<T: HasDiscriminator + Pod> InstructionBuilder<T> {
	/// Returns the raw byte representation of `instruction`.
	///
	/// The returned slice includes the discriminator prefix (which is part of
	/// the `Pod` struct layout) followed by any instruction arguments.
	#[inline]
	pub fn data(instruction: &T) -> &[u8] {
		bytemuck::bytes_of(instruction)
	}
}

/// An account metadata tuple of `(address, is_signer, is_writable)`.
///
/// This type alias mirrors the common three-field representation used by
/// Solana SDK `AccountMeta` without pulling in the full `solana-sdk`
/// dependency.
pub type AccountMetaTuple = (Address, bool, bool);

/// Creates an account metadata entry for an address that is both a signer and
/// writable.
///
/// # Examples
///
/// ```
/// use pina::Address;
/// use pina::transaction::writable_signer;
///
/// let address = Address::from([1u8; 32]);
/// let (addr, is_signer, is_writable) = writable_signer(&address);
/// assert!(is_signer);
/// assert!(is_writable);
/// assert_eq!(addr, address);
/// ```
#[inline]
pub fn writable_signer(address: &Address) -> AccountMetaTuple {
	(*address, true, true)
}

/// Creates an account metadata entry for an address that is writable but not a
/// signer.
///
/// # Examples
///
/// ```
/// use pina::Address;
/// use pina::transaction::writable;
///
/// let address = Address::from([2u8; 32]);
/// let (addr, is_signer, is_writable) = writable(&address);
/// assert!(!is_signer);
/// assert!(is_writable);
/// assert_eq!(addr, address);
/// ```
#[inline]
pub fn writable(address: &Address) -> AccountMetaTuple {
	(*address, false, true)
}

/// Creates an account metadata entry for an address that is a signer but
/// read-only.
///
/// # Examples
///
/// ```
/// use pina::Address;
/// use pina::transaction::readonly_signer;
///
/// let address = Address::from([3u8; 32]);
/// let (addr, is_signer, is_writable) = readonly_signer(&address);
/// assert!(is_signer);
/// assert!(!is_writable);
/// assert_eq!(addr, address);
/// ```
#[inline]
pub fn readonly_signer(address: &Address) -> AccountMetaTuple {
	(*address, true, false)
}

/// Creates an account metadata entry for an address that is neither a signer
/// nor writable.
///
/// # Examples
///
/// ```
/// use pina::Address;
/// use pina::transaction::readonly;
///
/// let address = Address::from([4u8; 32]);
/// let (addr, is_signer, is_writable) = readonly(&address);
/// assert!(!is_signer);
/// assert!(!is_writable);
/// assert_eq!(addr, address);
/// ```
#[inline]
pub fn readonly(address: &Address) -> AccountMetaTuple {
	(*address, false, false)
}

#[cfg(test)]
mod tests {
	#![allow(unsafe_code)]
	extern crate std;

	use bytemuck::Pod;
	use bytemuck::Zeroable;

	use super::*;
	use crate::PodU64;

	#[repr(C)]
	#[derive(Copy, Clone, Debug, Zeroable, Pod)]
	struct TestInstruction {
		discriminator: u8,
		amount: PodU64,
	}

	impl HasDiscriminator for TestInstruction {
		type Type = u8;

		const VALUE: u8 = 5;
	}

	#[test]
	fn instruction_builder_data_includes_discriminator() {
		let ix = TestInstruction {
			discriminator: 5,
			amount: PodU64::from_primitive(1000),
		};
		let bytes = InstructionBuilder::<TestInstruction>::data(&ix);
		assert_eq!(bytes[0], 5);
		assert_eq!(bytes.len(), size_of::<TestInstruction>());
	}

	#[test]
	fn instruction_builder_data_preserves_payload() {
		let ix = TestInstruction {
			discriminator: 5,
			amount: PodU64::from_primitive(u64::MAX),
		};
		let bytes = InstructionBuilder::<TestInstruction>::data(&ix);
		// First byte is discriminator.
		assert_eq!(bytes[0], 5);
		// Remaining bytes are the PodU64 in little-endian.
		assert_eq!(&bytes[1..], &u64::MAX.to_le_bytes());
	}

	#[test]
	fn writable_signer_returns_correct_tuple() {
		let addr = Address::from([0xAA; 32]);
		let (a, signer, w) = writable_signer(&addr);
		assert_eq!(a, addr);
		assert!(signer);
		assert!(w);
	}

	#[test]
	fn writable_returns_correct_tuple() {
		let addr = Address::from([0xBB; 32]);
		let (a, signer, w) = writable(&addr);
		assert_eq!(a, addr);
		assert!(!signer);
		assert!(w);
	}

	#[test]
	fn readonly_signer_returns_correct_tuple() {
		let addr = Address::from([0xCC; 32]);
		let (a, signer, w) = readonly_signer(&addr);
		assert_eq!(a, addr);
		assert!(signer);
		assert!(!w);
	}

	#[test]
	fn readonly_returns_correct_tuple() {
		let addr = Address::from([0xDD; 32]);
		let (a, signer, w) = readonly(&addr);
		assert_eq!(a, addr);
		assert!(!signer);
		assert!(!w);
	}

	#[test]
	fn zero_address_helpers() {
		let zero = Address::from([0u8; 32]);
		assert_eq!(writable_signer(&zero).0, zero);
		assert_eq!(writable(&zero).0, zero);
		assert_eq!(readonly_signer(&zero).0, zero);
		assert_eq!(readonly(&zero).0, zero);
	}
}
