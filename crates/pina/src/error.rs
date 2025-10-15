use pina_macros::error;

/// The internal Pina Errors. To prevent clashes with your own error these
/// errors work their way backwards from the max `u32`.
#[error(crate = crate)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PinaError {
	InvalidDiscriminator = 0xFFFF_FFFF,
}
