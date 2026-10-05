use base64::Engine as _;

use super::*;

/// A real legacy `Increment` transaction captured from Surfpool 1.6.
const LEGACY_INCREMENT: &str = "ARVq7ReEPx6GIcFULqFEm4sHH7txCqPKIw/6bIXrbnLy+vyDrRKYLPHCcquIEwQBATZkVuhdtaQZozlYjWgRVAgBAAED6kpsY+KcUgq+9VB7Ey7F+ZVHdq6+vnuSQh7qaRRG0iwA3aLyEiEGyz+tOTX0NPDIelj63P6NVvfVS1lVTmIjCeNU4kEH1fbes5Xf1hPpwCrw03CW8SU6B+xc59/lx3bFBoaFnTjICpchSBhBaRfgv4/KFX5GT5jM/Q/6nfhH29oBAgIAAQEB";

fn decode_base64(value: &str) -> Vec<u8> {
	base64::engine::general_purpose::STANDARD
		.decode(value)
		.unwrap_or_else(|error| panic!("fixture is base64: {error}"))
}

fn v0_transaction() -> Vec<u8> {
	let mut bytes = vec![1];
	bytes.extend([9_u8; 64]);
	bytes.push(0x80);
	bytes.extend([1, 0, 1]);
	bytes.push(2);
	bytes.extend([1_u8; 32]);
	bytes.extend([2_u8; 32]);
	bytes.extend([0_u8; 32]);
	bytes.push(1);
	bytes.extend([1, 1, 0, 2, 7, 8]);
	bytes.push(1);
	bytes.extend([3_u8; 32]);
	bytes.extend([1, 4, 2, 5, 6]);
	bytes
}

#[test]
fn decodes_a_captured_legacy_transaction() {
	let transaction = WireTransaction::decode(&decode_base64(LEGACY_INCREMENT))
		.unwrap_or_else(|error| panic!("decode legacy transaction: {error}"));

	assert_eq!(transaction.account_keys.len(), 3);
	assert_eq!(transaction.lookup_tables, Vec::<[u8; 32]>::new());
	assert_eq!(transaction.instructions.len(), 1);
	assert_eq!(transaction.instructions[0].data, vec![1]);
	let program = transaction
		.program_id(&transaction.instructions[0])
		.unwrap_or_else(|| panic!("program index is in range"));
	assert_eq!(
		bs58::encode(program).into_string(),
		"GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS"
	);
}

#[test]
fn decodes_v0_lookup_tables() {
	let transaction = WireTransaction::decode(&v0_transaction())
		.unwrap_or_else(|error| panic!("decode v0 transaction: {error}"));

	assert_eq!(transaction.account_keys, vec![[1_u8; 32], [2_u8; 32]]);
	assert_eq!(transaction.lookup_tables, vec![[3_u8; 32]]);
	assert_eq!(
		transaction.instructions,
		vec![WireInstruction {
			program_id_index: 1,
			data: vec![7, 8],
		}]
	);
	let outside = WireInstruction {
		program_id_index: 9,
		data: Vec::new(),
	};
	assert_eq!(transaction.program_id(&outside), None);
}

#[test]
fn rejects_truncated_and_unsupported_transactions() {
	let unsupported = {
		let mut bytes = v0_transaction();
		bytes[65] = 0x81;
		bytes
	};
	let truncated = &decode_base64(LEGACY_INCREMENT)[..100];
	let v0 = v0_transaction();
	// Cut the v0 transaction inside its instruction data.
	let mid_instruction = &v0[..v0.len() - 39];

	assert_eq!(
		WireTransaction::decode(&unsupported),
		Err("message version 1 is not supported".to_owned())
	);
	assert!(
		WireTransaction::decode(truncated)
			.unwrap_err()
			.contains("transaction ends before byte")
	);
	assert!(
		WireTransaction::decode(mid_instruction)
			.unwrap_err()
			.contains("transaction ends before byte")
	);
	assert_eq!(
		WireTransaction::decode(&[0]),
		Err("transaction has no message".to_owned())
	);
	assert_eq!(
		WireTransaction::decode(&[]),
		Err("transaction ends before byte 1".to_owned())
	);
}

#[test]
fn reads_canonical_compact_lengths() {
	let mut reader = Reader {
		bytes: &[
			0x80, 0x01, 0xff, 0xff, 0x03, 0xff, 0xff, 0x07, 0x80, 0x80, 0x80,
		],
		offset: 0,
	};

	assert_eq!(reader.compact_len(), Ok(128));
	assert_eq!(reader.compact_len(), Ok(65_535));
	assert_eq!(
		reader.compact_len(),
		Err("compact length 131071 exceeds u16".to_owned())
	);
	assert_eq!(
		reader.compact_len(),
		Err("compact length is longer than three bytes".to_owned())
	);
}
