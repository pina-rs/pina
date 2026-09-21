//! Stub of pina's discriminator trait for fixture-only use.
//!
//! The lint resolves the trait through the crate name, so the auxiliary is
//! named `pina` to stand in for the real dependency.

pub trait HasDiscriminator: Sized {
	type Type;
	const VALUE: Self::Type;
}

pub trait PinaAccount {}
pub trait PinaCompactAccount {}
