//! Read-only account-state parsers for a foreign program's accounts.
//!
//! A CPI client usually needs to read the target program's account state as
//! well as invoke it. Without generated parsers every consumer hand-writes
//! offset arithmetic against a layout they have to re-verify by hand — the
//! `pina-rs/lootbox` Switchboard integration shipped a 408-byte parser for
//! exactly this reason.
//!
//! Each account becomes a `parse_<name>` function over raw bytes. When the
//! layout is fixed-width the parser returns a borrowed zero-copy view, so
//! reading an account costs no copy; a layout with a length-prefixed or
//! otherwise variable tail is read into an owned value instead. Both carry a
//! `LEN`/`MAX_LEN` constant and the discriminator check the program itself
//! performs.

use codama_nodes::AccountNode;
use codama_nodes::DiscriminatorNode;
use codama_nodes::HasKind;
use codama_nodes::NestedTypeNodeTrait;
use heck::ToSnakeCase;

use super::helpers::pascal;
use super::helpers::render_docs;
use super::helpers::snake;
use super::wire::Encoded;
use super::wire::TypeIndex;
use super::wire::plan;
use super::wire::unsupported;
use crate::error::Result;

/// Renders the `accounts` module index for the rendered accounts.
pub(crate) fn render_accounts_mod(names: &[String]) -> String {
	let mut lines = Vec::new();

	for name in names {
		lines.push(format!("pub(crate) mod r#{};", snake(name)));
	}

	lines.push(String::new());

	for name in names {
		lines.push(format!("pub use self::r#{}::*;", snake(name)));
	}

	lines.join("\n")
}

/// Imports shared by every generated account page.
///
/// `Address` is only imported when a field actually uses it, because an
/// account without an address field would otherwise fail a `-D warnings`
/// build with an unused import.
fn account_prelude(uses_address: bool) -> Vec<String> {
	let mut lines = vec![
		"#![allow(rustdoc::broken_intra_doc_links)]".to_string(),
		String::new(),
	];
	if uses_address {
		lines.push("use pina::Address;".to_string());
		lines.push(String::new());
	}
	lines.extend([
		"#[allow(unused_imports)]".to_string(),
		"use crate::generated_types::*;".to_string(),
		String::new(),
	]);

	lines
}

/// The Rust field name for an account field, falling back to a positional one
/// when the IDL omits the name.
pub(crate) fn field_name(field: &codama_nodes::StructFieldTypeNode, index: usize) -> String {
	let declared = field.name.as_ref();
	if declared.is_empty() {
		format!("field_{index}")
	} else {
		snake(declared)
	}
}

/// Resolves an account's discriminator, if one can gate a read-only parser.
///
/// Migration-aware IDLs spread a multi-byte discriminator across several
/// adjacent one-byte constants (a version tag included), so parts are sorted
/// and combined while they continue each other from offset zero, exactly as
/// the instruction path does. A gap or a non-zero start returns `None` rather
/// than failing: the layout still renders, and the parser's guard degrades to
/// a non-empty check instead of the program's bytes.
fn account_discriminator(
	account: &AccountNode,
	context: &str,
) -> Result<Option<RenderedDiscriminator>> {
	// A size discriminator states the layout's length rather than bytes, so it
	// contributes nothing to the guard. The remaining parts each carry the
	// offset their bytes belong at.
	let mut parts: Vec<(u64, Part<'_>)> = account
		.discriminators
		.iter()
		.filter_map(|discriminator| {
			match discriminator {
				DiscriminatorNode::Field(field) => Some((field.offset, Part::Field(field))),
				DiscriminatorNode::Constant(node) => Some((node.offset, Part::Constant(node))),
				DiscriminatorNode::Size(_) => None,
			}
		})
		.collect();
	if parts.is_empty() {
		return Ok(None);
	}

	// Migration-aware IDLs tag an account with several adjacent constants (a
	// program byte plus a version byte), which together form the prefix. Parts
	// that do not continue each other from offset zero leave no byte range that
	// identifies the account, so the guard is withheld rather than guessed.
	parts.sort_by_key(|(offset, _)| *offset);

	let mut combined = Vec::new();
	for (offset, part) in parts {
		if offset != combined.len() as u64 {
			return Ok(None);
		}
		match part {
			Part::Field(field) => {
				let Some(data) = account
					.data
					.get_nested_type_node()
					.fields
					.iter()
					.find(|candidate| candidate.name.as_ref() == field.name.as_ref())
				else {
					return Err(unsupported(
						context,
						"fieldDiscriminatorNode",
						"the discriminated field is not present in this account's data layout",
					));
				};
				combined.extend_from_slice(&field_discriminator_bytes(data, context)?);
			}
			Part::Constant(node) => {
				combined.extend_from_slice(&constant_discriminator_bytes(node, context)?);
			}
		}
	}

	Ok(Some(RenderedDiscriminator {
		name: format!(
			"{}_DISCRIMINATOR",
			snake(account.name.as_ref()).to_uppercase()
		),
		bytes: combined,
	}))
}

/// One byte-carrying part of an account's discriminator.
enum Part<'a> {
	Field(&'a codama_nodes::FieldDiscriminatorNode),
	Constant(&'a codama_nodes::ConstantDiscriminatorNode),
}

pub(crate) struct RenderedDiscriminator {
	name: String,
	bytes: Vec<u8>,
}

/// The little-endian bytes of an integer discriminator literal, narrowed to
/// the width its declared type describes.
///
/// Migration-aware IDLs tag an account with several adjacent one-byte
/// constants, so the width has to come from the declared type rather than from
/// the widest integer that happens to hold the value. Signed literals keep
/// their two's-complement bit pattern, so a `-1` discriminator is still eight
/// `0xFF` bytes.
fn integer_literal_bytes(
	number: codama_nodes::Number,
	width: &codama_nodes::NumberTypeNode,
	kind: &'static str,
	context: &str,
) -> Result<Vec<u8>> {
	use codama_nodes::Number;
	use codama_nodes::NumberFormat;

	if !matches!(width.endian, codama_nodes::Endianness::Le) {
		return Err(unsupported(
			context,
			kind,
			"only little-endian discriminators are supported",
		));
	}

	let (bits, signed) = match number {
		Number::UnsignedInteger(value) => (value, false),
		Number::SignedInteger(value) => (value as u64, true),
		Number::Float(_) => {
			return Err(unsupported(
				context,
				kind,
				"a float discriminator has no integer byte representation",
			));
		}
	};

	let (byte_width, unsigned_max, signed_min) = match width.format {
		NumberFormat::U8 => (1usize, u128::from(u8::MAX), -128i128),
		NumberFormat::U16 => (2, u128::from(u16::MAX), -32_768),
		NumberFormat::U32 => (4, u128::from(u32::MAX), -2_147_483_648),
		NumberFormat::U64 => (8, u128::from(u64::MAX), i128::from(i64::MIN)),
		_ => {
			return Err(unsupported(
				context,
				kind,
				"discriminators must be an integer of at most 8 bytes",
			));
		}
	};

	let fits = if signed {
		i128::from(bits as i64) >= signed_min
	} else {
		u128::from(bits) <= unsigned_max
	};
	if !fits {
		return Err(unsupported(
			context,
			kind,
			&format!(
				"the value `{bits}` does not fit the declared {byte_width}-byte discriminator"
			),
		));
	}

	Ok(bits.to_le_bytes()[..byte_width].to_vec())
}

fn field_discriminator_bytes(
	field: &codama_nodes::StructFieldTypeNode,
	context: &str,
) -> Result<Vec<u8>> {
	use codama_nodes::DefaultValueStrategy;
	use codama_nodes::ValueNode;

	if !matches!(
		field.default_value_strategy,
		Some(DefaultValueStrategy::Omitted)
	) {
		return Err(unsupported(
			context,
			"fieldDiscriminatorNode",
			"the discriminator field has no omitted default value to read its bytes from",
		));
	}

	match (field.default_value.as_ref().as_ref(), field.r#type.as_ref()) {
		(Some(ValueNode::Bytes(value)), _) => {
			crate::render::helpers::decode_base16(&value.data, context)
		}
		(Some(ValueNode::Number(value)), codama_nodes::TypeNode::Number(width)) => {
			integer_literal_bytes(value.number, width, "fieldDiscriminatorNode", context)
		}
		(Some(ValueNode::Number(_)), _) => {
			Err(unsupported(
				context,
				"fieldDiscriminatorNode",
				"a number discriminator field must declare a number type",
			))
		}
		_ => {
			Err(unsupported(
				context,
				"fieldDiscriminatorNode",
				"the discriminator field's default value is not a byte or number literal",
			))
		}
	}
}

fn constant_discriminator_bytes(
	node: &codama_nodes::ConstantDiscriminatorNode,
	context: &str,
) -> Result<Vec<u8>> {
	use codama_nodes::ValueNode;

	match (node.constant.value.as_ref(), node.constant.r#type.as_ref()) {
		(ValueNode::Bytes(value), _) => crate::render::helpers::decode_base16(&value.data, context),
		(ValueNode::Number(value), codama_nodes::TypeNode::Number(width)) => {
			integer_literal_bytes(value.number, width, "constantDiscriminatorNode", context)
		}
		(ValueNode::Number(_), _) => {
			Err(unsupported(
				context,
				"constantDiscriminatorNode",
				"a number discriminator must declare a number type",
			))
		}
		(other, _) => {
			Err(unsupported(
				context,
				other.kind(),
				"account discriminators must be byte or number literals",
			))
		}
	}
}

/// Exposed for tests: plans an account's fields, if the layout supports it.
pub(crate) fn plan_account_fields(
	account: &AccountNode,
	types: &mut TypeIndex,
	context: &str,
) -> (Vec<(String, Encoded)>, Option<String>) {
	let mut planned = Vec::new();
	for (index, field) in account
		.data
		.get_nested_type_node()
		.fields
		.iter()
		.enumerate()
	{
		if matches!(
			field.default_value_strategy,
			Some(codama_nodes::DefaultValueStrategy::Omitted)
		) {
			continue;
		}
		let name = field_name(field, index);
		match plan(&field.r#type, types, &format!("{context} field `{name}`")) {
			Ok(encoded) => planned.push((name, encoded)),
			// The account's layout is still worth emitting, so the first
			// unsupported field is reported instead of failing the render.
			Err(error) => return (planned, Some(error.to_string())),
		}
	}

	(planned, None)
}

/// The `types` a rendered account references.
pub(crate) fn account_module_name(account: &AccountNode) -> String {
	account.name.as_ref().to_snake_case()
}

/// A planned account, ready to render.
pub(crate) struct PlannedAccount {
	pub(crate) name: String,
	pub(crate) module: String,
	pub(crate) docs: Vec<String>,
	pub(crate) fields: Vec<(String, Encoded)>,
	/// Why this account has no parser, when a field could not be planned.
	unsupported: Option<String>,
	discriminator: Option<RenderedDiscriminator>,
	/// Total encoded width when fixed, `None` when it varies.
	pub(crate) fixed_size: Option<usize>,
	pub(crate) max_size: usize,
}

/// Plans one account for rendering.
///
/// # Errors
///
/// Returns an error when the layout has no byte representation.
pub(crate) fn plan_account(account: &AccountNode, types: &mut TypeIndex) -> Result<PlannedAccount> {
	let name = pascal(account.name.as_ref());
	let context = format!("account `{name}`");
	let discriminator = account_discriminator(account, &context)?;
	let (fields, unsupported) = plan_account_fields(account, types, &context);

	let discriminator_len = discriminator.as_ref().map_or(0, |d| d.bytes.len());
	let fixed_size = fields
		.iter()
		.try_fold(0usize, |total, (_, encoded)| {
			encoded.fixed_size.map(|size| total.saturating_add(size))
		})
		.map(|payload| payload.saturating_add(discriminator_len));
	// A u32-prefixed collection reports a maximum of `u32::MAX`, so the sum can
	// exceed `usize` on a 32-bit host. Saturate instead of overflowing.
	let payload_max = fields.iter().fold(0usize, |total, (_, encoded)| {
		total.saturating_add(encoded.max_size)
	});

	Ok(PlannedAccount {
		module: account_module_name(account),
		name,
		docs: account.docs.to_vec(),
		fields,
		unsupported,
		discriminator,
		fixed_size,
		max_size: payload_max.saturating_add(discriminator_len),
	})
}

/// Renders a planned account's read-only parser page.
pub(crate) fn render_planned_account(account: &PlannedAccount) -> String {
	let name = &account.name;
	let uses_address = account
		.fields
		.iter()
		.any(|(_, encoded)| encoded.rust_type.contains("Address"));
	let mut lines = account_prelude(uses_address);
	lines.extend(render_docs(&account.docs, 0));
	// An account struct owns its decoded values, so a borrowed field type is
	// read as a slice that outlives the call rather than as a builder borrow.
	let borrows = account.fields.iter().any(|(_, encoded)| encoded.borrows);
	let generics = if borrows { "<'data>" } else { "" };
	let impl_generics = generics;

	lines.push("#[derive(Clone, Copy, Debug, PartialEq, Eq)]".to_string());
	lines.push(format!("pub struct {name}{generics} {{"));
	for (field, encoded) in &account.fields {
		lines.push(format!(
			"\tpub {field}: {},",
			encoded.rust_type.replace("'argument", "'data")
		));
	}
	lines.push("}".to_string());
	lines.push(String::new());

	if let Some(discriminator) = &account.discriminator {
		lines.push("/// Account discriminator declared by the program's IDL.".to_string());
		lines.push(format!(
			"pub const {}: [u8; {}] = {:?};",
			discriminator.name,
			discriminator.bytes.len(),
			discriminator.bytes
		));
		lines.push(String::new());
	}

	// Every field has to be locatable before any of the `impl` is emitted,
	// because a variable-width field leaves the fields after it with no fixed
	// offset. Planning first keeps the emitted block syntactically whole.
	let mut decodes = Vec::new();
	// A field that could not be planned already rules out a parser, and is the
	// more specific explanation, so it takes precedence over a decode failure.
	let mut decode_error = account.unsupported.clone();
	if decode_error.is_none() {
		let last = account.fields.len().saturating_sub(1);
		for (index, (field, encoded)) in account.fields.iter().enumerate() {
			// The final field has nothing after it, so its decode must not
			// advance the cursor: that would be a dead store.
			let decode = if index == last {
				encoded.decode_final(field, &format!("account `{name}` field `{field}`"))
			} else {
				encoded.decode_into(field, &format!("account `{name}` field `{field}`"))
			};
			match decode {
				Ok(read) => decodes.push(read),
				Err(error) => {
					decode_error = Some(error.to_string());
					break;
				}
			}
		}
	}

	lines.push(format!("impl{impl_generics} {name}{impl_generics} {{"));
	if let Some(size) = account.fixed_size {
		lines.push("\t/// Encoded size of this account's data.".to_string());
		lines.push(format!("\tpub const LEN: usize = {size};"));
	} else {
		lines.push("\t/// Largest encoded size of this account's data.".to_string());
		lines.push(format!(
			"\tpub const MAX_LEN: usize = {};",
			account.max_size
		));
	}
	lines.push(String::new());
	lines.push("\t/// Whether `data` carries this account's discriminator.".to_string());
	lines.push("\t#[inline(always)]".to_string());
	lines.push("\tpub fn matches(data: &[u8]) -> bool {".to_string());
	match &account.discriminator {
		Some(discriminator) => {
			let constants = discriminator.bytes.len();
			lines.push(format!("\t\tdata.len() >= {constants}"));
			lines.push(format!(
				"\t\t\t&& data[..{constants}] == {}",
				discriminator.name
			));
		}
		None => lines.push("\t\t!data.is_empty()".to_string()),
	}
	lines.push("\t}".to_string());
	lines.push(String::new());

	// When a field cannot be located, the layout and discriminator still ship —
	// they are what a caller needs to inspect the account — but no parser, since
	// a partial one would read the wrong offsets. The reason is recorded here.
	if let Some(error) = decode_error {
		// A doc comment must attach to an item, so the explanation is carried on
		// a constant rather than left dangling inside the `impl` block.
		lines.push("\t/// Why this account has no generated parser.".to_string());
		lines.push(format!(
			"\tpub const PARSER_UNSUPPORTED: &'static str = {error:?};"
		));
		lines.push("}".to_string());

		return lines.join("\n");
	}

	// A read-only parser copies the fields a caller can act on. It cannot hand
	// back a borrow of the whole account because the buffer only guarantees the
	// fields' byte ranges, not the alignment a reference would require.
	lines.push("\t/// Reads this account's fields from the account's data.".to_string());
	lines.push("\t///".to_string());
	lines.push(
		"\t/// Returns `None` when the data is too short, the discriminator does not match, or a \
		 field is truncated."
			.to_string(),
	);
	lines.push("\t#[inline(always)]".to_string());
	let data_param = if borrows { "&'data [u8]" } else { "&[u8]" };
	lines.push(format!(
		"\tpub fn parse(data: {data_param}) -> Option<{name}{impl_generics}> {{"
	));
	lines.push("\t\tif !Self::matches(data) {".to_string());
	lines.push("\t\t\treturn None;".to_string());
	lines.push("\t\t}".to_string());
	lines.push(String::new());
	lines.push("\t\tlet mut cursor = 0usize;".to_string());
	if let Some(discriminator) = &account.discriminator {
		lines.push(format!("\t\tcursor += {};", discriminator.bytes.len()));
		lines.push(String::new());
	}
	for read in &decodes {
		lines.push(format!("\t\t{read}"));
	}
	lines.push(String::new());
	lines.push(format!("\t\tSome({name} {{"));
	for (field, _) in &account.fields {
		lines.push(format!("\t\t\t{field},"));
	}
	lines.push("\t\t})".to_string());
	lines.push("\t}".to_string());
	lines.push("}".to_string());

	lines.join("\n")
}
