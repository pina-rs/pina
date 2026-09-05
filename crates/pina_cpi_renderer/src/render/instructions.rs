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
	let accounts_name = format!("{struct_name}Accounts");
	let context = format!("instruction `{struct_name}`");
	let discriminator = render_constant_discriminator(
		&snake_name,
		&instruction.discriminators,
		&instruction.arguments,
		&context,
	)?;

	let arguments = render_arguments(&instruction.arguments, &context)?;
	let accounts = render_accounts(&instruction.accounts, &context)?;
	let wire_size = discriminator.bytes.len()
		+ arguments
			.iter()
			.map(|argument| argument.wire_size)
			.sum::<usize>();
	let has_accounts = !accounts.is_empty();
	let accounts_type = if has_accounts {
		format!("{accounts_name}<'a>")
	} else {
		accounts_name.clone()
	};
	let struct_generics = if has_accounts { "<'a>" } else { "" };
	let impl_generics = if has_accounts {
		format!("impl<'a> {struct_name}<'a>")
	} else {
		format!("impl {struct_name}")
	};

	let mut lines = Vec::new();
	if has_accounts {
		lines.push("use pina::AccountView;".to_string());
	}
	if arguments
		.iter()
		.any(|argument| argument.rust_type == "Address")
	{
		lines.push("use pina::Address;".to_string());
	}
	lines.extend([
		"use pina::CpiContext;".to_string(),
		"use pina::CpiHandle;".to_string(),
	]);
	if has_accounts {
		lines.push("use pina::ProgramError;".to_string());
	}
	lines.extend([
		"use pina::ProgramResult;".to_string(),
		"use pina::Signer;".to_string(),
		"use pina::ToCpiAccounts;".to_string(),
		String::new(),
		"use crate::ProgramAccount;".to_string(),
	]);
	lines.push(String::new());

	lines.extend(render_docs(&instruction.docs, 0));
	lines.push(format!(
		"/// Accounts required by the `{snake_name}` instruction."
	));
	let accounts_derives = if accounts.is_empty() {
		"#[derive(Clone, Copy, Debug, Default)]"
	} else {
		"#[derive(Clone, Copy, Debug)]"
	};
	lines.push(accounts_derives.to_string());
	lines.push(format!("pub struct {accounts_name}{struct_generics} {{"));
	for account in &accounts {
		lines.extend(account.docs.iter().cloned());
		lines.push(format!("\tpub {}: CpiHandle<'a>,", account.field));
	}
	lines.push("}".to_string());
	lines.push(String::new());
	lines.extend(render_accounts_impl(&accounts_name, &accounts));
	lines.push(String::new());
	lines.extend(render_to_cpi_accounts_impl(&accounts_name, &accounts));
	lines.push(String::new());

	lines.push(format!(
		"/// CPI builder for the `{snake_name}` instruction."
	));
	lines.push("#[derive(Clone, Copy, Debug)]".to_string());
	lines.push(
		"#[must_use = \"the CPI has no effect until invoke or invoke_signed is called\"]"
			.to_string(),
	);
	lines.push(format!("pub struct {struct_name}{struct_generics} {{"));
	lines.push("\t/// Validated accounts required by the instruction.".to_string());
	lines.push(format!("\tpub accounts: {accounts_type},"));
	if !arguments.is_empty() {
		lines.push(String::new());
	}
	for argument in &arguments {
		lines.extend(argument.docs.iter().cloned());
		lines.push(format!("\tpub {}: {},", argument.field, argument.rust_type));
	}
	lines.push("}".to_string());
	lines.push(String::new());
	lines.push(format!("{impl_generics} {{"));
	lines.extend(render_builder_new(&struct_name, &accounts_type, &arguments));
	lines.push(String::new());
	lines.push("\t/// Invokes the instruction with no PDA seeds.".to_string());
	lines.push("\t#[inline(always)]".to_string());
	lines.push(
		"\tpub fn invoke(&self, program: &ProgramAccount<'_>) -> ProgramResult {".to_string(),
	);
	lines.push("\t\tself.invoke_signed(program, &[])".to_string());
	lines.push("\t}".to_string());
	lines.push(String::new());
	lines.push("\t/// Invokes the instruction, signing with the provided PDA seeds.".to_string());
	lines.push("\t#[inline(always)]".to_string());
	lines.push("\tpub fn invoke_signed(".to_string());
	lines.push("\t\t&self,".to_string());
	lines.push("\t\tprogram: &ProgramAccount<'_>,".to_string());
	lines.push("\t\tsigners: &[Signer<'_, '_>],".to_string());
	lines.push("\t) -> ProgramResult {".to_string());
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
	lines.push("\t\tlet context = CpiContext::new(*program, self.accounts);".to_string());
	lines.push(String::new());
	lines.push("\t\tcontext.invoke_signed(&data, signers)".to_string());
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

fn render_accounts_impl(accounts_name: &str, accounts: &[RenderedAccount]) -> Vec<String> {
	if accounts.is_empty() {
		return vec![
			format!("impl {accounts_name} {{"),
			"\t/// Builds the empty CPI account set.".to_string(),
			"\t#[inline(always)]".to_string(),
			"\tpub const fn new() -> Self {".to_string(),
			"\t\tSelf {}".to_string(),
			"\t}".to_string(),
			"}".to_string(),
		];
	}

	let mut lines = vec![format!("impl<'a> {accounts_name}<'a> {{")];
	lines.push("\t/// Validates writable privileges and builds the CPI account set.".to_string());
	lines.push("\tpub fn new(".to_string());
	for account in accounts {
		lines.push(format!("\t\t{}: &'a AccountView,", account.field));
	}
	lines.push("\t) -> Result<Self, ProgramError> {".to_string());
	lines.push("\t\tOk(Self {".to_string());
	for account in accounts {
		let constructor = match (account.is_writable, account.is_signer) {
			(true, true) => "writable_signer",
			(true, false) => "writable",
			(false, true) => "readonly_signer",
			(false, false) => "readonly",
		};
		let suffix = if account.is_writable { "?" } else { "" };
		lines.push(format!(
			"\t\t\t{}: CpiHandle::{constructor}({}){suffix},",
			account.field, account.field
		));
	}
	lines.push("\t\t})".to_string());
	lines.push("\t}".to_string());
	lines.push("}".to_string());

	lines
}

fn render_to_cpi_accounts_impl(accounts_name: &str, accounts: &[RenderedAccount]) -> Vec<String> {
	let count = accounts.len();
	let target = if accounts.is_empty() {
		accounts_name.to_string()
	} else {
		format!("{accounts_name}<'a>")
	};
	let handles = accounts
		.iter()
		.map(|account| format!("self.{}", account.field))
		.collect::<Vec<_>>()
		.join(", ");

	vec![
		format!("impl<'a> ToCpiAccounts<'a, {count}> for {target} {{"),
		format!("\tfn to_cpi_handles(&self) -> [CpiHandle<'a>; {count}] {{"),
		format!("\t\t[{handles}]"),
		"\t}".to_string(),
		"}".to_string(),
	]
}

fn render_builder_new(
	struct_name: &str,
	accounts_type: &str,
	arguments: &[RenderedArgument],
) -> Vec<String> {
	let mut parameters = vec![format!("accounts: {accounts_type}")];
	parameters.extend(
		arguments
			.iter()
			.map(|argument| format!("{}: {}", argument.field, argument.rust_type)),
	);
	let mut fields = vec!["accounts".to_string()];
	fields.extend(arguments.iter().map(|argument| argument.field.clone()));

	vec![
		format!("\t/// Builds a `{struct_name}` CPI instruction."),
		"\t#[inline(always)]".to_string(),
		format!("\tpub const fn new({}) -> Self {{", parameters.join(", ")),
		format!("\t\tSelf {{ {} }}", fields.join(", ")),
		"\t}".to_string(),
	]
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
