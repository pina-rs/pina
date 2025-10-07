use pinocchio::program_error::ProgramError;

/// A trait to add to your errors.
///
/// ```
/// use pina::*;
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive)]
/// pub struct MyError {
///   /// This is an invalid state.
///   Invalid = 0,
///   /// This is an unauthorized action.
///   Unauthorized = 1,
/// }
///
/// impl PinaError for MyError {}
/// ```
pub trait PinaError: Into<u32> + Copy {
	fn into_program_error(self) -> ProgramError {
		ProgramError::Custom(self.into())
	}
}
