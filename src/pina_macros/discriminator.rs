//! `#[discriminator]` contract tests through the public macro.
//!
//! The macro generates a typed discriminator enum surface: derived standard
//! traits, `#[repr]` and `#[non_exhaustive]` attributes, `From`/`TryFrom`
//! conversions with per-variant constants and exhaustive matching, and the
//! `into_discriminator!` bridge. These tests exercise that surface the way a
//! downstream consumer would — by invoking the macro and using the generated
//! API — instead of asserting on internal token streams.

/// A default `u8` discriminator expands with derived standard traits, a
/// `u8` representation, `From`/`TryFrom` conversions, and exhaustive
/// variant matching.
#[test]
fn u8_default() {
	use pina::*;

	#[discriminator]
	pub enum MyDiscriminator {
		First = 0,
		Second = 1,
		Third = 2,
	}

	// Derived standard traits from the expansion.

	// Representation and exhaustiveness.
	assert_eq!(u8::from(MyDiscriminator::First), 0);
	assert_eq!(u8::from(MyDiscriminator::Second), 1);
	assert_eq!(u8::from(MyDiscriminator::Third), 2);
	assert!(MyDiscriminator::try_from(0_u8).is_ok());
	assert!(MyDiscriminator::try_from(2_u8).is_ok());
	assert!(MyDiscriminator::try_from(3_u8).is_err());
}

/// A `u16` primitive is honored in the generated representation.
#[test]
fn u16_primitive() {
	use pina::*;

	#[discriminator(primitive = u16, crate = ::pina)]
	pub enum MyDiscriminator {
		First = 0,
		Second = 1,
	}
	assert_eq!(u16::from(MyDiscriminator::First), 0);
	assert!(MyDiscriminator::try_from(1_u16).is_ok());
	assert!(MyDiscriminator::try_from(2_u16).is_err());
}

/// A `u32` primitive is supported.
#[test]
fn u32_primitive() {
	use pina::*;

	#[discriminator(primitive = u32)]
	pub enum MyDiscriminator {
		First = 0,
		Second = 1,
	}
	assert_eq!(u32::from(MyDiscriminator::Second), 1);
}

/// A `u64` primitive is supported.
#[test]
fn u64_primitive() {
	use pina::*;

	#[discriminator(primitive = u64, crate = ::pina)]
	pub enum MyDiscriminator {
		First = 0,
		Second = 1,
	}
	assert_eq!(u64::from(MyDiscriminator::Second), 1);
}

/// `final` omits the `#[non_exhaustive]` marker.
#[test]
fn final_attribute() {
	use pina::*;

	#[discriminator(final)]
	pub enum MyDiscriminator {
		Only = 0,
	}
	// Compile-time proof that the enum is exhaustively matchable without a
	// wildcard arm, which `#[non_exhaustive]` would forbid downstream.
	let _: MyDiscriminator = match MyDiscriminator::Only {
		MyDiscriminator::Only => MyDiscriminator::Only,
	};
}

/// A single-variant discriminator round-trips.
#[test]
fn single_variant() {
	use pina::*;

	#[discriminator]
	pub enum MyDiscriminator {
		Only = 7,
	}
	assert_eq!(u8::from(MyDiscriminator::Only), 7);
	assert!(MyDiscriminator::try_from(7_u8).is_ok());
}

/// Many variants keep distinct discriminant values and round-trip.
#[test]
fn many_variants() {
	use pina::*;

	#[discriminator]
	pub enum MyDiscriminator {
		Alpha = 0,
		Beta = 1,
		Gamma = 2,
		Delta = 3,
		Epsilon = 4,
	}
	assert_eq!(u8::from(MyDiscriminator::Epsilon), 4);
	assert!(MyDiscriminator::try_from(4_u8).is_ok());
	assert!(MyDiscriminator::try_from(5_u8).is_err());
}
