use pina_macros::error;

/// Built-in pina framework errors.
///
/// These occupy the top end of the `u32` range (`0xFFFF_0000..=0xFFFF_FFFF`)
/// to avoid collisions with user-defined program errors. User `#[error]` enums
/// should use discriminant values below `0xFFFF_0000` to prevent overlap.
#[error(crate = crate)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PinaProgramError {
	/// More account keys were provided than the instruction expects.
	TooManyAccountKeys = 0xFFFF_FFFE,
	/// The discriminator bytes do not match any known variant.
	InvalidDiscriminator = 0xFFFF_FFFF,
}
