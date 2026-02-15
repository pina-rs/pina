use pina_macros::error;

/// Built-in pina framework errors.
///
/// These occupy the top end of the `u32` range to avoid collisions with
/// user-defined program errors.
#[error(crate = crate)]
#[derive(Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PinaProgramError {
	/// More account keys were provided than the instruction expects.
	TooManyAccountKeys = 0xFFFF_FFFE,
	/// The discriminator bytes do not match any known variant.
	InvalidDiscriminator = 0xFFFF_FFFF,
}
