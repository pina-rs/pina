use codama_nodes::AccountNode;
use codama_nodes::NestedTypeNodeTrait;
use codama_nodes::PdaNode;
use codama_nodes::PdaSeedNode;

use super::discriminator::render_constant_discriminator;
use super::helpers::pascal;
use super::helpers::render_docs;
use super::helpers::snake;
use super::seeds::render_constant_seed_expression;
use super::seeds::render_variable_seed_parameter;
use super::types::render_type_for_pod;
use crate::error::Result;

pub(crate) fn render_accounts_mod(accounts: &[AccountNode]) -> String {
	let mut lines = Vec::new();

	for account in accounts {
		lines.push(format!(
			"pub(crate) mod r#{};",
			snake(account.name.as_ref())
		));
	}

	lines.push(String::new());

	for account in accounts {
		lines.push(format!(
			"pub use self::r#{}::*;",
			snake(account.name.as_ref())
		));
	}

	lines.join("\n")
}

pub(crate) fn render_account_page(
	account: &AccountNode,
	primary_program_const: &str,
	pda: Option<&PdaNode>,
) -> Result<String> {
	let account_name = pascal(account.name.as_ref());
	let context = format!("account `{account_name}`");
	let discriminator =
		render_constant_discriminator(account.name.as_ref(), &account.discriminators, &context)?;

	let data_type = account.data.get_nested_type_node();
	let mut field_lines = Vec::new();
	let mut ctor_args = Vec::new();
	let mut ctor_inits = Vec::new();

	for doc_line in render_docs(&account.docs, 0) {
		field_lines.push(doc_line);
	}

	if let Some(discriminator) = &discriminator {
		field_lines.push(format!("\tpub discriminator: {},", discriminator.ty));
	}

	for field in &data_type.fields {
		let field_name = snake(field.name.as_ref());
		let field_context = format!("{account_name}.{field_name}");
		let field_type = render_type_for_pod(&field.r#type, &field_context)?;
		for doc_line in render_docs(&field.docs, 1) {
			field_lines.push(doc_line);
		}
		field_lines.push(format!("\tpub {field_name}: {field_type},"));
		ctor_args.push(format!("{field_name}: {field_type}"));
		ctor_inits.push(format!("\t\t\t{field_name},"));
	}

	let mut lines = Vec::new();
	lines.push("use bytemuck::Pod;".to_string());
	lines.push("use bytemuck::Zeroable;".to_string());
	lines.push(String::new());
	lines.push("#[repr(C)]".to_string());
	lines.push("#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]".to_string());
	lines.push(format!("pub struct {account_name} {{"));
	lines.extend(field_lines);
	lines.push("}".to_string());
	lines.push(String::new());

	if let Some(discriminator) = &discriminator {
		lines.push(format!(
			"pub const {}: {} = {};",
			discriminator.name, discriminator.ty, discriminator.value
		));
		lines.push(String::new());
	}

	lines.push(format!("impl {account_name} {{"));
	lines.push("\tpub const LEN: usize = core::mem::size_of::<Self>();".to_string());
	lines.push(String::new());

	if ctor_args.is_empty() {
		lines.push("\tpub const fn new() -> Self {".to_string());
		lines.push("\t\tSelf {".to_string());

		if let Some(discriminator) = &discriminator {
			lines.push(format!("\t\t\tdiscriminator: {},", discriminator.name));
		}

		lines.push("\t\t}".to_string());
		lines.push("\t}".to_string());
	} else {
		lines.push(format!(
			"\tpub const fn new({}) -> Self {{",
			ctor_args.join(", ")
		));
		lines.push("\t\tSelf {".to_string());

		if let Some(discriminator) = &discriminator {
			lines.push(format!("\t\t\tdiscriminator: {},", discriminator.name));
		}

		lines.extend(ctor_inits);
		lines.push("\t\t}".to_string());
		lines.push("\t}".to_string());
	}

	lines.push(String::new());
	lines.push(
		"\tpub fn from_bytes(data: &[u8]) -> Result<&Self, solana_program_error::ProgramError> {"
			.to_string(),
	);
	lines.push("\t\tlet account = bytemuck::try_from_bytes::<Self>(data)".to_string());
	lines.push(
		"\t\t\t.map_err(|_| solana_program_error::ProgramError::InvalidAccountData)?;".to_string(),
	);
	if let Some(discriminator) = &discriminator {
		lines.push(format!(
			"\t\tif account.discriminator != {} {{",
			discriminator.name
		));
		lines.push(
			"\t\t\treturn Err(solana_program_error::ProgramError::InvalidAccountData);".to_string(),
		);
		lines.push("\t\t}".to_string());
	}
	lines.push("\t\tOk(account)".to_string());
	lines.push("\t}".to_string());
	lines.push(String::new());
	lines.push(
		"\tpub fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self, \
		 solana_program_error::ProgramError> {"
			.to_string(),
	);
	lines.push("\t\tlet account = bytemuck::try_from_bytes_mut::<Self>(data)".to_string());
	lines.push(
		"\t\t\t.map_err(|_| solana_program_error::ProgramError::InvalidAccountData)?;".to_string(),
	);
	if let Some(discriminator) = &discriminator {
		lines.push(format!(
			"\t\tif account.discriminator != {} {{",
			discriminator.name
		));
		lines.push(
			"\t\t\treturn Err(solana_program_error::ProgramError::InvalidAccountData);".to_string(),
		);
		lines.push("\t\t}".to_string());
	}
	lines.push("\t\tOk(account)".to_string());
	lines.push("\t}".to_string());
	lines.push("}".to_string());
	lines.push(String::new());
	lines.push(format!(
		"impl<'a> TryFrom<&solana_account_info::AccountInfo<'a>> for {account_name} {{"
	));
	lines.push("\ttype Error = solana_program_error::ProgramError;".to_string());
	lines.push(String::new());
	lines.push(
		"\tfn try_from(account_info: &solana_account_info::AccountInfo<'a>) -> Result<Self, \
		 Self::Error> {"
			.to_string(),
	);
	lines.push("\t\tlet data_ref = (*account_info.data).borrow();".to_string());
	lines.push("\t\tlet account = Self::from_bytes(&data_ref)?;".to_string());
	lines.push("\t\tOk(*account)".to_string());
	lines.push("\t}".to_string());
	lines.push("}".to_string());

	if let Some(pda) = pda {
		lines.push(String::new());
		lines.extend(render_account_pda_helpers(
			account_name.as_str(),
			pda,
			primary_program_const,
		)?);
	}

	Ok(lines.join("\n"))
}

fn render_account_pda_helpers(
	account_name: &str,
	pda: &PdaNode,
	primary_program_const: &str,
) -> Result<Vec<String>> {
	let mut params = Vec::new();
	let mut seed_exprs = Vec::new();

	for seed in &pda.seeds {
		match seed {
			PdaSeedNode::Variable(variable) => {
				let seed_name = snake(variable.name.as_ref());
				let context = format!("PDA `{}` variable seed `{seed_name}`", pda.name.as_ref());
				let (param_type, seed_expr) =
					render_variable_seed_parameter(&seed_name, &variable.r#type, &context)?;

				params.push(format!("{seed_name}: {param_type}"));
				seed_exprs.push(seed_expr);
			}
			PdaSeedNode::Constant(constant) => {
				let context = format!("PDA `{}` constant seed", pda.name.as_ref());
				seed_exprs.push(render_constant_seed_expression(
					&constant.r#type,
					&constant.value,
					&context,
					primary_program_const,
				)?);
			}
		}
	}

	let mut lines = Vec::new();
	lines.push(format!("impl {account_name} {{"));
	lines.push(format!(
		"\tpub fn find_pda({}) -> (solana_pubkey::Pubkey, u8) {{",
		params.join(", ")
	));
	lines.push("\t\tsolana_pubkey::Pubkey::find_program_address(".to_string());
	lines.push("\t\t\t&[".to_string());

	for seed_expr in &seed_exprs {
		lines.push(format!("\t\t\t\t{seed_expr},"));
	}

	lines.push("\t\t\t],".to_string());
	lines.push(format!("\t\t\t&crate::{primary_program_const},"));
	lines.push("\t\t)".to_string());
	lines.push("\t}".to_string());
	lines.push(String::new());

	let mut create_params = params.clone();
	create_params.push("bump: u8".to_string());
	lines.push(format!(
		"\tpub fn create_pda({}) -> Result<solana_pubkey::Pubkey, solana_pubkey::PubkeyError> {{",
		create_params.join(", ")
	));
	lines.push("\t\tsolana_pubkey::Pubkey::create_program_address(".to_string());
	lines.push("\t\t\t&[".to_string());

	for seed_expr in &seed_exprs {
		lines.push(format!("\t\t\t\t{seed_expr},"));
	}

	lines.push("\t\t\t\t&[bump],".to_string());
	lines.push("\t\t\t],".to_string());
	lines.push(format!("\t\t\t&crate::{primary_program_const},"));
	lines.push("\t\t)".to_string());
	lines.push("\t}".to_string());
	lines.push("}".to_string());

	Ok(lines)
}
