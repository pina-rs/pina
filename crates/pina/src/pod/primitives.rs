use core::mem::size_of;

use pinocchio::error::ProgramError;

use crate::ZcElem;
use crate::ZcValidate;

/// Reinterprets a byte slice as `&T` (zero-copy), validating content first.
/// Returns an error if the slice has incorrect length or invalid content.
///
/// # Examples
///
/// ```
/// use pina::PodU64;
/// use pina::pod_from_bytes;
///
/// let bytes = [42u8, 0, 0, 0, 0, 0, 0, 0];
/// let value = pod_from_bytes::<PodU64>(&bytes).unwrap_or_else(|e| panic!("failed: {e:?}"));
/// assert_eq!(u64::from(*value), 42);
///
/// // Empty or wrong-sized slices produce an error:
/// assert!(pod_from_bytes::<PodU64>(&[]).is_err());
/// ```
#[allow(unsafe_code)]
pub fn pod_from_bytes<T: ZcElem>(bytes: &[u8]) -> Result<&T, ProgramError> {
	if bytes.len() != size_of::<T>() {
		return Err(ProgramError::InvalidArgument);
	}
	// SAFETY: `T: ZcElem` guarantees alignment 1, no padding, and that every
	// bit pattern is a valid reference. The length is checked above.
	let value = unsafe { &*(bytes.as_ptr() as *const T) };
	<T as ZcValidate>::validate_ref(value).map_err(|_| ProgramError::InvalidArgument)?;
	Ok(value)
}

/// Mutably reinterprets a byte slice as `T`, validating content first.
#[allow(unsafe_code)]
pub fn pod_from_bytes_mut<T: ZcElem>(bytes: &mut [u8]) -> Result<&mut T, ProgramError> {
	if bytes.len() != size_of::<T>() {
		return Err(ProgramError::InvalidArgument);
	}
	// SAFETY: `T: ZcElem` guarantees alignment 1, no padding, and that every
	// bit pattern is a valid reference. The length is checked above.
	let value = unsafe { &mut *(bytes.as_mut_ptr() as *mut T) };
	<T as ZcValidate>::validate_ref(value).map_err(|_| ProgramError::InvalidArgument)?;
	Ok(value)
}
