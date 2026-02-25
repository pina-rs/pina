use pina_macros::error;

/// Built-in pina framework errors.
///
/// These occupy the top end of the `u32` range (`0xFFFF_0000..=0xFFFF_FFFF`)
/// to avoid collisions with user-defined program errors. User `#[error]` enums
/// should use discriminant values below `0xFFFF_0000` to prevent overlap.
///
/// <!-- {=pinaPublicResultContract|trim|linePrefix:"/// ":true} -->/// All APIs in this section are designed for on-chain determinism.
///
/// They return `ProgramError` values for caller-side propagation with `?`.
///
/// No panics needed.<!-- {/pinaPublicResultContract} -->
#[error(crate = crate)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PinaProgramError {
	/// Account or instruction data is shorter than the expected minimum.
	DataTooShort = 0xFFFF_FFFA,
	/// Account size does not match the expected type size.
	InvalidAccountSize = 0xFFFF_FFFB,
	/// Account is not owned by the expected token program.
	InvalidTokenOwner = 0xFFFF_FFFC,
	/// Too many PDA seeds were provided.
	SeedsTooMany = 0xFFFF_FFFD,
	/// More account keys were provided than the instruction expects.
	TooManyAccountKeys = 0xFFFF_FFFE,
	/// The discriminator bytes do not match any known variant.
	InvalidDiscriminator = 0xFFFF_FFFF,
}
