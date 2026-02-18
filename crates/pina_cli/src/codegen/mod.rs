use codama_nodes::AccountNode;
use codama_nodes::Base16;
use codama_nodes::ConstantDiscriminatorNode;
use codama_nodes::ConstantPdaSeedNode;
use codama_nodes::ConstantValueNode;
use codama_nodes::DiscriminatorNode;
use codama_nodes::ErrorNode;
use codama_nodes::InstructionAccountNode;
use codama_nodes::InstructionArgumentNode;
use codama_nodes::InstructionInputValueNode;
use codama_nodes::InstructionNode;
use codama_nodes::IsAccountSigner;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::NumberValueNode;
use codama_nodes::PdaNode;
use codama_nodes::PdaSeedNode;
use codama_nodes::ProgramNode;
use codama_nodes::PublicKeyValueNode;
use codama_nodes::RootNode;
use codama_nodes::StringTypeNode;
use codama_nodes::StringValueNode;
use codama_nodes::StructFieldTypeNode;
use codama_nodes::StructTypeNode;
use codama_nodes::VariablePdaSeedNode;

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

/// Convert a `ProgramIr` into a Codama `RootNode`.
pub fn ir_to_root_node(ir: &ProgramIr) -> RootNode {
	let mut program = ProgramNode::new(ir.name.as_str(), ir.public_key.as_str());

	for account in &ir.accounts {
		program = program.add_account(build_account_node(account));
	}

	for instruction in &ir.instructions {
		program = program.add_instruction(build_instruction_node(instruction));
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
	let fields: Vec<StructFieldTypeNode> = account.fields.iter().map(build_struct_field).collect();

	let data = StructTypeNode::new(fields);
	let mut node = AccountNode::new(account.name.as_str(), data);
	node.discriminators = vec![build_discriminator_node(&account.discriminator)];

	if !account.docs.is_empty() {
		node.docs = account.docs.clone().into();
	}

	node
}

fn build_instruction_node(instruction: &InstructionIr) -> InstructionNode {
	let accounts: Vec<InstructionAccountNode> = instruction
		.accounts
		.iter()
		.map(build_instruction_account_node)
		.collect();

	let arguments: Vec<InstructionArgumentNode> = instruction
		.arguments
		.iter()
		.map(|f| InstructionArgumentNode::new(f.name.as_str(), rust_type_to_codama(&f.rust_type)))
		.collect();

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

fn build_instruction_account_node(account: &InstructionAccountIr) -> InstructionAccountNode {
	let is_signer = if account.is_signer {
		IsAccountSigner::True
	} else {
		IsAccountSigner::False
	};

	let mut node =
		InstructionAccountNode::new(account.name.as_str(), account.is_writable, is_signer);
	node.is_optional = account.is_optional;

	if !account.docs.is_empty() {
		node.docs = account.docs.clone().into();
	}

	if let Some(default_value) = &account.default_value {
		node.default_value = Some(build_default_value(default_value));
	}

	node
}

fn build_default_value(default_value: &DefaultValueIr) -> InstructionInputValueNode {
	match default_value {
		DefaultValueIr::ProgramId(addr) | DefaultValueIr::PublicKey(addr) => {
			InstructionInputValueNode::PublicKey(PublicKeyValueNode::new(addr.as_str()))
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

fn build_discriminator_node(disc: &DiscriminatorIr) -> DiscriminatorNode {
	let format = match disc.repr_size {
		2 => NumberFormat::U16,
		4 => NumberFormat::U32,
		8 => NumberFormat::U64,
		_ => NumberFormat::U8,
	};

	DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
		ConstantValueNode::new(NumberTypeNode::le(format), NumberValueNode::new(disc.value)),
		0,
	))
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

	let mut node = ErrorNode::new(error.name.as_str(), error.code as usize, message);

	if !error.docs.is_empty() {
		node.docs = error.docs.clone().into();
	}

	node
}
