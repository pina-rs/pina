use codama_nodes::AccountNode;
use codama_nodes::AccountValueNode;
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
	let fields: Vec<StructFieldTypeNode> = account.fields.iter().map(build_struct_field).collect();

	let data = StructTypeNode::new(fields);
	let mut node = AccountNode::new(account.name.as_str(), data);
	node.discriminators = vec![build_discriminator_node(&account.discriminator)];

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

fn build_instruction_account_node(
	account: &InstructionAccountIr,
	instruction: &InstructionIr,
	pdas: &[PdaIr],
) -> InstructionAccountNode {
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
	} else if let Some(default_value) = build_pda_default_value(account, instruction, pdas) {
		node.default_value = Some(default_value);
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

			PdaSeedValueNode::new(name.as_str(), AccountValueNode::new(name.as_str()))
		} else {
			return None;
		};

		seed_values.push(value);
	}

	Some(InstructionInputValueNode::Pda(PdaValueNode::new(
		PdaLinkNode::new(pda_name.as_str()),
		seed_values,
	)))
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

#[cfg(test)]
mod tests {
	use codama_nodes::InstructionInputValueNode;
	use codama_nodes::PdaSeedValueValueNode;
	use codama_nodes::PdaValue;

	use super::*;

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
		let Some(InstructionInputValueNode::Pda(default_value)) = &account.default_value else {
			panic!("expected PDA account default");
		};

		assert!(
			matches!(&default_value.pda, PdaValue::Linked(link) if link.name.as_ref() == "state")
		);
		assert_eq!(default_value.seeds.len(), 1);
		assert!(matches!(
			&default_value.seeds[0].value,
			PdaSeedValueValueNode::Account(account) if account.name.as_ref() == "authority"
		));
	}
}
