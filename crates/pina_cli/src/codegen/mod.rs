use codama_nodes::AccountNode;
use codama_nodes::AccountValueNode;
use codama_nodes::Base16;
use codama_nodes::BytesTypeNode;
use codama_nodes::CamelCaseString;
use codama_nodes::ConstantDiscriminatorNode;
use codama_nodes::ConstantPdaSeedNode;
use codama_nodes::ConstantValueNode;
use codama_nodes::DefaultValueStrategy;
use codama_nodes::DefinedTypeNode;
use codama_nodes::DiscriminatorNode;
use codama_nodes::EnumEmptyVariantTypeNode;
use codama_nodes::EnumTypeNode;
use codama_nodes::EnumVariantTypeNode;
use codama_nodes::ErrorNode;
use codama_nodes::EventNode;
use codama_nodes::FixedSizeTypeNode;
use codama_nodes::InstructionAccountNode;
use codama_nodes::InstructionArgumentNode;
use codama_nodes::InstructionInputValueNode;
use codama_nodes::InstructionNode;
use codama_nodes::IsSigner;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::NumberValueNode;
use codama_nodes::OptionalAccountStrategy;
use codama_nodes::PdaLinkNode;
use codama_nodes::PdaNode;
use codama_nodes::PdaSeedNode;
use codama_nodes::PdaSeedValueNode;
use codama_nodes::PdaValueNode;
use codama_nodes::ProgramNode;
use codama_nodes::PublicKeyValueNode;
use codama_nodes::RootNode;
use codama_nodes::StringTypeNode;
use codama_nodes::StringValueNode;
use codama_nodes::StructFieldTypeNode;
use codama_nodes::StructTypeNode;
use codama_nodes::VariablePdaSeedNode;
use pina_abi::ContractIdentity;
use pina_abi::ContractKind;
use pina_abi::MigrationVersionType;

use crate::compact_capacity::COMPACT_CAPACITY_MARKER_PREFIX;
use crate::compact_capacity::compact_capacity_marker_name;
use crate::error::IdlError;
use crate::ir::AccountIr;
use crate::ir::DefaultValueIr;
use crate::ir::DiscriminatorIr;
use crate::ir::ErrorIr;
use crate::ir::FieldIr;
use crate::ir::InstructionAccountIr;
use crate::ir::InstructionIr;
use crate::ir::PdaIr;
use crate::ir::PdaSeedIr;
use crate::ir::PinaPodEnumIr;
use crate::ir::ProgramIr;
use crate::migrations::IdlMigrationMetadata;
use crate::parse::types::CompactTailSchema;
use crate::parse::types::compact_tail_schema;
use crate::parse::types::try_rust_type_to_codama_compact_tail;
use crate::parse::types::try_rust_type_to_codama_compact_tail_at;
use crate::parse::types::try_rust_type_to_codama_with_pinapod_enums;
use crate::parse::types::type_node_size;

/// Validate every IR type mapping and convert a `ProgramIr` into a Codama
/// `RootNode`.
pub fn try_ir_to_root_node(ir: &ProgramIr) -> Result<RootNode, IdlError> {
	try_ir_to_root_node_with_migrations(ir, None)
}

/// Lower the current source contract with checked-in migration constants.
pub(crate) fn try_ir_to_root_node_with_migrations(
	ir: &ProgramIr,
	migrations: Option<&IdlMigrationMetadata>,
) -> Result<RootNode, IdlError> {
	let mut program = ProgramNode::new(ir.name.as_str(), ir.public_key.as_str());

	for pinapod_enum in &ir.pinapod_enums {
		let normalized_name = CamelCaseString::new(&pinapod_enum.name);
		if normalized_name
			.as_ref()
			.starts_with(COMPACT_CAPACITY_MARKER_PREFIX)
		{
			return Err(IdlError::Other(format!(
				"defined type `{}` uses Pina's reserved compact-capacity namespace `{}`",
				pinapod_enum.name, COMPACT_CAPACITY_MARKER_PREFIX,
			)));
		}
		program = program.add_defined_type(build_pinapod_enum_node(pinapod_enum));
	}

	for account in &ir.accounts {
		let migration = current_migration(
			ContractKind::Account,
			&account.name,
			&account.discriminator,
			account.is_migratable(),
			migrations,
		)?;
		program = program.add_account(build_account_node(account, &ir.pinapod_enums, migration)?);

		if account.is_compact() {
			for field in &account.fields {
				if let Some(schema) = compact_tail_schema(&field.rust_type).map_err(|reason| {
					IdlError::UnsupportedType {
						ty: field.rust_type.clone(),
						context: format!("account `{}.{}`", account.name, field.name),
						reason,
					}
				})? {
					program = program.add_defined_type(build_compact_capacity_marker(
						&account.name,
						&field.name,
						schema.capacity(),
					));
				}
			}
		}
	}

	for instruction in &ir.instructions {
		let migration = current_migration(
			ContractKind::Instruction,
			&instruction.name,
			&instruction.discriminator,
			instruction.is_migratable(),
			migrations,
		)?;
		program = program.add_instruction(build_instruction_node(
			instruction,
			&ir.pdas,
			&ir.pinapod_enums,
			migration,
		)?);
	}

	for event in &ir.events {
		program = program.add_event(build_event_node(event, &ir.pinapod_enums)?);
	}

	for pda in &ir.pdas {
		program = program.add_pda(build_pda_node(pda)?);
	}

	for error in &ir.errors {
		program = program.add_error(build_error_node(error));
	}

	Ok(RootNode::new(program))
}

#[derive(Clone, Copy)]
struct CurrentMigration {
	version_type: MigrationVersionType,
	version: u32,
}

fn current_migration(
	kind: ContractKind,
	name: &str,
	discriminator: &DiscriminatorIr,
	is_migratable: bool,
	metadata: Option<&IdlMigrationMetadata>,
) -> Result<Option<CurrentMigration>, IdlError> {
	if !is_migratable {
		return Ok(None);
	}

	let metadata = metadata.ok_or_else(|| {
		IdlError::Other(format!(
			"Migration-aware {kind} `{name}` requires checked-in migration metadata"
		))
	})?;
	let identity = ContractIdentity::try_new(kind, discriminator.repr_size, discriminator.value)
		.map_err(IdlError::Other)?;
	let version = metadata
		.current_versions
		.get(&identity.key())
		.copied()
		.ok_or_else(|| {
			IdlError::Other(format!(
				"Migration-aware {kind} `{name}` is missing current version metadata"
			))
		})?;

	Ok(Some(CurrentMigration {
		version_type: metadata.version_type,
		version,
	}))
}

fn build_compact_capacity_marker(account: &str, field: &str, capacity: usize) -> DefinedTypeNode {
	DefinedTypeNode::new(
		compact_capacity_marker_name(account, field),
		FixedSizeTypeNode::<codama_nodes::TypeNode>::new(BytesTypeNode::new(), capacity),
	)
}

/// Convert a `ProgramIr` into a Codama `RootNode` without silent type
/// substitutions.
pub fn ir_to_root_node(ir: &ProgramIr) -> Result<RootNode, IdlError> {
	try_ir_to_root_node(ir)
}

fn build_pinapod_enum_node(pinapod_enum: &PinaPodEnumIr) -> DefinedTypeNode {
	let variants = pinapod_enum
		.variants
		.iter()
		.map(|variant| {
			let mut node = EnumEmptyVariantTypeNode::new(variant.name.as_str());
			node.discriminator = Some(variant.value);
			EnumVariantTypeNode::Empty(node)
		})
		.collect();
	let format = match pinapod_enum.repr_size {
		2 => NumberFormat::U16,
		4 => NumberFormat::U32,
		8 => NumberFormat::U64,
		_ => NumberFormat::U8,
	};
	let mut node = DefinedTypeNode {
		name: pinapod_enum.name.as_str().into(),
		docs: pinapod_enum.docs.clone().into(),
		r#type: Box::new(
			EnumTypeNode {
				variants,
				size: NumberTypeNode::le(format).into(),
			}
			.into(),
		),
	};
	if node.docs.is_empty() {
		node.docs = vec![format!("PinaPod schema enum `{}`.", pinapod_enum.name)].into();
	}
	node
}

fn build_account_node(
	account: &AccountIr,
	pinapod_enums: &[PinaPodEnumIr],
	migration: Option<CurrentMigration>,
) -> Result<AccountNode, IdlError> {
	let mut fields = vec![build_account_discriminator_field(&account.discriminator)];
	if let Some(migration) = migration {
		fields.push(build_account_migration_field(migration));
	}
	let compact_schemas = if account.is_compact() {
		Some(
			account
				.fields
				.iter()
				.map(|field| {
					compact_tail_schema(&field.rust_type).map_err(|reason| {
						IdlError::UnsupportedType {
							ty: field.rust_type.clone(),
							context: format!("account `{}.{}`", account.name, field.name),
							reason,
						}
					})
				})
				.collect::<Result<Vec<_>, _>>()?,
		)
	} else {
		None
	};
	let first_tail = compact_schemas
		.as_ref()
		.and_then(|schemas| schemas.iter().position(Option::is_some));
	if account.is_compact() && first_tail.is_none() {
		return Err(IdlError::UnsupportedType {
			ty: account.name.clone(),
			context: format!("account `{}`", account.name),
			reason: "compact accounts require at least one dynamic compact field".to_string(),
		});
	}
	let mut header_offset = account.discriminator.repr_size
		+ migration.map_or(0, |migration| migration.version_type.bytes());
	let tail_prefix_sizes = first_tail
		.map(|start| {
			compact_schemas
				.as_ref()
				.expect("compact schemas exist when a tail was found")[start..]
				.iter()
				.enumerate()
				.map(|(tail_index, schema)| {
					schema
						.as_ref()
						.map(CompactTailSchema::header_size)
						.ok_or_else(|| {
							let field = &account.fields[start + tail_index];
							IdlError::UnsupportedType {
								ty: field.rust_type.clone(),
								context: format!("account `{}.{}`", account.name, field.name),
								reason: "inline fields cannot follow a compact tail".to_string(),
							}
						})
				})
				.collect::<Result<Vec<_>, _>>()
		})
		.transpose()?;
	let total_prefix_size = tail_prefix_sizes
		.as_ref()
		.map_or(0, |sizes| sizes.iter().sum());
	let uses_shared_header_offsets = tail_prefix_sizes
		.as_ref()
		.is_some_and(|sizes| sizes.len() > 1);
	let mut prefix_offset = 0;

	for (index, field) in account.fields.iter().enumerate() {
		let context = format!("account `{}.{}`", account.name, field.name);
		let mut node = if let Some(start) = first_tail.filter(|start| index >= *start) {
			if index == start {
				prefix_offset = header_offset;
			}
			let tail_index = index - start;
			let skip = if tail_index == 0 {
				total_prefix_size
			} else {
				0
			};
			let r#type = if uses_shared_header_offsets {
				try_rust_type_to_codama_compact_tail_at(
					&field.rust_type,
					&context,
					pinapod_enums,
					Some(prefix_offset),
					skip,
				)?
			} else {
				try_rust_type_to_codama_compact_tail(&field.rust_type, &context, pinapod_enums)?
			};
			StructFieldTypeNode::new(field.name.as_str(), r#type)
		} else {
			let node = build_struct_field(field, context, pinapod_enums)?;
			if account.is_compact() {
				header_offset = header_offset
					.checked_add(type_node_size(&node.r#type).ok_or_else(|| {
						IdlError::UnsupportedType {
							ty: field.rust_type.clone(),
							context: format!("account `{}.{}`", account.name, field.name),
							reason: "compact inline fields must have a fixed byte size".to_string(),
						}
					})?)
					.ok_or_else(|| IdlError::Other("compact header size overflowed".to_string()))?;
			}
			node
		};
		if uses_shared_header_offsets
			&& let Some(sizes) = &tail_prefix_sizes
			&& let Some(start) = first_tail
			&& index >= start
		{
			prefix_offset += sizes[index - start];
		}
		if !field.docs.is_empty() {
			node.docs = field.docs.clone().into();
		}
		fields.push(node);
	}

	let data = StructTypeNode::new(fields);
	let mut node = AccountNode::new(account.name.as_str(), data);
	node.discriminators = vec![build_discriminator_node(&account.discriminator)];
	if let Some(migration) = migration {
		node.discriminators.push(build_migration_discriminator_node(
			migration,
			account.discriminator.repr_size,
		));
	}
	node.pda = account
		.pda_name
		.as_ref()
		.map(|name| PdaLinkNode::new(name.as_str()));

	let docs = account.visible_docs();
	if !docs.is_empty() {
		node.docs = docs.into();
	}

	Ok(node)
}

fn build_instruction_node(
	instruction: &InstructionIr,
	pdas: &[PdaIr],
	pinapod_enums: &[PinaPodEnumIr],
	migration: Option<CurrentMigration>,
) -> Result<InstructionNode, IdlError> {
	let accounts: Vec<InstructionAccountNode> = instruction
		.accounts
		.iter()
		.map(|account| build_instruction_account_node(account, instruction, pdas))
		.collect();

	let mut arguments = vec![build_instruction_discriminator_argument(
		&instruction.discriminator,
	)];
	if let Some(migration) = migration {
		arguments.push(build_instruction_migration_argument(migration));
	}
	for argument in &instruction.arguments {
		let r#type = map_type(
			&argument.rust_type,
			format!("instruction `{}.{}`", instruction.name, argument.name),
			pinapod_enums,
		)?;
		arguments.push(InstructionArgumentNode::new(argument.name.as_str(), r#type));
	}

	let mut discriminators = vec![build_discriminator_node(&instruction.discriminator)];
	if let Some(migration) = migration {
		discriminators.push(build_migration_discriminator_node(
			migration,
			instruction.discriminator.repr_size,
		));
	}

	let mut node = InstructionNode {
		name: instruction.name.as_str().into(),
		accounts,
		arguments,
		discriminators,
		optional_account_strategy: instruction
			.accounts
			.iter()
			.any(|account| account.is_optional)
			.then_some(OptionalAccountStrategy::ProgramId),
		..Default::default()
	};

	let docs = instruction.visible_docs();
	if !docs.is_empty() {
		node.docs = docs.into();
	}

	Ok(node)
}

fn build_instruction_account_node(
	account: &InstructionAccountIr,
	instruction: &InstructionIr,
	pdas: &[PdaIr],
) -> InstructionAccountNode {
	let is_signer = if account.is_signer {
		IsSigner::True
	} else {
		IsSigner::False
	};

	let mut node =
		InstructionAccountNode::new(account.name.as_str(), account.is_writable, is_signer);
	node.is_optional = Some(account.is_optional);

	if !account.docs.is_empty() {
		node.docs = account.docs.clone().into();
	}

	if !account.is_optional {
		if let Some(default_value) = &account.default_value {
			node.default_value = Box::new(Some(build_default_value(default_value)));
		} else if let Some(default_value) = build_pda_default_value(account, instruction, pdas) {
			node.default_value = Box::new(Some(default_value));
		}
	}

	node
}

fn build_pda_default_value(
	account: &InstructionAccountIr,
	instruction: &InstructionIr,
	pdas: &[PdaIr],
) -> Option<InstructionInputValueNode> {
	let pda_name = account.pda_name.as_ref()?;
	let pda = pdas.iter().find(|pda| pda.name == *pda_name)?;
	let mut seed_values = Vec::new();

	for seed in &pda.seeds {
		let PdaSeedIr::Variable { name, .. } = seed else {
			continue;
		};

		let value = if let Some(seed_account) = instruction
			.accounts
			.iter()
			.find(|account| account.name == *name)
		{
			if seed_account.name == account.name
				|| seed_account.is_optional
				|| seed_account.default_value.is_some()
			{
				return None;
			}

			PdaSeedValueNode {
				name: name.as_str().into(),
				value: Box::new(AccountValueNode::new(name.as_str()).into()),
			}
		} else {
			return None;
		};

		seed_values.push(value);
	}

	Some(InstructionInputValueNode::PdaValue(PdaValueNode::new(
		PdaLinkNode::new(pda_name.as_str()),
		seed_values,
	)))
}

fn build_default_value(default_value: &DefaultValueIr) -> InstructionInputValueNode {
	match default_value {
		DefaultValueIr::ProgramId(addr) | DefaultValueIr::PublicKey(addr) => {
			InstructionInputValueNode::PublicKeyValue(PublicKeyValueNode::new(addr.as_str()))
		}
	}
}

fn build_struct_field(
	field: &FieldIr,
	context: String,
	pinapod_enums: &[PinaPodEnumIr],
) -> Result<StructFieldTypeNode, IdlError> {
	let type_node = map_type(&field.rust_type, context, pinapod_enums)?;
	let mut node = StructFieldTypeNode::new(field.name.as_str(), type_node);

	if !field.docs.is_empty() {
		node.docs = field.docs.clone().into();
	}

	Ok(node)
}

fn map_type(
	ty: &str,
	context: String,
	pinapod_enums: &[PinaPodEnumIr],
) -> Result<codama_nodes::TypeNode, IdlError> {
	try_rust_type_to_codama_with_pinapod_enums(ty, pinapod_enums).map_err(|reason| {
		IdlError::UnsupportedType {
			ty: ty.to_owned(),
			context,
			reason,
		}
	})
}

fn build_account_discriminator_field(disc: &DiscriminatorIr) -> StructFieldTypeNode {
	let (r#type, value) = build_discriminator_type_and_value(disc);
	let mut field = StructFieldTypeNode::new("discriminator", r#type);
	field.default_value = Box::new(Some(value.into()));
	field.default_value_strategy = Some(DefaultValueStrategy::Omitted);
	field
}

fn build_account_migration_field(migration: CurrentMigration) -> StructFieldTypeNode {
	let (r#type, value) = migration_type_and_value(migration);
	let mut field = StructFieldTypeNode::new("migrationVersion", r#type);
	field.default_value = Box::new(Some(value.into()));
	field.default_value_strategy = Some(DefaultValueStrategy::Omitted);
	field
}

fn build_instruction_discriminator_argument(disc: &DiscriminatorIr) -> InstructionArgumentNode {
	let (r#type, value) = build_discriminator_type_and_value(disc);
	let mut argument = InstructionArgumentNode::new("discriminator", r#type);
	argument.default_value = Box::new(Some(InstructionInputValueNode::NumberValue(value)));
	argument.default_value_strategy = Some(DefaultValueStrategy::Omitted);
	argument
}

fn build_instruction_migration_argument(migration: CurrentMigration) -> InstructionArgumentNode {
	let (r#type, value) = migration_type_and_value(migration);
	let mut argument = InstructionArgumentNode::new("migrationVersion", r#type);
	argument.default_value = Box::new(Some(InstructionInputValueNode::NumberValue(value)));
	argument.default_value_strategy = Some(DefaultValueStrategy::Omitted);
	argument
}

fn build_discriminator_node(disc: &DiscriminatorIr) -> DiscriminatorNode {
	let (r#type, value) = build_discriminator_type_and_value(disc);

	DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
		ConstantValueNode::new(r#type, value),
		0,
	))
}

fn build_migration_discriminator_node(
	migration: CurrentMigration,
	offset: usize,
) -> DiscriminatorNode {
	let (r#type, value) = migration_type_and_value(migration);
	DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
		ConstantValueNode::new(r#type, value),
		offset as u64,
	))
}

fn build_discriminator_type_and_value(disc: &DiscriminatorIr) -> (NumberTypeNode, NumberValueNode) {
	let format = match disc.repr_size {
		2 => NumberFormat::U16,
		4 => NumberFormat::U32,
		8 => NumberFormat::U64,
		_ => NumberFormat::U8,
	};

	(NumberTypeNode::le(format), NumberValueNode::new(disc.value))
}

fn migration_type_and_value(migration: CurrentMigration) -> (NumberTypeNode, NumberValueNode) {
	let format = match migration.version_type {
		MigrationVersionType::U8 => NumberFormat::U8,
		MigrationVersionType::U16 => NumberFormat::U16,
		MigrationVersionType::U32 => NumberFormat::U32,
	};

	(
		NumberTypeNode::le(format),
		NumberValueNode::new(u64::from(migration.version)),
	)
}

fn build_pda_node(pda: &PdaIr) -> Result<PdaNode, IdlError> {
	let seeds: Vec<PdaSeedNode> = pda
		.seeds
		.iter()
		.map(|seed| {
			Ok(match seed {
				PdaSeedIr::Constant { value } => {
					// Try to interpret as UTF-8 string first.
					if let Ok(s) = std::str::from_utf8(value) {
						PdaSeedNode::Constant(ConstantPdaSeedNode::new(
							StringTypeNode::utf8(),
							StringValueNode::new(s),
						))
					} else {
						// Fall back to hex-encoded bytes.
						use std::fmt::Write;
						let hex = value.iter().fold(String::new(), |mut acc, b| {
							let _ = write!(acc, "{b:02x}");
							acc
						});
						PdaSeedNode::Constant(ConstantPdaSeedNode::new(
							BytesTypeNode::new(),
							codama_nodes::BytesValueNode::new(Base16, hex),
						))
					}
				}
				PdaSeedIr::Variable { name, rust_type } => {
					PdaSeedNode::Variable(VariablePdaSeedNode::new(
						name.as_str(),
						map_type(rust_type, format!("PDA `{}.{name}`", pda.name), &[])?,
					))
				}
			})
		})
		.collect::<Result<_, IdlError>>()?;

	Ok(PdaNode::new(pda.name.as_str(), seeds))
}

fn build_error_node(error: &ErrorIr) -> ErrorNode {
	let message = error.docs.first().cloned().unwrap_or_default();

	let mut node = ErrorNode::new(error.name.as_str(), error.code, message);

	if !error.docs.is_empty() {
		node.docs = error.docs.clone().into();
	}

	node
}

#[cfg(test)]
mod tests {
	use std::collections::BTreeMap;

	use codama_nodes::DefaultValueStrategy;
	use codama_nodes::DiscriminatorNode;
	use codama_nodes::InstructionInputValueNode;
	use codama_nodes::NestedTypeNodeTrait;
	use codama_nodes::NumberFormat;
	use codama_nodes::PdaSeedValueValue;
	use codama_nodes::PdaValuePda;
	use codama_nodes::TypeNode;
	use codama_nodes::ValueNode;

	use super::*;
	use crate::ir::PdaSeedIr;

	fn discriminator_number_format(discriminators: &[DiscriminatorNode]) -> NumberFormat {
		let Some(DiscriminatorNode::Constant(discriminator)) = discriminators.first() else {
			panic!("expected a constant discriminator");
		};
		let TypeNode::Number(number_type) = discriminator.constant.r#type.as_ref() else {
			panic!("expected a numeric discriminator");
		};

		number_type.format
	}

	#[test]
	fn compact_account_capacity_uses_machine_readable_marker_not_docs() {
		let account = AccountIr {
			name: "DynamicState".to_owned(),
			fields: vec![FieldIr {
				name: "values".to_owned(),
				rust_type: "Vec<u64, 8>".to_owned(),
				docs: vec![],
			}],
			discriminator: DiscriminatorIr {
				value: 1,
				repr_size: 1,
			},
			docs: vec![
				"Dynamic values.".to_owned(),
				crate::ir::COMPACT_ACCOUNT_DOC_MARKER.to_owned(),
			],
			pda_name: None,
		};

		let ir = ProgramIr {
			name: "capacity_program".to_owned(),
			public_key: "11111111111111111111111111111111".to_owned(),
			pinapod_enums: vec![],
			accounts: vec![account],
			events: Vec::new(),
			instructions: vec![],
			errors: vec![],
			pdas: vec![],
		};
		let root =
			try_ir_to_root_node(&ir).unwrap_or_else(|error| panic!("IDL codegen failed: {error}"));
		let node = &root.program.accounts[0];
		let docs = node.docs.iter().map(String::as_str).collect::<Vec<_>>();
		let field_docs = node
			.data
			.get_nested_type_node()
			.fields
			.iter()
			.find(|field| field.name.as_ref() == "values")
			.map(|field| field.docs.iter().map(String::as_str).collect::<Vec<_>>())
			.unwrap_or_else(|| panic!("compact values field missing"));

		assert_eq!(docs, vec!["Dynamic values."]);
		assert!(field_docs.is_empty());

		let marker_name = compact_capacity_marker_name("DynamicState", "values");
		let marker = root
			.program
			.defined_types
			.iter()
			.find(|defined_type| defined_type.name.as_ref() == marker_name)
			.unwrap_or_else(|| panic!("compact capacity marker missing"));
		let TypeNode::FixedSize(fixed) = marker.r#type.as_ref() else {
			panic!("compact capacity marker must be fixed-size");
		};
		assert_eq!(fixed.size, 8);
		assert!(matches!(fixed.r#type.as_ref(), TypeNode::Bytes(_)));
		assert!(marker.docs.is_empty());
	}

	#[test]
	fn rejects_source_types_in_reserved_capacity_namespace() {
		let ir = ProgramIr {
			name: "reserved_program".to_owned(),
			public_key: "11111111111111111111111111111111".to_owned(),
			pinapod_enums: vec![PinaPodEnumIr {
				name: "PinaPodV1CompactCapacityPretender".to_owned(),
				repr_size: 1,
				variants: vec![],
				docs: vec![],
			}],
			accounts: vec![],
			events: Vec::new(),
			instructions: vec![],
			errors: vec![],
			pdas: vec![],
		};

		let error = try_ir_to_root_node(&ir)
			.expect_err("reserved capacity marker namespace must reject source types");
		assert!(
			error
				.to_string()
				.contains("reserved compact-capacity namespace")
		);
	}

	#[test]
	fn compact_account_codegen_rejects_invalid_ir_shapes() {
		fn compact_account(fields: Vec<FieldIr>) -> AccountIr {
			AccountIr {
				name: "DynamicState".to_owned(),
				fields,
				discriminator: DiscriminatorIr {
					value: 1,
					repr_size: 1,
				},
				docs: vec![crate::ir::COMPACT_ACCOUNT_DOC_MARKER.to_owned()],
				pda_name: None,
			}
		}

		fn field(name: &str, rust_type: &str) -> FieldIr {
			FieldIr {
				name: name.to_owned(),
				rust_type: rust_type.to_owned(),
				docs: vec![],
			}
		}

		let missing_tail = build_account_node(&compact_account(vec![]), &[], None)
			.expect_err("compact IR without a tail must be rejected");
		assert!(missing_tail.to_string().contains("at least one dynamic"));

		let inline_after_tail = build_account_node(
			&compact_account(vec![field("values", "Vec<u64, 8>"), field("count", "u64")]),
			&[],
			None,
		)
		.expect_err("inline fields after a tail must be rejected");
		assert!(
			inline_after_tail
				.to_string()
				.contains("cannot follow a compact tail")
		);

		let enums = [PinaPodEnumIr {
			name: "Status".to_owned(),
			repr_size: 1,
			variants: vec![],
			docs: vec![],
		}];
		let unresolved_inline_size = build_account_node(
			&compact_account(vec![
				field("status", "Status"),
				field("values", "Vec<u64, 8>"),
			]),
			&enums,
			None,
		)
		.expect_err("compact inline fields need a known byte size");
		assert!(
			unresolved_inline_size
				.to_string()
				.contains("fixed byte size")
		);

		let invalid_tail = build_account_node(
			&compact_account(vec![
				field("values", "Vec<u64, 8>"),
				field("unknown", "Vec<Unknown, 8>"),
			]),
			&[],
			None,
		)
		.expect_err("every compact tail element needs a known size");
		assert!(invalid_tail.to_string().contains("element size"));
	}

	#[test]
	fn preserves_source_discriminator_widths_in_codegen() {
		for (primitive, repr_size, format) in [
			("u16", 2, NumberFormat::U16),
			("u32", 4, NumberFormat::U32),
			("u64", 8, NumberFormat::U64),
		] {
			let source = format!(
				r#"
					declare_id!("11111111111111111111111111111111");

					#[discriminator(crate = ::pina, primitive = {primitive}, final)]
					pub enum WideDiscriminator {{
						State = 1,
						Update = 2,
					}}

					#[account(discriminator = WideDiscriminator::State)]
					pub struct State {{}}

					#[instruction(discriminator = WideDiscriminator::Update)]
					pub struct Update {{}}
				"#
			);
			let file = syn::parse_file(&source).unwrap_or_else(|e| panic!("parse failed: {e}"));
			let ir = crate::parse::assemble_program_ir(&file, "wide_discriminator")
				.unwrap_or_else(|e| panic!("IDL extraction failed: {e}"));

			assert_eq!(
				ir.accounts[0].discriminator.repr_size, repr_size,
				"account primitive {primitive}"
			);
			assert_eq!(
				ir.instructions[0].discriminator.repr_size, repr_size,
				"instruction primitive {primitive}"
			);

			let root =
				try_ir_to_root_node(&ir).unwrap_or_else(|e| panic!("IDL codegen failed: {e}"));
			assert_eq!(
				discriminator_number_format(&root.program.accounts[0].discriminators),
				format,
				"account primitive {primitive}"
			);
			assert_eq!(
				discriminator_number_format(&root.program.instructions[0].discriminators),
				format,
				"instruction primitive {primitive}"
			);
		}
	}

	#[test]
	fn lowers_discriminators_into_encoded_data() {
		let discriminator = DiscriminatorIr {
			value: 7,
			repr_size: 1,
		};
		let ir = ProgramIr {
			events: Vec::new(),
			name: "discriminator_program".to_string(),
			public_key: "11111111111111111111111111111111".to_string(),
			pinapod_enums: vec![],
			accounts: vec![AccountIr {
				name: "State".to_string(),
				pda_name: None,
				fields: vec![],
				discriminator: discriminator.clone(),
				docs: vec![],
			}],
			instructions: vec![InstructionIr {
				name: "update".to_string(),
				rust_name: "UpdateInstruction".to_string(),
				accounts: vec![],
				arguments: vec![],
				discriminator,
				docs: vec![],
			}],
			errors: vec![],
			pdas: vec![],
		};

		let root = ir_to_root_node(&ir).unwrap_or_else(|error| panic!("{error}"));
		let account_data = root.program.accounts[0].data.get_nested_type_node();
		let field = &account_data.fields[0];
		assert_eq!(field.name.as_ref(), "discriminator");
		assert_eq!(
			field.default_value_strategy,
			Some(DefaultValueStrategy::Omitted)
		);
		assert!(matches!(
			field.default_value.as_ref(),
			Some(ValueNode::Number(_))
		));

		let argument = &root.program.instructions[0].arguments[0];
		assert_eq!(argument.name.as_ref(), "discriminator");
		assert_eq!(
			argument.default_value_strategy,
			Some(DefaultValueStrategy::Omitted)
		);
		assert!(matches!(
			argument.default_value.as_ref(),
			Some(InstructionInputValueNode::NumberValue(_))
		));
	}

	#[test]
	fn lowers_only_current_migration_constants_into_encoded_data() {
		let account_discriminator = DiscriminatorIr {
			value: 1,
			repr_size: 1,
		};
		let instruction_discriminator = DiscriminatorIr {
			value: 2,
			repr_size: 1,
		};
		let ir = ProgramIr {
			events: Vec::new(),
			name: "migration_program".to_owned(),
			public_key: "11111111111111111111111111111111".to_owned(),
			pinapod_enums: vec![],
			accounts: vec![AccountIr {
				name: "State".to_owned(),
				fields: vec![],
				discriminator: account_discriminator.clone(),
				docs: vec![crate::ir::MIGRATABLE_DOC_MARKER.to_owned()],
				pda_name: None,
			}],
			instructions: vec![InstructionIr {
				name: "update".to_owned(),
				rust_name: "update".to_owned(),
				accounts: vec![],
				arguments: vec![],
				discriminator: instruction_discriminator.clone(),
				docs: vec![crate::ir::MIGRATABLE_DOC_MARKER.to_owned()],
			}],
			errors: vec![],
			pdas: vec![],
		};
		let account_key = ContractIdentity::try_new(
			ContractKind::Account,
			account_discriminator.repr_size,
			account_discriminator.value,
		)
		.unwrap_or_else(|error| panic!("account identity: {error}"))
		.key();
		let instruction_key = ContractIdentity::try_new(
			ContractKind::Instruction,
			instruction_discriminator.repr_size,
			instruction_discriminator.value,
		)
		.unwrap_or_else(|error| panic!("instruction identity: {error}"))
		.key();
		let metadata = IdlMigrationMetadata {
			version_type: MigrationVersionType::U16,
			current_versions: BTreeMap::from([(account_key.clone(), 7), (instruction_key, 9)]),
		};

		let missing = try_ir_to_root_node(&ir)
			.expect_err("migration-aware lowering without checked history must fail");
		assert!(
			missing
				.to_string()
				.contains("checked-in migration metadata")
		);
		let account_only_metadata = IdlMigrationMetadata {
			version_type: MigrationVersionType::U16,
			current_versions: BTreeMap::from([(account_key.clone(), 7)]),
		};
		let missing_instruction =
			try_ir_to_root_node_with_migrations(&ir, Some(&account_only_metadata))
				.expect_err("every migratable instruction needs a current version");
		assert!(
			missing_instruction
				.to_string()
				.contains("missing current version metadata")
		);

		let root = try_ir_to_root_node_with_migrations(&ir, Some(&metadata))
			.unwrap_or_else(|error| panic!("migration-aware IDL codegen failed: {error}"));
		let json = serde_json::to_value(root)
			.unwrap_or_else(|error| panic!("serialize generated IDL: {error}"));
		assert_eq!(
			json.pointer("/program/accounts/0/data/fields/1/name"),
			Some(&serde_json::json!("migrationVersion")),
		);
		assert_eq!(
			json.pointer("/program/accounts/0/data/fields/1/type/format"),
			Some(&serde_json::json!("u16")),
		);
		assert_eq!(
			json.pointer("/program/accounts/0/data/fields/1/defaultValue/number"),
			Some(&serde_json::json!(7)),
		);
		assert_eq!(
			json.pointer("/program/accounts/0/discriminators/1/offset"),
			Some(&serde_json::json!(1)),
		);
		assert_eq!(
			json.pointer("/program/instructions/0/arguments/1/name"),
			Some(&serde_json::json!("migrationVersion")),
		);
		assert_eq!(
			json.pointer("/program/instructions/0/arguments/1/defaultValue/number"),
			Some(&serde_json::json!(9)),
		);
		assert!(json.get("versions").is_none());
		assert!(!json.to_string().contains("transition"));

		let (version_type, version) = migration_type_and_value(CurrentMigration {
			version_type: MigrationVersionType::U32,
			version: 11,
		});
		assert_eq!(version_type.format, NumberFormat::U32);
		assert_eq!(version.number, codama_nodes::Number::UnsignedInteger(11));
	}

	#[test]
	fn optional_accounts_never_carry_default_values() {
		let ir = ProgramIr {
			name: "optional_pda_program".to_string(),
			public_key: "11111111111111111111111111111111".to_string(),
			pinapod_enums: vec![],
			accounts: vec![],
			events: Vec::new(),
			instructions: vec![InstructionIr {
				name: "touch".to_string(),
				rust_name: "touch".to_string(),
				accounts: vec![
					InstructionAccountIr {
						name: "authority".to_string(),
						is_writable: false,
						is_signer: true,
						is_optional: false,
						default_value: None,
						is_pda: false,
						pda_name: None,
						constraints: vec![],
						docs: vec![],
					},
					// An optional PDA must stay a plain optional slot.
					InstructionAccountIr {
						name: "store".to_string(),
						is_writable: true,
						is_signer: false,
						is_optional: true,
						default_value: None,
						is_pda: true,
						pda_name: Some("store".to_string()),
						constraints: vec![],
						docs: vec![],
					},
					InstructionAccountIr {
						name: "system_program".to_string(),
						is_writable: false,
						is_signer: false,
						is_optional: true,
						default_value: Some(DefaultValueIr::PublicKey(
							"11111111111111111111111111111111".to_owned(),
						)),
						is_pda: false,
						pda_name: None,
						constraints: vec![],
						docs: vec![],
					},
				],
				arguments: vec![],
				discriminator: DiscriminatorIr {
					value: 1,
					repr_size: 1,
				},
				docs: vec![],
			}],
			errors: vec![],
			pdas: vec![PdaIr {
				name: "store".to_string(),
				seeds: vec![crate::ir::PdaSeedIr::Constant {
					value: b"store".to_vec(),
				}],
			}],
		};

		let root = ir_to_root_node(&ir).unwrap_or_else(|error| panic!("{error}"));
		assert!(
			root.program.instructions[0].accounts[1]
				.default_value
				.is_none(),
			"optional accounts must not gain derived defaults"
		);
		assert!(
			root.program.instructions[0].accounts[2]
				.default_value
				.is_none(),
			"optional accounts must not retain explicit defaults"
		);
	}

	#[test]
	fn sets_program_id_strategy_only_when_accounts_are_optional() {
		let discriminator = DiscriminatorIr {
			value: 1,
			repr_size: 1,
		};
		let account = |name: &str, is_optional: bool| {
			InstructionAccountIr {
				name: name.to_string(),
				is_writable: false,
				is_signer: false,
				is_optional,
				default_value: None,
				is_pda: false,
				pda_name: None,
				constraints: vec![],
				docs: vec![],
			}
		};

		let build_ir = |accounts: Vec<InstructionAccountIr>| {
			ProgramIr {
				name: "strategy_program".to_string(),
				public_key: "11111111111111111111111111111111".to_string(),
				pinapod_enums: vec![],
				accounts: vec![],
				events: Vec::new(),
				instructions: vec![InstructionIr {
					name: "do_it".to_string(),
					rust_name: "do_it".to_string(),
					accounts,
					arguments: vec![],
					discriminator: discriminator.clone(),
					docs: vec![],
				}],
				errors: vec![],
				pdas: vec![],
			}
		};

		let required = ir_to_root_node(&build_ir(vec![
			account("authority", false),
			account("state", false),
		]))
		.unwrap_or_else(|error| panic!("{error}"));
		assert_eq!(
			required.program.instructions[0].optional_account_strategy, None,
			"required-only instructions keep the default strategy"
		);

		let optional = ir_to_root_node(&build_ir(vec![
			account("authority", false),
			account("witness", true),
		]))
		.unwrap_or_else(|error| panic!("{error}"));
		assert_eq!(
			optional.program.instructions[0].optional_account_strategy,
			Some(OptionalAccountStrategy::ProgramId)
		);
		assert_eq!(
			optional.program.instructions[0].accounts[1].is_optional,
			Some(true),
			"optional accounts are preserved on the node"
		);
	}

	#[test]
	fn rejects_unresolved_pod_collection_layouts() {
		let ir = ProgramIr {
			events: Vec::new(),
			name: "unsupported_collection_program".to_string(),
			public_key: "11111111111111111111111111111111".to_string(),
			pinapod_enums: vec![],
			accounts: vec![AccountIr {
				name: "State".to_string(),
				pda_name: None,
				fields: vec![FieldIr {
					name: "values".to_string(),
					rust_type: "PodVec<MyPod, 8>".to_string(),
					docs: vec![],
				}],
				discriminator: DiscriminatorIr {
					value: 1,
					repr_size: 1,
				},
				docs: vec![],
			}],
			instructions: vec![],
			errors: vec![],
			pdas: vec![],
		};

		let error = try_ir_to_root_node(&ir)
			.expect_err("unresolved Pod collection layouts must fail generation");
		let message = error.to_string();
		assert!(message.contains("PodVec<MyPod, 8>"));
		assert!(message.contains("State.values"));
	}

	#[test]
	fn lowers_local_pinapod_enums() {
		let ir = ProgramIr {
			events: Vec::new(),
			name: "pinapod_enum_program".to_string(),
			public_key: "11111111111111111111111111111111".to_string(),
			pinapod_enums: vec![PinaPodEnumIr {
				name: "Color".to_string(),
				repr_size: 1,
				variants: vec![
					crate::ir::PinaPodEnumVariantIr {
						name: "Red".to_string(),
						value: 0,
					},
					crate::ir::PinaPodEnumVariantIr {
						name: "Blue".to_string(),
						value: 1,
					},
				],
				docs: vec![],
			}],
			accounts: vec![AccountIr {
				name: "Palette".to_string(),
				pda_name: None,
				fields: vec![
					FieldIr {
						name: "color".to_string(),
						rust_type: "Color".to_string(),
						docs: vec![],
					},
					FieldIr {
						name: "colors".to_string(),
						rust_type: "Vec<Color, 8>".to_string(),
						docs: vec![],
					},
				],
				discriminator: DiscriminatorIr {
					value: 1,
					repr_size: 1,
				},
				docs: vec![],
			}],
			instructions: vec![],
			errors: vec![],
			pdas: vec![],
		};

		let root = try_ir_to_root_node(&ir).unwrap_or_else(|error| panic!("{error}"));
		assert_eq!(root.program.defined_types[0].name.as_ref(), "color");
		let account = root.program.accounts[0].data.get_nested_type_node();
		assert!(matches!(
			account.fields[1].r#type.as_ref(),
			TypeNode::Link(_)
		));
		assert!(matches!(
			account.fields[2].r#type.as_ref(),
			TypeNode::FixedSize(_)
		));
	}

	#[test]
	fn lowers_pda_instruction_account_default_from_account_seed() {
		let ir = ProgramIr {
			name: "default_program".to_string(),
			public_key: "11111111111111111111111111111111".to_string(),
			pinapod_enums: vec![],
			accounts: vec![],
			events: Vec::new(),
			instructions: vec![InstructionIr {
				name: "initialize".to_string(),
				rust_name: "initialize".to_string(),
				accounts: vec![
					InstructionAccountIr {
						name: "authority".to_string(),
						is_writable: false,
						is_signer: true,
						is_optional: false,
						default_value: None,
						is_pda: false,
						pda_name: None,
						constraints: vec![],
						docs: vec![],
					},
					InstructionAccountIr {
						name: "state".to_string(),
						is_writable: true,
						is_signer: false,
						is_optional: false,
						default_value: None,
						is_pda: true,
						pda_name: Some("state".to_string()),
						constraints: vec![],
						docs: vec![],
					},
				],
				arguments: vec![],
				discriminator: DiscriminatorIr {
					value: 1,
					repr_size: 1,
				},
				docs: vec![],
			}],
			errors: vec![],
			pdas: vec![PdaIr {
				name: "state".to_string(),
				seeds: vec![
					PdaSeedIr::Constant {
						value: b"state".to_vec(),
					},
					PdaSeedIr::Variable {
						name: "authority".to_string(),
						rust_type: "Pubkey".to_string(),
					},
				],
			}],
		};

		let root = ir_to_root_node(&ir).unwrap_or_else(|error| panic!("{error}"));
		let account = &root.program.instructions[0].accounts[1];
		let Some(InstructionInputValueNode::PdaValue(default_value)) =
			account.default_value.as_ref()
		else {
			panic!("expected PDA account default");
		};

		assert!(
			matches!(default_value.pda.as_ref(), PdaValuePda::PdaLink(link) if link.name.as_ref() == "state")
		);
		assert_eq!(default_value.seeds.len(), 1);
		assert!(matches!(
			default_value.seeds[0].value.as_ref(),
			PdaSeedValueValue::Account(account) if account.name.as_ref() == "authority"
		));
	}
}

/// Build a Codama event node: camel-cased name, constant discriminator at
/// offset 0, and the event's field schema as its data struct.
fn build_event_node(
	event: &crate::ir::EventIr,
	pinapod_enums: &[PinaPodEnumIr],
) -> Result<EventNode, IdlError> {
	let mut fields = Vec::with_capacity(event.fields.len());
	for field in &event.fields {
		let context = format!("event `{}.{}`", event.name, field.name);
		fields.push(build_struct_field(field, context, pinapod_enums)?);
	}

	let mut node = EventNode::new(event.name.as_str(), StructTypeNode::new(fields));
	node.discriminators = vec![build_discriminator_node(&event.discriminator)];
	if !event.docs.is_empty() {
		node.docs = event.docs.clone().into();
	}

	Ok(node)
}
