use pina::ProgramError;
use profile_program_cpi::InitializeIx;

#[test]
fn semantic_strings_encode_the_exact_pinapod_wire() {
	let bytes = InitializeIx {
		bump: 7,
		name: "Pina",
		bio: "Rust",
	}
	.to_bytes()
	.unwrap_or_else(|error| panic!("semantic strings should encode: {error:?}"));

	// Envelope: discriminator, migration version, bump, then the payload.
	assert_eq!(bytes.len(), 165);
	assert_eq!(&bytes[..4], &[0, 0, 7, 4]);
	assert_eq!(&bytes[4..8], b"Pina");
	assert!(bytes[8..36].iter().all(|byte| *byte == 0));
	assert_eq!(bytes[36], 4);
	assert_eq!(&bytes[37..41], b"Rust");
	assert!(bytes[41..].iter().all(|byte| *byte == 0));
}

#[test]
fn semantic_string_limits_are_measured_in_utf8_bytes() {
	let valid = "é".repeat(16);
	let bytes = InitializeIx {
		bump: 0,
		name: &valid,
		bio: "",
	}
	.to_bytes()
	.unwrap_or_else(|error| panic!("32 UTF-8 bytes should encode: {error:?}"));
	assert_eq!(bytes[3], 32);

	let oversized = "é".repeat(17);
	let Err(error) = InitializeIx {
		bump: 0,
		name: &oversized,
		bio: "",
	}
	.to_bytes() else {
		panic!("34 UTF-8 bytes must be rejected");
	};
	assert_eq!(error, ProgramError::InvalidInstructionData);
}
