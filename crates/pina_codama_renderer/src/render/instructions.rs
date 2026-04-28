use codama_nodes::HasKind;
use codama_nodes::InstructionAccountNode;
use codama_nodes::InstructionInputValueNode;
use codama_nodes::InstructionNode;
use codama_nodes::InstructionOptionalAccountStrategy;
use codama_nodes::IsAccountSigner;
use codama_nodes::PdaNode;
use codama_nodes::PdaSeedNode;
use codama_nodes::PdaSeedValueValueNode;
use codama_nodes::PdaValue;
use codama_nodes::PdaValueNode;
use codama_nodes::ProgramNode;

use super::discriminator::render_constant_discriminator;
use super::helpers::pascal;
use super::helpers::program_id_const_name;
use super::helpers::render_docs;
use super::helpers::snake;
use super::seeds::render_constant_seed_expression;
use super::types::render_type_for_pod;
use crate::error::RenderError;
use crate::error::Result;

pub(crate) fn render_instructions_mod(instructions: &[InstructionNode]) -> String {
	let mut lines = Vec::new();

	for instruction in instructions {
		lines.push(format!(
			"pub(crate) mod r#{};",
			snake(instruction.name.as_ref())
		));
	}

	lines.push(String::new());

	for instruction in instructions {
		lines.push(format!(
			"pub use self::r#{}::*;",
			snake(instruction.name.as_ref())
		));
	}

	lines.join("\n")
}

pub(crate) fn render_instruction_page(
	instruction: &InstructionNode,
	program: &ProgramNode,
	primary_program_const: &str,
) -> Result<String> {
	let instruction_name = pascal(instruction.name.as_ref());
	let context = format!("instruction `{instruction_name}`");
	let discriminator = render_constant_discriminator(
		instruction.name.as_ref(),
		&instruction.discriminators,
		&context,
	)?;
	let discriminator = discriminator.ok_or_else(|| {
		RenderError::MissingDiscriminator {
			context: context.clone(),
		}
	})?;

	let mut lines = Vec::new();

	for doc_line in render_docs(&instruction.docs, 0) {
		lines.push(doc_line);
	}

	lines.push(format!(
		"pub const {}: {} = {};",
		discriminator.name, discriminator.ty, discriminator.value
	));
	lines.push(String::new());
	lines.push("/// Accounts.".to_string());
	lines.push("#[derive(Clone, Debug)]".to_string());
	lines.push(format!("pub struct {instruction_name} {{"));

	for account in &instruction.accounts {
		let (field_type, _) = render_instruction_account_field_type(account);

		for doc_line in render_docs(&account.docs, 1) {
			lines.push(doc_line);
		}

		lines.push(format!(
			"\tpub {}: {},",
			snake(account.name.as_ref()),
			field_type
		));
	}

	lines.push("}".to_string());
	lines.push(String::new());

	let mut new_params = Vec::new();
	let mut new_inits = Vec::new();

	for account in &instruction.accounts {
		let field_name = snake(account.name.as_ref());
		let (field_type, base_type) = render_instruction_account_field_type(account);
		let default_value = render_instruction_account_default_value(
			account,
			instruction,
			&base_type,
			primary_program_const,
			program,
		)?;

		if let Some(default_expr) = default_value {
			new_inits.push(format!("\t\t\t{field_name}: {default_expr},"));
		} else {
			new_params.push(format!("{field_name}: {field_type}"));
			new_inits.push(format!("\t\t\t{field_name},"));
		}
	}

	lines.push(format!("impl {instruction_name} {{"));
	lines.push(format!(
		"\tpub fn new({}) -> Self {{",
		new_params.join(", ")
	));
	lines.push("\t\tSelf {".to_string());
	lines.extend(new_inits);
	lines.push("\t\t}".to_string());
	lines.push("\t}".to_string());
	lines.push(String::new());
	lines.push(format!(
		"\tpub fn instruction(&self, data: {instruction_name}InstructionData) -> \
		 solana_instruction::Instruction {{"
	));
	lines.push("\t\tself.instruction_with_remaining_accounts(data, &[])".to_string());
	lines.push("\t}".to_string());
	lines.push(String::new());
	lines.push("\t#[allow(clippy::arithmetic_side_effects)]".to_string());
	lines.push("\tpub fn instruction_with_remaining_accounts(".to_string());
	lines.push("\t\t&self,".to_string());
	lines.push(format!("\t\tdata: {instruction_name}InstructionData,"));
	lines.push("\t\tremaining_accounts: &[solana_instruction::AccountMeta],".to_string());
	lines.push("\t) -> solana_instruction::Instruction {".to_string());
	lines.push(format!(
		"\t\tlet mut accounts = Vec::with_capacity({} + remaining_accounts.len());",
		instruction.accounts.len()
	));
	lines.extend(render_instruction_account_metas(
		instruction,
		primary_program_const,
	));
	lines.push("\t\taccounts.extend_from_slice(remaining_accounts);".to_string());
	lines.push("\t\tlet data = bytemuck::bytes_of(&data).to_vec();".to_string());
	lines.push(String::new());
	lines.push("\t\tsolana_instruction::Instruction {".to_string());
	lines.push(format!("\t\t\tprogram_id: crate::{primary_program_const},"));
	lines.push("\t\t\taccounts,".to_string());
	lines.push("\t\t\tdata,".to_string());
	lines.push("\t\t}".to_string());
	lines.push("\t}".to_string());
	lines.push("}".to_string());
	lines.push(String::new());

	let mut data_fields = Vec::new();
	data_fields.push(format!("\tpub discriminator: {},", discriminator.ty));

	let mut data_new_args = Vec::new();
	let mut data_inits = Vec::new();
	data_inits.push(format!("\t\t\tdiscriminator: {},", discriminator.name));

	for argument in &instruction.arguments {
		let argument_name = snake(argument.name.as_ref());
		let field_context = format!("{instruction_name}.{argument_name}");
		let argument_type = render_type_for_pod(&argument.r#type, &field_context)?;

		for doc_line in render_docs(&argument.docs, 1) {
			data_fields.push(doc_line);
		}

		data_fields.push(format!("\tpub {argument_name}: {argument_type},"));
		data_new_args.push(format!("{argument_name}: {argument_type}"));
		data_inits.push(format!("\t\t\t{argument_name},"));
	}

	lines.push("#[repr(C)]".to_string());
	lines.push(
		"#[derive(Clone, Copy, Debug, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]"
			.to_string(),
	);
	lines.push(format!("pub struct {instruction_name}InstructionData {{"));
	lines.extend(data_fields);
	lines.push("}".to_string());
	lines.push(String::new());
	lines.push(format!("impl {instruction_name}InstructionData {{"));
	lines.push(format!(
		"\tpub const fn new({}) -> Self {{",
		data_new_args.join(", ")
	));
	lines.push("\t\tSelf {".to_string());
	lines.extend(data_inits);
	lines.push("\t\t}".to_string());
	lines.push("\t}".to_string());
	lines.push("}".to_string());

	Ok(lines.join("\n"))
}

fn render_instruction_account_metas(
	instruction: &InstructionNode,
	primary_program_const: &str,
) -> Vec<String> {
	let mut lines = Vec::new();

	for account in &instruction.accounts {
		let field_name = snake(account.name.as_ref());
		let meta_ctor = if account.is_writable {
			"solana_instruction::AccountMeta::new"
		} else {
			"solana_instruction::AccountMeta::new_readonly"
		};

		let signer_expr = match account.is_signer {
			IsAccountSigner::False => "false".to_string(),
			IsAccountSigner::True => "true".to_string(),
			IsAccountSigner::Either => format!("self.{field_name}.1"),
		};
		let key_expr = match account.is_signer {
			IsAccountSigner::Either => format!("self.{field_name}.0"),
			_ => format!("self.{field_name}"),
		};

		if account.is_optional {
			if account.is_signer == IsAccountSigner::Either {
				lines.push(format!(
					"\t\tif let Some(({field_name}, signer)) = self.{field_name} {{"
				));
				lines.push(format!(
					"\t\t\taccounts.push({meta_ctor}({field_name}, signer));"
				));
				lines.push("\t\t}".to_string());
			} else {
				lines.push(format!(
					"\t\tif let Some({field_name}) = self.{field_name} {{"
				));
				lines.push(format!(
					"\t\t\taccounts.push({meta_ctor}({field_name}, {signer_expr}));"
				));
				lines.push("\t\t}".to_string());
			}

			if matches!(
				instruction.optional_account_strategy,
				InstructionOptionalAccountStrategy::ProgramId
			) {
				lines.push("\t\telse {".to_string());
				lines.push(format!(
					"\t\t\taccounts.\
					 push(solana_instruction::AccountMeta::new_readonly(crate::{primary_program_const}, \
					 false));"
				));
				lines.push("\t\t}".to_string());
			}

			continue;
		}

		lines.push(format!(
			"\t\taccounts.push({meta_ctor}({key_expr}, {signer_expr}));"
		));
	}

	lines
}

fn render_instruction_account_field_type(account: &InstructionAccountNode) -> (String, String) {
	let base_type = match account.is_signer {
		IsAccountSigner::Either => "(solana_pubkey::Pubkey, bool)".to_string(),
		IsAccountSigner::False | IsAccountSigner::True => "solana_pubkey::Pubkey".to_string(),
	};
	if account.is_optional {
		(format!("Option<{base_type}>"), base_type)
	} else {
		(base_type.clone(), base_type)
	}
}

fn render_pda_default_value(
	pda_value: &PdaValueNode,
	instruction: &InstructionNode,
	primary_program_const: &str,
	program: &ProgramNode,
	account: &InstructionAccountNode,
) -> Result<String> {
	let pda = match &pda_value.pda {
		PdaValue::Linked(link) => {
			program
				.pdas
				.iter()
				.find(|pda| pda.name == link.name)
				.ok_or_else(|| {
					RenderError::UnsupportedValue {
						context: format!(
							"instruction `{}` account `{}` default PDA",
							pascal(instruction.name.as_ref()),
							snake(account.name.as_ref())
						),
						kind: pda_value.kind(),
						reason: format!("linked PDA `{}` was not found", link.name.as_ref()),
					}
				})?
		}
		PdaValue::Nested(pda) => pda,
	};

	let seed_expressions = render_pda_default_seed_expressions(
		pda,
		pda_value,
		instruction,
		primary_program_const,
		account,
	)?;

	Ok(format!(
		"solana_pubkey::Pubkey::find_program_address(\n\t\t\t\t&[{}],\n\t\t\t\t&\
		 crate::{primary_program_const},\n\t\t\t).0",
		seed_expressions.join(", ")
	))
}

fn render_pda_default_seed_expressions(
	pda: &PdaNode,
	pda_value: &PdaValueNode,
	instruction: &InstructionNode,
	primary_program_const: &str,
	account: &InstructionAccountNode,
) -> Result<Vec<String>> {
	let mut seed_expressions = Vec::new();

	for seed in &pda.seeds {
		match seed {
			PdaSeedNode::Constant(constant) => {
				seed_expressions.push(render_constant_seed_expression(
					&constant.r#type,
					&constant.value,
					&pda_default_context(instruction, account),
					primary_program_const,
				)?);
			}
			PdaSeedNode::Variable(variable) => {
				let Some(value) = pda_value
					.seeds
					.iter()
					.find(|value| value.name == variable.name)
				else {
					return Err(RenderError::UnsupportedValue {
						context: pda_default_context(instruction, account),
						kind: pda_value.kind(),
						reason: format!("missing value for PDA seed `{}`", variable.name.as_ref()),
					});
				};

				seed_expressions.push(render_pda_default_seed_value(
					value,
					instruction,
					account,
					&pda_default_context(instruction, account),
				)?);
			}
		}
	}

	Ok(seed_expressions)
}

fn render_pda_default_seed_value(
	value: &codama_nodes::PdaSeedValueNode,
	instruction: &InstructionNode,
	default_account: &InstructionAccountNode,
	context: &str,
) -> Result<String> {
	match &value.value {
		PdaSeedValueValueNode::Account(account) => {
			let name = account.name.as_ref();
			let referenced_account = instruction
				.accounts
				.iter()
				.find(|instruction_account| instruction_account.name.as_ref() == name)
				.ok_or_else(|| {
					RenderError::UnsupportedValue {
						context: context.to_string(),
						kind: value.value.kind(),
						reason: format!("account PDA seed `{name}` was not found"),
					}
				})?;

			if referenced_account.name == default_account.name
				|| referenced_account.is_optional
				|| referenced_account.default_value.is_some()
			{
				return Err(RenderError::UnsupportedValue {
					context: context.to_string(),
					kind: value.value.kind(),
					reason: format!(
						"account PDA seed `{name}` is not an explicit builder parameter"
					),
				});
			}

			let seed_name = snake(name);
			if matches!(referenced_account.is_signer, IsAccountSigner::Either) {
				Ok(format!("{seed_name}.0.as_ref()"))
			} else {
				Ok(format!("{seed_name}.as_ref()"))
			}
		}
		PdaSeedValueValueNode::Argument(_) => {
			Err(RenderError::UnsupportedValue {
				context: context.to_string(),
				kind: value.value.kind(),
				reason: "instruction argument PDA seed defaults are not supported by account \
				         builders"
					.to_string(),
			})
		}
		other => {
			Err(RenderError::UnsupportedValue {
				context: context.to_string(),
				kind: other.kind(),
				reason: "only account and argument PDA seed values are supported".to_string(),
			})
		}
	}
}

fn pda_default_context(instruction: &InstructionNode, account: &InstructionAccountNode) -> String {
	format!(
		"instruction `{}` account `{}` default PDA",
		pascal(instruction.name.as_ref()),
		snake(account.name.as_ref())
	)
}

fn render_instruction_account_default_value(
	account: &InstructionAccountNode,
	instruction: &InstructionNode,
	_base_type: &str,
	primary_program_const: &str,
	program: &ProgramNode,
) -> Result<Option<String>> {
	if account.is_optional {
		return Ok(Some("None".to_string()));
	}

	let Some(default_value) = &account.default_value else {
		return Ok(None);
	};

	let value = match default_value {
		InstructionInputValueNode::PublicKey(public_key) => {
			format!("solana_pubkey::pubkey!(\"{}\")", public_key.public_key)
		}
		InstructionInputValueNode::ProgramId(_) => {
			format!("crate::{primary_program_const}")
		}
		InstructionInputValueNode::ProgramLink(program_link) => {
			format!(
				"crate::{}",
				program_id_const_name(program_link.name.as_ref())
			)
		}
		InstructionInputValueNode::Pda(pda) => {
			render_pda_default_value(pda, instruction, primary_program_const, program, account)?
		}
		_ => {
			return Err(RenderError::UnsupportedValue {
				context: format!(
					"instruction `{}` account `{}` default value",
					pascal(program.name.as_ref()),
					snake(account.name.as_ref())
				),
				kind: default_value.kind(),
				reason: "only public key/program defaults are supported".to_string(),
			});
		}
	};

	if matches!(account.is_signer, IsAccountSigner::Either) {
		return Ok(Some(format!("({value}, false)")));
	}

	Ok(Some(value))
}
