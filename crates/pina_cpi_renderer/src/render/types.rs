//! Generated Rust types for `definedTypes` arguments.
//!
//! Real-world Anchor IDLs route most non-primitive arguments through
//! `definedTypes`, so a CPI builder cannot describe its arguments without
//! declaring those shapes. Each referenced struct or enum becomes a Rust type
//! with an `encode_into` that writes its own ABI bytes, matching the
//! `offset`-advancing model in [`super::wire`].
//!
//! A type borrows only when one of its fields does, so the common fixed-layout
//! case stays `Copy`.

use codama_nodes::DefinedTypeNode;
use codama_nodes::EnumTypeNode;
use codama_nodes::EnumVariantTypeNode;
use codama_nodes::HasKind;
use codama_nodes::NestedTypeNodeTrait;
use codama_nodes::NumberFormat;
use codama_nodes::TypeNode;

use super::helpers::pascal;
use super::helpers::render_docs;
use super::helpers::snake;
use super::wire::Encoded;
use super::wire::TypeIndex;
use super::wire::plan_variant_payload;
use super::wire::unsupported;
use crate::error::Result;

/// Renders the `types` module index for the registered types.
pub(crate) fn render_types_mod(names: &[String]) -> String {
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

/// Renders one generated type page.
///
/// # Errors
///
/// Returns an error when the type has no instruction-data layout.
pub(crate) fn render_type_page(
	defined_type: &DefinedTypeNode,
	types: &mut TypeIndex,
) -> Result<String> {
	let name = pascal(defined_type.name.as_ref());
	let context = format!("defined type `{name}`");

	match defined_type.r#type.as_ref() {
		TypeNode::Struct(structure) => {
			let fields = structure.fields.clone();
			let mut planned: Vec<(String, Vec<String>, Encoded)> = Vec::new();
			for field in &fields {
				planned.push((
					snake(field.name.as_ref()),
					field.docs.to_vec(),
					super::wire::plan(
						&field.r#type,
						types,
						&format!("{context} field `{}`", field.name.as_ref()),
					)?,
				));
			}

			Ok(render_struct(&name, &defined_type.docs, &planned))
		}
		TypeNode::Enum(enumeration) => {
			let variants = enumeration
				.variants
				.iter()
				.enumerate()
				.map(|(index, variant)| {
					let payload = plan_variant_payload(variant, types, &context)?;

					Ok(RenderedVariant {
						name: super::wire::enum_variant_name(&super::wire::variant_name(variant)),
						docs: variant_docs(variant),
						discriminator: variant_discriminator(variant, index),
						payload,
					})
				})
				.collect::<Result<Vec<_>>>()?;

			render_enum(&name, enumeration, &defined_type.docs, &variants)
		}
		other => {
			Err(unsupported(
				&context,
				other.kind(),
				"only struct and enum defined types become Rust types; every other shape resolves \
				 to its underlying type at the argument site",
			))
		}
	}
}

/// One enum variant with its planned payload.
struct RenderedVariant {
	name: String,
	docs: Vec<String>,
	discriminator: usize,
	payload: Vec<Encoded>,
}

fn render_struct(name: &str, docs: &[String], fields: &[(String, Vec<String>, Encoded)]) -> String {
	let borrows = fields.iter().any(|(_, _, encoded)| encoded.borrows);
	let generics = if borrows { "<'a>" } else { "" };

	let mut lines = type_prelude();
	lines.extend(render_docs(docs, 0));
	lines.push("#[derive(Clone, Copy, Debug, PartialEq, Eq)]".to_string());
	lines.push(format!("pub struct {name}{generics} {{"));
	for (field_name, field_docs, encoded) in fields {
		lines.extend(render_docs(field_docs, 1));
		lines.push(format!(
			"\tpub {field_name}: {},",
			lifetime_type(&encoded.rust_type)
		));
	}
	lines.push("}".to_string());
	lines.push(String::new());

	let fixed = fields.iter().try_fold(0usize, |total, (_, _, encoded)| {
		encoded.fixed_size.map(|size| total.saturating_add(size))
	});
	let maximum = fields.iter().fold(0usize, |total, (_, _, encoded)| {
		total.saturating_add(encoded.max_size)
	});

	lines.push(format!("impl{generics} {name}{generics} {{"));
	lines.extend(render_size_constants(fixed, maximum));
	lines.push(String::new());
	lines.extend(render_encoder(fields.iter().map(
		|(field_name, _, encoded)| encoded.clone().with_value(&format!("(&self.{field_name})")),
	)));
	lines.push("}".to_string());

	lines.join("\n")
}

fn render_enum(
	name: &str,
	enumeration: &EnumTypeNode,
	docs: &[String],
	variants: &[RenderedVariant],
) -> Result<String> {
	let size = enumeration.size.get_nested_type_node();
	let (tag_type, tag_width) = match size.format {
		NumberFormat::U8 => ("u8", 1),
		NumberFormat::U16 => ("u16", 2),
		NumberFormat::U32 => ("u32", 4),
		NumberFormat::U64 => ("u64", 8),
		_ => {
			return Err(unsupported(
				&format!("defined type `{name}`"),
				"enumTypeNode",
				"enum discriminants must be a little-endian u8, u16, u32, or u64",
			));
		}
	};

	let payload_widths = variants
		.iter()
		.map(|variant| {
			variant
				.payload
				.iter()
				.try_fold(0usize, |total, item| {
					item.fixed_size.map(|size| total.saturating_add(size))
				})
				.unwrap_or(0)
		})
		.collect::<Vec<_>>();
	// Borsh serializes the tag followed by the variant payload, so variants
	// with different widths simply make the enum variable-length.
	let uniform = payload_widths.windows(2).all(|pair| pair[0] == pair[1]);
	let payload_width = payload_widths.first().copied().unwrap_or(0);
	let maximum = variants
		.iter()
		.map(|variant| {
			variant
				.payload
				.iter()
				.fold(tag_width, |total: usize, item| {
					total.saturating_add(item.max_size)
				})
		})
		.max()
		.unwrap_or(tag_width);
	let borrows = variants
		.iter()
		.any(|variant| variant.payload.iter().any(|item| item.borrows));
	let generics = if borrows { "<'a>" } else { "" };

	let mut lines = type_prelude();
	lines.extend(render_docs(docs, 0));
	lines.push("#[derive(Clone, Copy, Debug, PartialEq, Eq)]".to_string());
	lines.push(format!("pub enum {name}{generics} {{"));
	for variant in variants {
		lines.extend(render_docs(&variant.docs, 1));
		match variant.payload.len() {
			0 => lines.push(format!("\t{},", variant.name)),
			1 => {
				lines.push(format!(
					"\t{}({}),",
					variant.name,
					lifetime_type(&variant.payload[0].rust_type)
				));
			}
			_ => {
				lines.push(format!("\t{} {{", variant.name));
				for (index, item) in variant.payload.iter().enumerate() {
					lines.push(format!(
						"\t\tfield_{index}: {},",
						lifetime_type(&item.rust_type)
					));
				}
				lines.push("\t},".to_string());
			}
		}
	}
	lines.push("}".to_string());
	lines.push(String::new());

	lines.push(format!("impl{generics} {name}{generics} {{"));
	lines.extend(render_size_constants(
		uniform.then_some(tag_width + payload_width),
		maximum,
	));
	lines.push(String::new());

	let arms = variants
		.iter()
		.map(|variant| {
			let fields = (0..variant.payload.len())
				.map(|index| format!("field_{index}"))
				.collect::<Vec<_>>();
			let pattern = match variant.payload.len() {
				0 => format!("Self::{}", variant.name),
				1 => format!("Self::{}(value)", variant.name),
				_ => format!("Self::{} {{ {} }}", variant.name, fields.join(", ")),
			};
			let mut lines = vec![format!(
				"let tag: {tag_type} = {};\ndata[offset..offset + \
				 {tag_width}].copy_from_slice(&tag.to_le_bytes());\noffset += {tag_width};",
				variant.discriminator
			)];
			for (index, item) in variant.payload.iter().enumerate() {
				let value = if variant.payload.len() == 1 {
					"value".to_string()
				} else {
					format!("field_{index}")
				};
				lines.push(item.clone().with_value(&value).encode);
			}

			// The arm body is indented one level past the `match` block that
			// encloses it.
			format!("{pattern} => {{\n{}\n\t\t}}", indent(&lines.join("\n"), 3))
		})
		.collect::<Vec<_>>();

	lines.push("\t/// Writes this value into `data` and advances `offset`.".to_string());
	lines.push(
		"\tpub fn encode_into(&self, data: &mut [u8], __offset: &mut usize) -> Result<(), \
		 ProgramError> {"
			.to_string(),
	);
	lines.push("\t\tlet mut offset = *__offset;".to_string());
	lines.push(format!(
		"\t\tmatch self {{\n\t\t\t{}\n\t\t}}",
		arms.join("\n\t\t\t")
	));
	lines.push(String::new());
	lines.push("\t\t*__offset = offset;".to_string());
	lines.push("\t\tOk(())".to_string());
	lines.push("\t}".to_string());
	lines.push("}".to_string());

	Ok(lines.join("\n"))
}

fn render_size_constants(fixed: Option<usize>, maximum: usize) -> Vec<String> {
	vec![
		"\t/// Encoded length of this type.".to_string(),
		match fixed {
			Some(size) => format!("\tpub const LEN: usize = {size};"),
			None => format!("\tpub const MAX_LEN: usize = {maximum};"),
		},
	]
}

fn render_encoder(fields: impl Iterator<Item = Encoded>) -> Vec<String> {
	let mut lines = vec![
		"\t/// Writes this value into `data` and advances `offset`.".to_string(),
		"\tpub fn encode_into(&self, data: &mut [u8], __offset: &mut usize) -> Result<(), \
		 ProgramError> {"
			.to_string(),
	];
	lines.push("\t\tlet mut offset = *__offset;".to_string());
	for encoded in fields {
		lines.push(indent(&encoded.encode, 2));
	}
	lines.push(String::new());
	lines.push("\t\t*__offset = offset;".to_string());
	lines.push("\t\tOk(())".to_string());
	lines.push("\t}".to_string());

	lines
}

/// Enum variants carry display metadata, not doc comments.
fn variant_docs(_variant: &EnumVariantTypeNode) -> Vec<String> {
	Vec::new()
}

fn variant_discriminator(variant: &EnumVariantTypeNode, index: usize) -> usize {
	let explicit = match variant {
		EnumVariantTypeNode::Empty(variant) => variant.discriminator,
		EnumVariantTypeNode::Struct(variant) => variant.discriminator,
		EnumVariantTypeNode::Tuple(variant) => variant.discriminator,
	};

	// Codama records an explicit discriminant only when the IDL sets one, so
	// an absent value means the variant's position in the enum.
	explicit.map_or(index, |value| value as usize)
}

/// Imports and lint allowances shared by every generated type page.
///
/// A generated type encodes through `ProgramError` and may reference sibling
/// types from the same module, so both are always in scope; `Address` and the
/// collection helpers are only needed by some types.
fn type_prelude() -> Vec<String> {
	vec![
		"#![allow(rustdoc::broken_intra_doc_links)]".to_string(),
		String::new(),
		"use pina::Address;".to_string(),
		"use pina::ProgramError;".to_string(),
		String::new(),
		"#[allow(unused_imports)]".to_string(),
		"use super::*;".to_string(),
		String::new(),
	]
}

/// Rewrites an argument type into the lifetime the generated struct declares.
fn lifetime_type(rust_type: &str) -> String {
	rust_type.replace("'argument", "'a")
}

fn indent(body: &str, levels: usize) -> String {
	let tab = "\t".repeat(levels);

	body.lines()
		.map(|line| format!("{tab}{line}"))
		.collect::<Vec<_>>()
		.join("\n")
}
