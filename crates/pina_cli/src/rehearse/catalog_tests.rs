use super::*;
use crate::ir::AccountIr;
use crate::ir::DiscriminatorIr;
use crate::ir::FieldIr;
use crate::ir::InstructionIr;

fn field(name: &str, rust_type: &str) -> FieldIr {
	FieldIr {
		name: name.to_owned(),
		rust_type: rust_type.to_owned(),
		docs: Vec::new(),
	}
}

fn account(name: &str, value: u64, docs: &[&str], fields: Vec<FieldIr>) -> AccountIr {
	AccountIr {
		name: name.to_owned(),
		fields,
		discriminator: DiscriminatorIr {
			value,
			repr_size: 1,
		},
		docs: docs.iter().map(|doc| (*doc).to_owned()).collect(),
		pda_name: None,
	}
}

fn program(accounts: Vec<AccountIr>) -> ProgramIr {
	ProgramIr {
		name: "fixture".to_owned(),
		public_key: "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS".to_owned(),
		pinapod_enums: Vec::new(),
		accounts,
		instructions: vec![InstructionIr {
			name: "increment".to_owned(),
			rust_name: "IncrementInstruction".to_owned(),
			accounts: Vec::new(),
			arguments: Vec::new(),
			discriminator: DiscriminatorIr {
				value: 1,
				repr_size: 1,
			},
			docs: Vec::new(),
		}],
		events: Vec::new(),
		errors: Vec::new(),
		pdas: Vec::new(),
	}
}

fn counter_catalog(version_type: Option<MigrationVersionType>) -> ProgramCatalog {
	ProgramCatalog::new(
		&program(vec![account(
			"counterState",
			1,
			&[crate::ir::MIGRATABLE_DOC_MARKER],
			vec![field("bump", "u8"), field("count", "u64")],
		)]),
		version_type,
	)
}

#[test]
fn names_instructions_by_discriminator() {
	let catalog = counter_catalog(Some(MigrationVersionType::U8));

	assert_eq!(catalog.instruction_name(&[1]), "increment");
	assert_eq!(catalog.instruction_name(&[5, 1]), "unknown (0x05)");
	assert_eq!(catalog.instruction_name(&[]), "unknown (no data)");
	assert!(!Discriminator { value: 1, width: 9 }.matches(&[1; 9]));
}

#[test]
fn decodes_counter_fields_from_captured_account_bytes() {
	let catalog = counter_catalog(Some(MigrationVersionType::U8));
	// The real `CounterState` bytes Surfpool returned before and after an
	// increment: discriminator 1, version 0, bump 255, count 3 then 4.
	let before = [1_u8, 0, 255, 3, 0, 0, 0, 0, 0, 0, 0];
	let after = [1_u8, 0, 255, 4, 0, 0, 0, 0, 0, 0, 0];
	let layout = catalog
		.account_layout(&before)
		.unwrap_or_else(|| panic!("counter discriminator is known"));

	assert_eq!(layout.name(), "counterState");
	assert_eq!(
		layout.compare(&before, &after),
		DataComparison {
			fields: vec![FieldChange {
				name: "count".to_owned(),
				baseline: "3".to_owned(),
				candidate: "4".to_owned(),
			}],
			ranges: Vec::new(),
			omitted_ranges: 0,
		}
	);
	assert!(catalog.account_layout(&[9, 0]).is_none());
}

#[test]
fn reports_header_changes_and_bytes_outside_known_fields() {
	let catalog = counter_catalog(Some(MigrationVersionType::U8));
	let layout = catalog
		.account_layout(&[1])
		.unwrap_or_else(|| panic!("counter discriminator is known"));
	let before = [1_u8, 0, 255, 3, 0, 0, 0, 0, 0, 0, 0];
	let resized = [1_u8, 1, 255, 3, 0, 0, 0, 0, 0, 0, 0, 7, 7];
	let comparison = layout.compare(&before, &resized);

	assert_eq!(
		comparison.fields,
		vec![FieldChange {
			name: "(migration version)".to_owned(),
			baseline: "0".to_owned(),
			candidate: "1".to_owned(),
		}]
	);
	assert_eq!(comparison.ranges, vec![ByteRange { start: 11, end: 13 }]);
}

#[test]
fn unknown_version_width_decodes_only_the_discriminator() {
	let catalog = counter_catalog(None);
	let layout = catalog
		.account_layout(&[1])
		.unwrap_or_else(|| panic!("counter discriminator is known"));
	let comparison = layout.compare(&[1, 0, 9], &[2, 0, 8]);

	assert_eq!(
		comparison.fields,
		vec![FieldChange {
			name: "(discriminator)".to_owned(),
			baseline: "1".to_owned(),
			candidate: "2".to_owned(),
		}]
	);
	assert_eq!(comparison.ranges, vec![ByteRange { start: 2, end: 3 }]);
}

#[test]
fn compact_and_unsupported_layouts_fall_back_to_byte_ranges() {
	let catalog = ProgramCatalog::new(
		&program(vec![
			account(
				"compactState",
				2,
				&[crate::ir::COMPACT_ACCOUNT_DOC_MARKER],
				vec![field("name", "PodString<8>")],
			),
			account("opaqueState", 3, &[], vec![field("inner", "Custom")]),
		]),
		None,
	);

	for discriminator in [2_u8, 3] {
		let layout = catalog
			.account_layout(&[discriminator])
			.unwrap_or_else(|| panic!("discriminator {discriminator} is known"));
		let comparison = layout.compare(&[discriminator, 1, 2], &[discriminator, 1, 3]);
		assert_eq!(comparison.fields, Vec::<FieldChange>::new());
		assert_eq!(comparison.ranges, vec![ByteRange { start: 2, end: 3 }]);
	}
}

#[test]
fn decodes_every_primitive_kind() {
	let address = [7_u8; 32];

	assert_eq!(
		decode(FieldKind::Unsigned, &u64::MAX.to_le_bytes()),
		u64::MAX.to_string()
	);
	assert_eq!(decode(FieldKind::Signed, &(-2_i16).to_le_bytes()), "-2");
	assert_eq!(decode(FieldKind::Signed, &5_i32.to_le_bytes()), "5");
	assert_eq!(decode(FieldKind::Bool, &[1]), "true");
	assert_eq!(decode(FieldKind::Bool, &[0]), "false");
	assert_eq!(decode(FieldKind::Bool, &[2]), "0x02");
	assert_eq!(
		decode(FieldKind::Address, &address),
		bs58::encode(address).into_string()
	);
	assert_eq!(decode(FieldKind::Float, &1.5_f32.to_le_bytes()), "1.5");
	assert_eq!(decode(FieldKind::Float, &2.25_f64.to_le_bytes()), "2.25");
	assert_eq!(decode(FieldKind::Bytes, &[0xab, 0x01]), "0xab01");
	assert_eq!(
		decode(FieldKind::Unsigned, &[0; 17]),
		format!("0x{}", "00".repeat(17))
	);

	for (rust_type, kind) in [
		("PodU64", FieldKind::Unsigned),
		("PodI32", FieldKind::Signed),
		("PodBool", FieldKind::Bool),
		("Address", FieldKind::Address),
		("f64", FieldKind::Float),
		("[u8; 4]", FieldKind::Bytes),
	] {
		assert_eq!(field_kind(rust_type), kind);
	}
}

#[test]
fn bounds_the_number_of_reported_ranges() {
	let before = vec![0_u8; 64];
	let after = (0..64)
		.map(|index| u8::from(index % 2 == 0))
		.collect::<Vec<_>>();
	let comparison = compare_raw(&before, &after);

	assert_eq!(comparison.ranges.len(), MAX_BYTE_RANGES);
	assert_eq!(comparison.ranges[0], ByteRange { start: 0, end: 1 });
	assert_eq!(comparison.omitted_ranges, 32 - MAX_BYTE_RANGES);
	assert_eq!(compare_raw(&[1, 2], &[1, 2]), DataComparison::default());
}
