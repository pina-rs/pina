//! Per-instruction CPI builder pages.

use codama_nodes::InstructionAccountNode;
use codama_nodes::InstructionArgumentNode;
use codama_nodes::InstructionNode;
use codama_nodes::IsSigner;
use codama_nodes::OptionalAccountStrategy;
use heck::ToSnakeCase;

use super::args::RenderedArgument;
use super::args::render_argument;
use super::discriminator::render_constant_discriminator;
use super::helpers::pascal;
use super::helpers::render_docs;
use super::helpers::rust_identifier;
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
	let instruction_name = format!("{struct_name}Instruction");
	let context = format!("instruction `{struct_name}`");
	let discriminator = render_constant_discriminator(
		&snake_name,
		&instruction.discriminators,
		&instruction.arguments,
		&context,
	)?;

	let arguments = render_arguments(&instruction.arguments, &context)?;
	let accounts = render_accounts(instruction, &context)?;
	let wire_size = discriminator.bytes.len()
		+ arguments
			.iter()
			.map(|argument| argument.wire_size)
			.sum::<usize>();
	let has_accounts = !accounts.is_empty();
	let has_addresses = arguments
		.iter()
		.any(|argument| argument.rust_type.contains("Address"));
	let instruction_generics = lifetime_generics(false, has_addresses);
	let struct_generics = lifetime_generics(has_accounts, has_addresses);
	let instruction_type = format!("{instruction_name}{instruction_generics}");

	let mut lines = vec![
		"#![allow(rustdoc::broken_intra_doc_links)]".to_string(),
		String::new(),
	];
	if has_accounts {
		lines.push("use pina::AccountView;".to_string());
	}
	if has_addresses {
		lines.push("use pina::Address;".to_string());
	}
	lines.extend([
		"use pina::CpiContext;".to_string(),
		"use pina::CpiHandle;".to_string(),
		"use pina::ProgramResult;".to_string(),
		"use pina::Signer;".to_string(),
		String::new(),
		"use crate::ProgramAccount;".to_string(),
		String::new(),
	]);

	lines.extend(render_docs(&instruction.docs, 0));
	lines.push(format!("/// CPI call for the `{snake_name}` instruction."));
	lines.push("#[derive(Clone, Copy, Debug)]".to_string());
	lines.push(
		"#[must_use = \"the CPI has no effect until invoke or invoke_signed is called\"]"
			.to_string(),
	);
	lines.push(format!("pub struct {struct_name}{struct_generics} {{"));
	for (index, account) in accounts.iter().enumerate() {
		if index > 0 {
			lines.push(String::new());
		}
		lines.extend(account.docs.iter().cloned());
		lines.push(format!("\tpub {}: {},", account.field, account.rust_type()));
	}
	if has_accounts {
		lines.push(String::new());
	}
	lines.push(format!(
		"\t/// Instruction arguments encoded and sent as CPI data for `{snake_name}`."
	));
	lines.push(format!("\tpub instruction: {instruction_type},"));
	lines.push("}".to_string());
	lines.push(String::new());

	lines.push(format!(
		"/// Instruction arguments for the `{snake_name}` CPI call."
	));
	let instruction_derives = if arguments.is_empty() {
		"#[derive(Clone, Copy, Debug, Default)]"
	} else {
		"#[derive(Clone, Copy, Debug)]"
	};
	lines.push(instruction_derives.to_string());
	if arguments.is_empty() {
		lines.push(format!("pub struct {instruction_name};"));
	} else {
		lines.push(format!(
			"pub struct {instruction_name}{instruction_generics} {{"
		));
		for (index, argument) in arguments.iter().enumerate() {
			if index > 0 {
				lines.push(String::new());
			}
			lines.extend(argument.docs.iter().cloned());
			lines.push(format!("\tpub {}: {},", argument.field, argument.rust_type));
		}
		lines.push("}".to_string());
	}
	lines.push(String::new());

	let instruction_impl = impl_header(&instruction_name, false, has_addresses);
	lines.push(format!("{instruction_impl} {{"));
	lines.push(
		"\t/// Number of bytes in the encoded instruction, including its discriminator."
			.to_string(),
	);
	lines.push(format!("\tpub const LEN: usize = {wire_size};"));
	lines.push(String::new());
	lines.push("\t/// Encodes the discriminator and instruction arguments for CPI.".to_string());
	lines.push("\t#[inline(always)]".to_string());
	lines.push(format!("\tpub fn to_bytes(&self) -> [u8; {wire_size}] {{"));
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
	lines.push(String::new());
	lines.push("\t\tdata".to_string());
	lines.push("\t}".to_string());
	lines.push("}".to_string());
	lines.push(String::new());

	let builder_impl = impl_header(&struct_name, has_accounts, has_addresses);
	lines.push(format!("{builder_impl} {{"));
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
	lines.extend(render_account_handles(&accounts));
	lines.push("\t\tlet data = self.instruction.to_bytes();".to_string());
	lines.push("\t\tlet context = CpiContext::new(*program, accounts);".to_string());
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

fn lifetime_generics(has_accounts: bool, has_addresses: bool) -> String {
	match (has_accounts, has_addresses) {
		(false, false) => String::new(),
		(true, false) => "<'account>".to_string(),
		(false, true) => "<'address>".to_string(),
		(true, true) => "<'account, 'address>".to_string(),
	}
}

fn impl_header(name: &str, has_accounts: bool, has_addresses: bool) -> String {
	let generics = lifetime_generics(has_accounts, has_addresses);
	if generics.is_empty() {
		format!("impl {name}")
	} else {
		format!("impl{generics} {name}{generics}")
	}
}

fn render_account_handles(accounts: &[RenderedAccount]) -> Vec<String> {
	let mut lines = vec![format!(
		"\t\tlet accounts: [CpiHandle<'_>; {}] = [",
		accounts.len()
	)];
	for account in accounts {
		lines.extend(account.handle_lines());
	}
	lines.push("\t\t];".to_string());

	lines
}

struct RenderedAccount {
	field: String,
	is_writable: bool,
	is_optional: bool,
	is_signer: IsSigner,
	docs: Vec<String>,
}

impl RenderedAccount {
	fn rust_type(&self) -> String {
		let base = if self.is_signer == IsSigner::Either {
			"(&'account AccountView, bool)"
		} else {
			"&'account AccountView"
		};

		if self.is_optional {
			format!("Option<{base}>")
		} else {
			base.to_string()
		}
	}

	fn handle_lines(&self) -> Vec<String> {
		let field = &self.field;
		match (self.is_optional, self.is_signer) {
			(true, IsSigner::Either) => {
				vec![
					format!("\t\t\tmatch self.{field} {{"),
					format!(
						"\t\t\t\tSome((account, true)) => {},",
						self.constructor("account", true)
					),
					format!(
						"\t\t\t\tSome((account, false)) => {},",
						self.constructor("account", false)
					),
					"\t\t\t\tNone => CpiHandle::readonly(program.account()),".to_string(),
					"\t\t\t},".to_string(),
				]
			}
			(true, signer) => {
				vec![
					format!("\t\t\tmatch self.{field} {{"),
					format!(
						"\t\t\t\tSome(account) => {},",
						self.constructor("account", signer == IsSigner::True)
					),
					"\t\t\t\tNone => CpiHandle::readonly(program.account()),".to_string(),
					"\t\t\t},".to_string(),
				]
			}
			(false, IsSigner::Either) => {
				vec![
					format!("\t\t\tmatch self.{field} {{"),
					format!(
						"\t\t\t\t(account, true) => {},",
						self.constructor("account", true)
					),
					format!(
						"\t\t\t\t(account, false) => {},",
						self.constructor("account", false)
					),
					"\t\t\t},".to_string(),
				]
			}
			(false, signer) => {
				vec![format!(
					"\t\t\t{},",
					self.constructor(&format!("self.{field}"), signer == IsSigner::True)
				)]
			}
		}
	}

	fn constructor(&self, account: &str, is_signer: bool) -> String {
		let constructor = match (self.is_writable, is_signer) {
			(true, true) => "writable_signer",
			(true, false) => "writable",
			(false, true) => "readonly_signer",
			(false, false) => "readonly",
		};
		let suffix = if self.is_writable { "?" } else { "" };

		format!("CpiHandle::{constructor}({account}){suffix}")
	}
}

fn render_accounts(instruction: &InstructionNode, context: &str) -> Result<Vec<RenderedAccount>> {
	if instruction
		.accounts
		.iter()
		.any(|account| account.is_optional == Some(true))
		&& instruction.optional_account_strategy.unwrap_or_default()
			== OptionalAccountStrategy::Omitted
	{
		return Err(RenderError::UnsupportedAccount {
			context: context.to_string(),
			account: "optional accounts".to_string(),
			reason: "the omitted optional-account strategy cannot use a fixed-size CPI account \
			         array"
				.to_string(),
		});
	}

	instruction
		.accounts
		.iter()
		.map(|account| render_account(account, context))
		.collect()
}

fn render_account(account: &InstructionAccountNode, context: &str) -> Result<RenderedAccount> {
	let name = account.name.as_ref().to_string();
	let mut docs = vec![format!("\t/// CPI account `{name}`.")];
	docs.extend(render_docs(&account.docs, 1));
	let privilege = match (account.is_writable, account.is_signer) {
		(true, IsSigner::True) => "Required privileges: writable and signer.",
		(true, IsSigner::False) => "Required privileges: writable.",
		(true, IsSigner::Either) => {
			"Required privileges: writable; the tuple flag selects signer status."
		}
		(false, IsSigner::True) => "Required privileges: read-only and signer.",
		(false, IsSigner::False) => "Required privileges: read-only.",
		(false, IsSigner::Either) => {
			"Required privileges: read-only; the tuple flag selects signer status."
		}
	};
	docs.push(format!("\t/// {privilege}"));
	if account.is_optional == Some(true) {
		docs.push(
			"\t/// Pass `None` to use the target program ID as the account placeholder."
				.to_string(),
		);
	}

	Ok(RenderedAccount {
		field: rust_identifier(&account.name.as_ref().to_snake_case(), context)?,
		is_writable: account.is_writable,
		is_optional: account.is_optional == Some(true),
		is_signer: account.is_signer,
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
	rendered.docs = vec![format!(
		"\t/// Instruction argument `{}`.",
		argument.name.as_ref()
	)];
	rendered.docs.extend(render_docs(&argument.docs, 1));

	Ok(rendered)
}

fn render_argument_write(argument: &RenderedArgument, offset: usize) -> String {
	let write = argument
		.write
		.replace("{offset}", &offset.to_string())
		.replace("{offset_end}", &(offset + argument.wire_size).to_string());

	format!("\t\t{write}")
}
