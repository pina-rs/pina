use bytemuck::Pod;
use pinocchio::error::ProgramError;

/// Reinterprets a byte slice as `&T` (zero-copy). Returns an error if the
/// slice has incorrect length or alignment.
pub fn pod_from_bytes<T: Pod>(bytes: &[u8]) -> Result<&T, ProgramError> {
	bytemuck::try_from_bytes(bytes).map_err(|_| ProgramError::InvalidArgument)
}

#[cfg(test)]
mod tests {
	use pina_pod_primitives::*;

	use super::*;

	#[test]
	fn test_pod_bool() {
		assert!(pod_from_bytes::<PodBool>(&[]).is_err());
		assert!(pod_from_bytes::<PodBool>(&[0, 0]).is_err());

		for i in 0..=u8::MAX {
			assert_eq!(i != 0, bool::from(pod_from_bytes::<PodBool>(&[i]).unwrap()));
		}
	}

	#[test]
	fn test_pod_u16() {
		assert!(pod_from_bytes::<PodU16>(&[]).is_err());
		assert_eq!(1u16, u16::from(*pod_from_bytes::<PodU16>(&[1, 0]).unwrap()));
	}

	#[test]
	fn test_pod_i16() {
		assert!(pod_from_bytes::<PodI16>(&[]).is_err());
		assert_eq!(
			-1i16,
			i16::from(*pod_from_bytes::<PodI16>(&[255, 255]).unwrap())
		);
	}

	#[test]
	fn test_pod_u64() {
		assert!(pod_from_bytes::<PodU64>(&[]).is_err());
		assert_eq!(
			1u64,
			u64::from(*pod_from_bytes::<PodU64>(&[1, 0, 0, 0, 0, 0, 0, 0]).unwrap())
		);
	}

	#[test]
	fn test_pod_i64() {
		assert!(pod_from_bytes::<PodI64>(&[]).is_err());
		assert_eq!(
			-1i64,
			i64::from(
				*pod_from_bytes::<PodI64>(&[255, 255, 255, 255, 255, 255, 255, 255]).unwrap()
			)
		);
	}

	#[test]
	fn test_pod_i32() {
		assert!(pod_from_bytes::<PodI32>(&[]).is_err());
		assert_eq!(
			-1i32,
			i32::from(*pod_from_bytes::<PodI32>(&[255, 255, 255, 255]).unwrap())
		);
		assert_eq!(
			1i32,
			i32::from(*pod_from_bytes::<PodI32>(&[1, 0, 0, 0]).unwrap())
		);
	}

	#[test]
	fn test_pod_u128() {
		assert!(pod_from_bytes::<PodU128>(&[]).is_err());
		assert_eq!(
			1u128,
			u128::from(
				*pod_from_bytes::<PodU128>(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
					.unwrap()
			)
		);
	}

	#[test]
	fn test_pod_i128() {
		assert!(pod_from_bytes::<PodI128>(&[]).is_err());
		assert_eq!(
			-1i128,
			i128::from(
				*pod_from_bytes::<PodI128>(&[
					255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
				])
				.unwrap()
			)
		);
		assert_eq!(
			1i128,
			i128::from(
				*pod_from_bytes::<PodI128>(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
					.unwrap()
			)
		);
	}
}
