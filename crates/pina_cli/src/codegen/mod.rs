use codama_nodes::AccountNode;
use codama_nodes::AccountValueNode;
use codama_nodes::Base16;
use codama_nodes::ConstantDiscriminatorNode;
use codama_nodes::ConstantPdaSeedNode;
use codama_nodes::ConstantValueNode;
use codama_nodes::DefaultValueStrategy;
use codama_nodes::DiscriminatorNode;
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
use crate::parse::types::rust_type_to_codama;
use crate::parse::types::try_rust_type_to_codama;

/// Validate every IR type mapping and convert a `ProgramIr` into a Codama
/// `RootNode`.
pub fn try_ir_to_root_node(ir: &ProgramIr) -> Result<RootNode, IdlError> {
	for account in &ir.accounts {
		for field in &account.fields {
			validate_type_mapping(
				&field.rust_type,
				format!("account `{}.{}`", account.name, field.name),
			)?;
		}
	}

	for instruction in &ir.instructions {
		for argument in &instruction.arguments {
			validate_type_mapping(
				&argument.rust_type,
				format!("instruction `{}.{}`", instruction.name, argument.name),
			)?;
		}
	}

	for pda in &ir.pdas {
		for seed in &pda.seeds {
			if let PdaSeedIr::Variable { name, rust_type } = seed {
				validate_type_mapping(rust_type, format!("PDA `{}.{name}`", pda.name))?;
			}
		}
	}

	Ok(ir_to_root_node(ir))
}

fn validate_type_mapping(ty: &str, context: String) -> Result<(), IdlError> {
	try_rust_type_to_codama(ty).map(|_| ()).map_err(|reason| {
		IdlError::UnsupportedType {
			ty: ty.to_owned(),
			context,
			reason,
		}
	})
}

/// Convert a `ProgramIr` into a Codama `RootNode`.
pub fn ir_to_root_node(ir: &ProgramIr) -> RootNode {
	let mut program = ProgramNode::new(ir.name.as_str(), ir.public_key.as_str());

	for account in &ir.accounts {
		program = program.add_account(build_account_node(account));
	}

	for instruction in &ir.instructions {
		program = program.add_instruction(build_instruction_node(instruction, &ir.pdas));
	}

	for pda in &ir.pdas {
		program = program.add_pda(build_pda_node(pda));
	}

	for error in &ir.errors {
		program = program.add_error(build_error_node(error));
	}

	RootNode::new(program)
}

fn build_account_node(account: &AccountIr) -> AccountNode {
	let mut fields = vec![build_account_discriminator_field(&account.discriminator)];
	fields.extend(account.fields.iter().map(build_struct_field));

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

	node
}

fn build_instruction_node(instruction: &InstructionIr, pdas: &[PdaIr]) -> InstructionNode {
	let accounts: Vec<InstructionAccountNode> = instruction
		.accounts
		.iter()
		.map(|account| build_instruction_account_node(account, instruction, pdas))
		.collect();

	let mut arguments = vec![build_instruction_discriminator_argument(
		&instruction.discriminator,
	)];
	arguments.extend(
		instruction.arguments.iter().map(|f| {
			InstructionArgumentNode::new(f.name.as_str(), rust_type_to_codama(&f.rust_type))
		}),
	);

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

	node
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

fn build_struct_field(field: &FieldIr) -> StructFieldTypeNode {
	let type_node = rust_type_to_codama(&field.rust_type);
	let mut node = StructFieldTypeNode::new(field.name.as_str(), type_node);

	if !field.docs.is_empty() {
		node.docs = field.docs.clone().into();
	}

	node
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

fn build_pda_node(pda: &PdaIr) -> PdaNode {
	let seeds: Vec<PdaSeedNode> = pda
		.seeds
		.iter()
		.map(|seed| {
			match seed {
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
						rust_type_to_codama(rust_type),
					))
				}
			}
		})
		.collect();

	PdaNode::new(pda.name.as_str(), seeds)
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

		let root = ir_to_root_node(&ir);
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
	fn lowers_pda_instruction_account_default_from_account_seed() {
		let ir = ProgramIr {
			name: "default_program".to_string(),
			public_key: "11111111111111111111111111111111".to_string(),
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

		let root = ir_to_root_node(&ir);
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
