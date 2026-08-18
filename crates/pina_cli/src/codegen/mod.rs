use codama_nodes::AccountNode;
use codama_nodes::AccountValueNode;
use codama_nodes::Base16;
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
use codama_nodes::InstructionAccountNode;
use codama_nodes::InstructionArgumentNode;
use codama_nodes::InstructionInputValueNode;
use codama_nodes::InstructionNode;
use codama_nodes::IsSigner;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::NumberValueNode;
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
use crate::ir::ProgramIr;
use crate::ir::ZeroPodEnumIr;
use crate::parse::types::try_rust_type_to_codama_with_zeropod_enums;

/// Validate every IR type mapping and convert a `ProgramIr` into a Codama
/// `RootNode`.
pub fn try_ir_to_root_node(ir: &ProgramIr) -> Result<RootNode, IdlError> {
	let mut program = ProgramNode::new(ir.name.as_str(), ir.public_key.as_str());

	for zeropod_enum in &ir.zeropod_enums {
		program = program.add_defined_type(build_zeropod_enum_node(zeropod_enum));
	}

	for account in &ir.accounts {
		program = program.add_account(build_account_node(account, &ir.zeropod_enums)?);
	}

	for instruction in &ir.instructions {
		program = program.add_instruction(build_instruction_node(
			instruction,
			&ir.pdas,
			&ir.zeropod_enums,
		)?);
	}

	for pda in &ir.pdas {
		program = program.add_pda(build_pda_node(pda)?);
	}

	for error in &ir.errors {
		program = program.add_error(build_error_node(error));
	}

	Ok(RootNode::new(program))
}

/// Convert a `ProgramIr` into a Codama `RootNode` without silent type
/// substitutions.
pub fn ir_to_root_node(ir: &ProgramIr) -> Result<RootNode, IdlError> {
	try_ir_to_root_node(ir)
}

fn build_zeropod_enum_node(zeropod_enum: &ZeroPodEnumIr) -> DefinedTypeNode {
	let variants = zeropod_enum
		.variants
		.iter()
		.map(|variant| {
			let mut node = EnumEmptyVariantTypeNode::new(variant.name.as_str());
			node.discriminator = Some(variant.value);
			EnumVariantTypeNode::Empty(node)
		})
		.collect();
	let format = match zeropod_enum.repr_size {
		2 => NumberFormat::U16,
		4 => NumberFormat::U32,
		8 => NumberFormat::U64,
		_ => NumberFormat::U8,
	};
	let mut node = DefinedTypeNode {
		name: zeropod_enum.name.as_str().into(),
		docs: zeropod_enum.docs.clone().into(),
		r#type: Box::new(
			EnumTypeNode {
				variants,
				size: NumberTypeNode::le(format).into(),
			}
			.into(),
		),
	};
	if node.docs.is_empty() {
		node.docs = vec![format!("Zeropod schema enum `{}`.", zeropod_enum.name)].into();
	}
	node
}

fn build_account_node(
	account: &AccountIr,
	zeropod_enums: &[ZeroPodEnumIr],
) -> Result<AccountNode, IdlError> {
	let mut fields = vec![build_account_discriminator_field(&account.discriminator)];
	for field in &account.fields {
		fields.push(build_struct_field(
			field,
			format!("account `{}.{}`", account.name, field.name),
			zeropod_enums,
		)?);
	}

	let data = StructTypeNode::new(fields);
	let mut node = AccountNode::new(account.name.as_str(), data);
	node.discriminators = vec![build_discriminator_node(&account.discriminator)];
	node.pda = account
		.pda_name
		.as_ref()
		.map(|name| PdaLinkNode::new(name.as_str()));

	if !account.docs.is_empty() {
		node.docs = account.docs.clone().into();
	}

	Ok(node)
}

fn build_instruction_node(
	instruction: &InstructionIr,
	pdas: &[PdaIr],
	zeropod_enums: &[ZeroPodEnumIr],
) -> Result<InstructionNode, IdlError> {
	let accounts: Vec<InstructionAccountNode> = instruction
		.accounts
		.iter()
		.map(|account| build_instruction_account_node(account, instruction, pdas))
		.collect();

	let mut arguments = vec![build_instruction_discriminator_argument(
		&instruction.discriminator,
	)];
	for argument in &instruction.arguments {
		let r#type = map_type(
			&argument.rust_type,
			format!("instruction `{}.{}`", instruction.name, argument.name),
			zeropod_enums,
		)?;
		arguments.push(InstructionArgumentNode::new(argument.name.as_str(), r#type));
	}

	let discriminators = vec![build_discriminator_node(&instruction.discriminator)];

	let mut node = InstructionNode {
		name: instruction.name.as_str().into(),
		accounts,
		arguments,
		discriminators,
		..Default::default()
	};

	if !instruction.docs.is_empty() {
		node.docs = instruction.docs.clone().into();
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

	if let Some(default_value) = &account.default_value {
		node.default_value = Box::new(Some(build_default_value(default_value)));
	} else if let Some(default_value) = build_pda_default_value(account, instruction, pdas) {
		node.default_value = Box::new(Some(default_value));
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
	zeropod_enums: &[ZeroPodEnumIr],
) -> Result<StructFieldTypeNode, IdlError> {
	let type_node = map_type(&field.rust_type, context, zeropod_enums)?;
	let mut node = StructFieldTypeNode::new(field.name.as_str(), type_node);

	if !field.docs.is_empty() {
		node.docs = field.docs.clone().into();
	}

	Ok(node)
}

fn map_type(
	ty: &str,
	context: String,
	zeropod_enums: &[ZeroPodEnumIr],
) -> Result<codama_nodes::TypeNode, IdlError> {
	try_rust_type_to_codama_with_zeropod_enums(ty, zeropod_enums).map_err(|reason| {
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

fn build_instruction_discriminator_argument(disc: &DiscriminatorIr) -> InstructionArgumentNode {
	let (r#type, value) = build_discriminator_type_and_value(disc);
	let mut argument = InstructionArgumentNode::new("discriminator", r#type);
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

fn build_discriminator_type_and_value(disc: &DiscriminatorIr) -> (NumberTypeNode, NumberValueNode) {
	let format = match disc.repr_size {
		2 => NumberFormat::U16,
		4 => NumberFormat::U32,
		8 => NumberFormat::U64,
		_ => NumberFormat::U8,
	};

	(NumberTypeNode::le(format), NumberValueNode::new(disc.value))
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
							codama_nodes::BytesTypeNode::new(),
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
	use codama_nodes::DefaultValueStrategy;
	use codama_nodes::InstructionInputValueNode;
	use codama_nodes::NestedTypeNodeTrait;
	use codama_nodes::PdaSeedValueValue;
	use codama_nodes::PdaValuePda;
	use codama_nodes::TypeNode;
	use codama_nodes::ValueNode;

	use super::*;

	#[test]
	fn lowers_discriminators_into_encoded_data() {
		let discriminator = DiscriminatorIr {
			value: 7,
			repr_size: 1,
		};
		let ir = ProgramIr {
			name: "discriminator_program".to_string(),
			public_key: "11111111111111111111111111111111".to_string(),
			zeropod_enums: vec![],
			accounts: vec![AccountIr {
				name: "State".to_string(),
				pda_name: None,
				fields: vec![],
				discriminator: discriminator.clone(),
				docs: vec![],
			}],
			instructions: vec![InstructionIr {
				name: "update".to_string(),
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
	fn rejects_unresolved_pod_collection_layouts() {
		let ir = ProgramIr {
			name: "unsupported_collection_program".to_string(),
			public_key: "11111111111111111111111111111111".to_string(),
			zeropod_enums: vec![],
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
	fn lowers_local_zeropod_enums() {
		let ir = ProgramIr {
			name: "zeropod_enum_program".to_string(),
			public_key: "11111111111111111111111111111111".to_string(),
			zeropod_enums: vec![ZeroPodEnumIr {
				name: "Color".to_string(),
				repr_size: 1,
				variants: vec![
					crate::ir::ZeroPodEnumVariantIr {
						name: "Red".to_string(),
						value: 0,
					},
					crate::ir::ZeroPodEnumVariantIr {
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
			zeropod_enums: vec![],
			accounts: vec![],
			instructions: vec![InstructionIr {
				name: "initialize".to_string(),
				accounts: vec![
					InstructionAccountIr {
						name: "authority".to_string(),
						is_writable: false,
						is_signer: true,
						is_optional: false,
						default_value: None,
						is_pda: false,
						pda_name: None,
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
