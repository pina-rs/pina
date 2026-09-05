//! Per-instruction CPI builder pages.

use codama_nodes::InstructionAccountNode;
use codama_nodes::InstructionArgumentNode;
use codama_nodes::InstructionNode;
use codama_nodes::IsSigner;
use heck::ToSnakeCase;

use super::args::RenderedArgument;
use super::args::render_argument;
use super::discriminator::render_constant_discriminator;
use super::helpers::pascal;
use super::helpers::render_docs;
use super::helpers::snake;
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

pub(crate) fn render_instruction_page(instruction: &InstructionNode) -> Result<String> {
	let snake_name = snake(instruction.name.as_ref());
	let struct_name = pascal(instruction.name.as_ref());
	let context = format!("instruction `{struct_name}`");
	let discriminator =
		render_constant_discriminator(&snake_name, &instruction.discriminators, &context)?;

	let arguments = render_arguments(&instruction.arguments, &context)?;
	let accounts = render_accounts(&instruction.accounts, &context)?;
	let wire_size = discriminator.bytes.len()
		+ arguments
			.iter()
			.map(|argument| argument.wire_size)
			.sum::<usize>();
	let impl_generics = format!("impl {struct_name}<'_>");

	let mut lines = vec![
		"use pinocchio::cpi::Signer;".to_string(),
		"use pinocchio::cpi::invoke_signed;".to_string(),
		"use pinocchio::error::ProgramResult;".to_string(),
		"use pinocchio::instruction::InstructionView;".to_string(),
		"use pinocchio::Address;".to_string(),
	];
	lines.push("use pinocchio::instruction::InstructionAccount;".to_string());
	lines.push("use pinocchio::AccountView;".to_string());
	lines.push(String::new());

	lines.extend(render_docs(&instruction.docs, 0));
	lines.push(format!(
		"/// CPI arguments for the `{snake_name}` instruction."
	));
	lines.push("#[derive(Clone, Copy, Debug)]".to_string());
	lines.push("#[must_use = \"the CPI has no effect until invoke_signed is called\"]".to_string());
	lines.push(format!("pub struct {struct_name}<'a> {{"));
	lines.push("\t/// Switchboard program the instruction is invoked on.".to_string());
	lines.push("\tpub program_id: &'a Address,".to_string());
	for account in &accounts {
		lines.push(String::new());
		lines.extend(account.docs.iter().cloned());
		lines.push(format!("\tpub {}: &'a AccountView,", account.field));
	}
	if !accounts.is_empty() {
		lines.push(String::new());
	}
	for argument in &arguments {
		lines.extend(argument.docs.iter().cloned());
		lines.push(format!("\tpub {}: {},", argument.field, argument.rust_type));
	}
	lines.push("}".to_string());
	lines.push(String::new());
	lines.push(format!("{impl_generics} {{"));
	lines.push("\t/// Invokes the instruction with no PDA seeds.".to_string());
	lines.push("\t///".to_string());
	lines.push(
		"\t/// Signer accounts must still be covered by the enclosing transaction or".to_string(),
	);
	lines.push("\t/// by seeds passed to [`Self::invoke_signed`].".to_string());
	lines.push("\t#[inline]".to_string());
	lines.push("\tpub fn invoke(&self) -> ProgramResult {".to_string());
	lines.push("\t\tself.invoke_signed(&[])".to_string());
	lines.push("\t}".to_string());
	lines.push(String::new());
	lines.push("\t/// Invokes the instruction, signing with the provided PDA seeds.".to_string());
	lines.push("\tpub fn invoke_signed(&self, signers: &[Signer]) -> ProgramResult {".to_string());
	lines.push(format!("\t\tlet mut data = [0u8; {wire_size}];"));
	lines.push(format!(
		"\t\tdata[..{}].copy_from_slice(&{});",
		discriminator.bytes.len(),
		discriminator.name
	));
	let mut offset = discriminator.bytes.len();
	for argument in &arguments {
		lines.push(render_argument_write(argument, offset));
		offset += argument.wire_size;
	}
	lines.push("\t\tlet accounts = [".to_string());
	for account in &accounts {
		lines.push(format!(
			"\t\t\tInstructionAccount::new(self.{}.address(), {}, {}),",
			account.field, account.is_writable, account.is_signer
		));
	}
	lines.push("\t\t];".to_string());
	lines.push(format!(
		"\t\tlet views: [&AccountView; {}] = [",
		accounts.len()
	));
	for account in &accounts {
		lines.push(format!("\t\t\tself.{},", account.field));
	}
	lines.push("\t\t];".to_string());
	lines.push("\t\tlet instruction = InstructionView {".to_string());
	lines.push("\t\t\tprogram_id: self.program_id,".to_string());
	lines.push("\t\t\taccounts: &accounts,".to_string());
	lines.push("\t\t\tdata: &data,".to_string());
	lines.push("\t\t};".to_string());
	lines.push("\t\tinvoke_signed(&instruction, &views, signers)".to_string());
	lines.push("\t}".to_string());
	lines.push("}".to_string());
	lines.push(String::new());
	lines.push(format!(
		"const {}: [u8; {}] = {:?};",
		discriminator.name,
		discriminator.bytes.len(),
		discriminator.bytes
	));

	Ok(lines.join("\n"))
}

struct RenderedAccount {
	field: String,
	is_writable: bool,
	is_signer: bool,
	docs: Vec<String>,
}

fn render_accounts(
	accounts: &[InstructionAccountNode],
	context: &str,
) -> Result<Vec<RenderedAccount>> {
	accounts
		.iter()
		.map(|account| render_account(account, context))
		.collect()
}

fn render_account(account: &InstructionAccountNode, context: &str) -> Result<RenderedAccount> {
	let name = account.name.as_ref().to_string();

	if account.is_optional == Some(true) {
		return Err(RenderError::UnsupportedAccount {
			context: context.to_string(),
			account: name.clone(),
			reason: "optional accounts are not supported yet".to_string(),
		});
	}

	// Accounts derived from PDA seeds or defaulted to known programs stay
	// ordinary builder fields: at CPI time the caller must pass the derived
	// account explicitly anyway, because the runtime resolves CPI accounts
	// against the executing program's own account list.

	let is_signer = match account.is_signer {
		IsSigner::True => true,
		IsSigner::False => false,
		IsSigner::Either => {
			return Err(RenderError::UnsupportedAccount {
				context: context.to_string(),
				account: name.clone(),
				reason: "optional signers are not supported yet".to_string(),
			});
		}
	};

	let mut docs = render_docs(&account.docs, 1);
	let mut role = String::new();
	if account.is_writable {
		role.push_str("writable");
	}
	if is_signer {
		if !role.is_empty() {
			role.push_str(", ");
		}
		role.push_str("must sign");
	}
	if !role.is_empty() {
		docs.push(format!("\t/// {role}."));
	}

	Ok(RenderedAccount {
		field: account.name.as_ref().to_snake_case(),
		is_writable: account.is_writable,
		is_signer,
		docs,
	})
}

fn render_arguments(
	arguments: &[InstructionArgumentNode],
	context: &str,
) -> Result<Vec<RenderedArgument>> {
	arguments
		.iter()
		.filter(|argument| {
			!matches!(
				argument.default_value_strategy,
				Some(codama_nodes::DefaultValueStrategy::Omitted)
			)
		})
		.map(|argument| render_argument_with_docs(argument, context))
		.collect()
}

fn render_argument_with_docs(
	argument: &InstructionArgumentNode,
	context: &str,
) -> Result<RenderedArgument> {
	if matches!(
		argument.default_value_strategy,
		Some(codama_nodes::DefaultValueStrategy::Optional)
	) {
		return Err(RenderError::UnsupportedType {
			context: context.to_string(),
			kind: "instructionArgumentNode",
			reason: format!(
				"argument `{}` has an optional default; optional arguments are not supported yet",
				argument.name.as_ref()
			),
		});
	}

	let mut rendered = render_argument(argument.name.as_ref(), &argument.r#type, context)?;
	if rendered.docs.is_empty() && rendered.field != argument.name.as_ref() {
		rendered
			.docs
			.push(format!("\t/// `{}` argument.", argument.name.as_ref()));
	}

	Ok(rendered)
}

fn render_argument_write(argument: &RenderedArgument, offset: usize) -> String {
	let write = argument
		.write
		.replace("{offset}", &offset.to_string())
		.replace("{offset_end}", &(offset + argument.wire_size).to_string());

	format!("\t\t{write}")
}
