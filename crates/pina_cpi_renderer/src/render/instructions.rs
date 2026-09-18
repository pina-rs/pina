//! Per-instruction CPI builder pages.

use codama_nodes::InstructionAccountNode;
use codama_nodes::InstructionArgumentNode;
use codama_nodes::InstructionNode;
use codama_nodes::IsSigner;
use codama_nodes::OptionalAccountStrategy;
use heck::ToSnakeCase;

use super::args::RenderedArgument;
use super::discriminator::render_constant_discriminator;
use super::helpers::pascal;
use super::helpers::render_docs;
use super::helpers::rust_identifier;
use super::helpers::snake;
use super::wire::TypeIndex;
use crate::error::RenderError;
use crate::error::Result;

pub(crate) fn render_instructions_mod(rendered: &[String], skipped: &[(String, String)]) -> String {
	let mut lines = Vec::new();

	// A skipped instruction is absent from the client, so the reason is
	// recorded here where a reviewer of the generated crate sees it first.
	for (name, reason) in skipped {
		lines.push(format!(
			"// Skipped `{name}`: {}",
			reason.replace('\n', "\n// ")
		));
	}
	if !skipped.is_empty() {
		lines.push(String::new());
	}

	for name in rendered {
		lines.push(format!("pub(crate) mod r#{name};"));
	}

	lines.push(String::new());

	for name in rendered {
		lines.push(format!("pub use self::r#{name}::*;"));
	}

	lines.join("\n")
}

/// Renders one instruction page.
///
/// Arguments first try the terse fixed-layout renderer. Anything it rejects is
/// re-planned through [`super::wire`], which knows how to encode `definedTypes`
/// references, structs, enums, options, and length-prefixed collections. A
/// wire plan that is genuinely variable-length switches the instruction to a
/// caller-buffer encoder, so the CPI still runs without an allocator.
pub(crate) fn render_instruction_page(
	instruction: &InstructionNode,
	types: &mut TypeIndex,
) -> Result<String> {
	let snake_name = snake(instruction.name.as_ref());
	let struct_name = pascal(instruction.name.as_ref());
	let ix_name = format!("{struct_name}Ix");
	let context = format!("instruction `{struct_name}`");
	let discriminator = render_constant_discriminator(
		&snake_name,
		&instruction.discriminators,
		&instruction.arguments,
		&context,
	)?;

	let arguments = render_arguments(&instruction.arguments, types, &context)?;
	let accounts = render_accounts(instruction, &context)?;
	let variable = arguments.iter().any(|argument| argument.variable);
	// `definedTypes` arguments write through a cursor; instructions whose
	// arguments are all fixed-layout keep their literal per-argument offsets,
	// which keeps previously generated clients byte-identical.
	let planned = arguments.iter().any(|argument| argument.planned);
	let disc_len = discriminator.bytes.len();
	// Saturating so an unbounded length prefix (whose declared maximum is
	// `usize::MAX`) reports a ceiling instead of overflowing. A variable
	// instruction sizes its buffer from this ceiling.
	let wire_size = arguments.iter().fold(disc_len, |total, argument| {
		total.saturating_add(argument.wire_size)
	});
	let has_accounts = !accounts.is_empty();
	let has_addresses = arguments
		.iter()
		.any(|argument| argument.rust_type.contains("Address"));
	let has_borrowed_arguments = arguments.iter().any(|argument| argument.borrows);
	let ix_generics = lifetime_generics(false, has_borrowed_arguments);
	let struct_generics = lifetime_generics(has_accounts, has_borrowed_arguments);
	let ix_type = format!("{ix_name}{ix_generics}");

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
		"use pina::ProgramError;".to_string(),
		"use pina::Signer;".to_string(),
		String::new(),
		"use crate::ProgramAccount;".to_string(),
		String::new(),
	]);
	if planned {
		// Generated argument types live in a sibling module, which only
		// carries content once an argument references the IDL's `definedTypes`.
		lines.push("#[allow(unused_imports)]".to_string());
		lines.push("use crate::generated_types::*;".to_string());
		lines.push(String::new());
	}

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
	lines.push(format!("\tpub ix: {ix_type},"));
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
		lines.push(format!("pub struct {ix_name};"));
	} else {
		lines.push(format!("pub struct {ix_name}{ix_generics} {{"));
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

	let ix_impl = impl_header(&ix_name, false, has_borrowed_arguments);
	lines.push(format!("{ix_impl} {{"));
	lines.push(
		"\t/// Number of bytes in the encoded instruction, including its discriminator."
			.to_string(),
	);
	if variable {
		lines.push(
			"\t/// Largest encoded instruction length, including its discriminator.".to_string(),
		);
		lines.push(format!("\tpub const MAX_DATA_LEN: usize = {wire_size};"));
		lines.push(String::new());
		lines.push(
			"\t/// Encodes the discriminator and arguments into `buffer`, returning the bytes \
			 written."
				.to_string(),
		);
		lines.push("\t///".to_string());
		lines.push(
			"\t/// The caller owns the buffer because these arguments carry caller-supplied \
			 lengths."
				.to_string(),
		);
		lines.push("\t#[inline(always)]".to_string());
		lines.push(
			"\tpub fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, ProgramError> {"
				.to_string(),
		);
		lines.push("\t\tlet data = buffer;".to_string());
		lines.push("\t\tlet mut offset = 0usize;".to_string());
		lines.push(format!(
			"\t\tif offset + {} > data.len() {{\n\t\t\treturn \
			 Err(ProgramError::InvalidInstructionData);\n\t\t}}",
			discriminator.bytes.len()
		));
		lines.push(format!(
			"\t\tdata[offset..offset + {}].copy_from_slice(&{});",
			discriminator.bytes.len(),
			discriminator.name
		));
		lines.push(format!("\t\toffset += {};", discriminator.bytes.len()));
		for argument in &arguments {
			lines.push(render_argument_write(argument));
		}
		lines.push(String::new());
		lines.push("\t\tOk(offset)".to_string());
		lines.push("\t}".to_string());
	} else {
		lines.push(format!("\tpub const LEN: usize = {wire_size};"));
		lines.push(String::new());
		lines
			.push("\t/// Encodes the discriminator and instruction arguments for CPI.".to_string());
		lines.push("\t#[inline(always)]".to_string());
		lines.push(format!(
			"\tpub fn to_bytes(&self) -> Result<[u8; {wire_size}], ProgramError> {{"
		));
		lines.push(format!("\t\tlet mut data = [0u8; {wire_size}];"));
		lines.push(format!(
			"\t\tdata[..{}].copy_from_slice(&{});",
			discriminator.bytes.len(),
			discriminator.name
		));
		if planned {
			// A planned argument writes through a cursor, so every argument
			// shares it and advances it by its own width.
			lines.push(format!(
				"\t\t#[allow(unused_mut, unused_variables)]\n\t\tlet mut offset = {}usize;",
				discriminator.bytes.len()
			));
			for argument in &arguments {
				lines.push(render_argument_write(argument));
			}
		} else {
			let mut offset = discriminator.bytes.len();
			for argument in &arguments {
				lines.push(render_argument_write_at(argument, offset));
				offset += argument.wire_size;
			}
		}
		lines.push(String::new());
		lines.push("\t\tOk(data)".to_string());
		lines.push("\t}".to_string());
	}
	lines.push("}".to_string());
	lines.push(String::new());

	let builder_impl = impl_header(&struct_name, has_accounts, has_borrowed_arguments);
	lines.push(format!("{builder_impl} {{"));
	lines.push("\t/// Invokes the instruction with no PDA seeds.".to_string());
	if variable {
		lines.push("\t///".to_string());
		lines.push(
			"\t/// `buffer` receives the encoded instruction data and must be at least \
			 [`Self::MAX_DATA_LEN`] bytes."
				.to_string(),
		);
	}
	lines.push("\t#[inline(always)]".to_string());
	if variable {
		lines.push(
			"\tpub fn invoke(\n\t\t&self,\n\t\tprogram: &ProgramAccount<'_>,\n\t\tbuffer: &mut \
			 [u8],\n\t) -> ProgramResult {"
				.to_string(),
		);
		lines.push("\t\tself.invoke_signed(program, &[], buffer)".to_string());
	} else {
		lines.push(
			"\tpub fn invoke(&self, program: &ProgramAccount<'_>) -> ProgramResult {".to_string(),
		);
		lines.push("\t\tself.invoke_signed(program, &[])".to_string());
	}
	lines.push("\t}".to_string());
	lines.push(String::new());
	lines.push("\t/// Invokes the instruction, signing with the provided PDA seeds.".to_string());
	lines.push("\t#[inline(always)]".to_string());
	lines.push("\tpub fn invoke_signed(".to_string());
	lines.push("\t\t&self,".to_string());
	lines.push("\t\tprogram: &ProgramAccount<'_>,".to_string());
	lines.push("\t\tsigners: &[Signer<'_, '_>],".to_string());
	if variable {
		lines.push("\t\tbuffer: &mut [u8],".to_string());
	}
	lines.push("\t) -> ProgramResult {".to_string());
	lines.extend(render_account_handles(&accounts));
	if variable {
		lines.push("\t\tlet len = self.ix.encode_into(buffer)?;".to_string());
		lines.push("\t\tlet context = CpiContext::new(*program, accounts);".to_string());
		lines.push(String::new());
		lines.push("\t\tcontext.invoke_signed(&buffer[..len], signers)".to_string());
	} else {
		lines.push("\t\tlet data = self.ix.to_bytes()?;".to_string());
		lines.push("\t\tlet context = CpiContext::new(*program, accounts);".to_string());
		lines.push(String::new());
		lines.push("\t\tcontext.invoke_signed(&data, signers)".to_string());
	}
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

fn lifetime_generics(has_accounts: bool, has_arguments: bool) -> String {
	match (has_accounts, has_arguments) {
		(false, false) => String::new(),
		(true, false) => "<'account>".to_string(),
		(false, true) => "<'argument>".to_string(),
		(true, true) => "<'account, 'argument>".to_string(),
	}
}

fn impl_header(name: &str, has_accounts: bool, has_arguments: bool) -> String {
	let generics = lifetime_generics(has_accounts, has_arguments);
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

/// Renders the CPI account list.
///
/// Under Anchor's `omitted` strategy an absent optional account is dropped from
/// the account list entirely rather than replaced by a placeholder. The
/// generated handle set is a fixed-size `[CpiHandle; N]` array — the crate is
/// `no_std`, so there is no allocation to shorten a list with — and every
/// account slot has to be filled. Dropping an account is therefore impossible
/// without silently shifting the accounts after it, so the strategy is
/// rejected with the instruction named; regenerate against an IDL using the
/// `programId` strategy, whose absent accounts the renderer can represent.
fn render_accounts(instruction: &InstructionNode, context: &str) -> Result<Vec<RenderedAccount>> {
	if instruction.optional_account_strategy.unwrap_or_default() == OptionalAccountStrategy::Omitted
	{
		return Err(RenderError::UnsupportedAccount {
			context: context.to_string(),
			account: "optional accounts".to_string(),
			reason: "the omitted optional-account strategy drops an absent account from the \
			         account list entirely, and a fixed-size handle set cannot express a \
			         shortened list without shifting the accounts after the hole; this renderer \
			         only supports the `programId` strategy, which replaces an absent account \
			         with the program ID"
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
	types: &mut TypeIndex,
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
		.map(|argument| render_argument_with_docs(argument, types, context))
		.collect()
}

fn render_argument_with_docs(
	argument: &InstructionArgumentNode,
	types: &mut TypeIndex,
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

	let mut rendered =
		RenderedArgument::render(argument.name.as_ref(), &argument.r#type, types, context)?;
	rendered.docs = vec![format!(
		"\t/// Instruction argument `{}`.",
		argument.name.as_ref()
	)];
	rendered.docs.extend(render_docs(&argument.docs, 1));

	Ok(rendered)
}

/// Renders one argument's write when every argument shares a cursor.
///
/// The cursor starts just past the discriminator. A planned argument advances
/// it itself; a terse one writes at the cursor and advances it by its width.
fn render_argument_write(argument: &RenderedArgument) -> String {
	// A terse write is identified by its `{offset}` placeholders.
	if !argument.write.contains("{offset}") {
		return indent(&argument.write, 2);
	}

	let write = argument
		.write
		.replace("{offset}", "offset")
		.replace("{offset_end}", &format!("offset + {}", argument.wire_size));

	// The buffer is caller-owned and may be shorter than the instruction needs,
	// so every write is bounds-checked instead of allowed to panic.
	format!(
		"\t\tif offset + {} > data.len() {{\n\t\t\treturn \
		 Err(ProgramError::InvalidInstructionData);\n\t\t}}\n\t\t{write}\n\t\toffset += {};",
		argument.wire_size, argument.wire_size
	)
}

/// Renders one argument's write at its literal position.
///
/// Used when no argument needs the general planner, so the generated
/// instruction data keeps the exact offsets this renderer has always emitted.
fn render_argument_write_at(argument: &RenderedArgument, offset: usize) -> String {
	let write = argument
		.write
		.replace("{offset}", &offset.to_string())
		.replace("{offset_end}", &(offset + argument.wire_size).to_string());

	format!("\t\t{write}")
}

fn indent(body: &str, levels: usize) -> String {
	let tab = "\t".repeat(levels);

	body.lines()
		.map(|line| format!("{tab}{line}"))
		.collect::<Vec<_>>()
		.join("\n")
}
