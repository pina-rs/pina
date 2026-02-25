use pina::PinaProgramError;
use pina::ProgramError;

fn error_to_code(variant: PinaProgramError) -> u32 {
	let program_error: ProgramError = variant.into();
	match program_error {
		ProgramError::Custom(code) => code,
		_ => panic!("expected Custom error variant"),
	}
}

/// Verify that all PinaProgramError variants have the correct error codes
/// in the top end of the u32 range (0xFFFF_0000..=0xFFFF_FFFF).
#[test]
fn error_codes_match_expected_discriminants() {
	assert_eq!(error_to_code(PinaProgramError::DataTooShort), 0xFFFF_FFFA);
	assert_eq!(
		error_to_code(PinaProgramError::InvalidAccountSize),
		0xFFFF_FFFB
	);
	assert_eq!(
		error_to_code(PinaProgramError::InvalidTokenOwner),
		0xFFFF_FFFC
	);
	assert_eq!(error_to_code(PinaProgramError::SeedsTooMany), 0xFFFF_FFFD);
	assert_eq!(
		error_to_code(PinaProgramError::TooManyAccountKeys),
		0xFFFF_FFFE
	);
	assert_eq!(
		error_to_code(PinaProgramError::InvalidDiscriminator),
		0xFFFF_FFFF
	);
}

/// Error codes should occupy the reserved top range and not collide with
/// typical user error codes (which start at 0).
#[test]
fn error_codes_are_in_reserved_range() {
	let variants = [
		PinaProgramError::DataTooShort,
		PinaProgramError::InvalidAccountSize,
		PinaProgramError::InvalidTokenOwner,
		PinaProgramError::SeedsTooMany,
		PinaProgramError::TooManyAccountKeys,
		PinaProgramError::InvalidDiscriminator,
	];

	for variant in &variants {
		let code = error_to_code(*variant);
		assert!(
			code >= 0xFFFF_0000,
			"error code {code:#X} is outside the reserved range"
		);
	}
}

/// All six variants should have distinct error codes.
#[test]
fn error_codes_are_unique() {
	let codes: Vec<u32> = [
		PinaProgramError::DataTooShort,
		PinaProgramError::InvalidAccountSize,
		PinaProgramError::InvalidTokenOwner,
		PinaProgramError::SeedsTooMany,
		PinaProgramError::TooManyAccountKeys,
		PinaProgramError::InvalidDiscriminator,
	]
	.iter()
	.map(|v| error_to_code(*v))
	.collect();

	for (i, code) in codes.iter().enumerate() {
		for (j, other) in codes.iter().enumerate() {
			if i != j {
				assert_ne!(code, other, "duplicate error code at indices {i} and {j}");
			}
		}
	}
}

/// PartialEq works correctly for all variants.
#[test]
fn error_variant_equality() {
	assert!(PinaProgramError::DataTooShort == PinaProgramError::DataTooShort);
	assert!(PinaProgramError::DataTooShort != PinaProgramError::InvalidDiscriminator);
}

/// Verify that conversion to ProgramError::Custom is consistent.
#[test]
fn error_into_program_error_is_custom() {
	let variants = [
		PinaProgramError::DataTooShort,
		PinaProgramError::InvalidAccountSize,
		PinaProgramError::InvalidTokenOwner,
		PinaProgramError::SeedsTooMany,
		PinaProgramError::TooManyAccountKeys,
		PinaProgramError::InvalidDiscriminator,
	];

	for variant in &variants {
		let pe: ProgramError = (*variant).into();
		assert!(matches!(pe, ProgramError::Custom(_)));
	}
}
