use codama_nodes::BooleanTypeNode;
use codama_nodes::BytesEncoding;
use codama_nodes::CountNode;
use codama_nodes::DefinedTypeNode;
use codama_nodes::Docs;
use codama_nodes::Endianness;
use codama_nodes::EnumTypeNode;
use codama_nodes::EnumVariantTypeNode;
use codama_nodes::HasKind;
use codama_nodes::NestedTypeNodeTrait;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::StructTypeNode;
use codama_nodes::TypeNode;

use super::helpers::pascal;
use super::helpers::render_docs;
use super::helpers::snake;
use crate::error::RenderError;
use crate::error::Result;

pub(crate) fn render_type_for_pod(r#type: &TypeNode, context: &str) -> Result<String> {
	match r#type {
		TypeNode::Number(number_type) => render_number_type_for_pod(number_type, context),
		TypeNode::Boolean(boolean_type) => render_boolean_type(boolean_type, context),
		TypeNode::PublicKey(_) => Ok("solana_pubkey::Pubkey".to_string()),
		TypeNode::Bytes(_) => {
			Err(RenderError::UnsupportedType {
				context: context.to_string(),
				kind: r#type.kind(),
				reason: "bytes must be wrapped in fixedSizeTypeNode".to_string(),
			})
		}
		TypeNode::String(_) => {
			Err(RenderError::UnsupportedType {
				context: context.to_string(),
				kind: r#type.kind(),
				reason: "variable-size strings are not POD".to_string(),
			})
		}
		TypeNode::FixedSize(fixed_size) => {
			if matches!(fixed_size.r#type.as_ref(), TypeNode::Bytes(_)) {
				Ok(format!("[u8; {}]", fixed_size.size))
			} else if matches!(fixed_size.r#type.as_ref(), TypeNode::Link(_)) {
				render_type_for_pod(&fixed_size.r#type, context)
			} else if let Some(collection) =
				render_pod_collection_type(fixed_size.size, &fixed_size.r#type, context)?
			{
				Ok(collection)
			} else {
				Err(RenderError::UnsupportedType {
					context: context.to_string(),
					kind: r#type.kind(),
					reason: "unsupported fixed-size wrapper; its size is a total byte size, not \
					         an element count"
						.to_string(),
				})
			}
		}
		TypeNode::Array(array_type) => {
			let item_type = render_type_for_pod(&array_type.item, context)?;
			match array_type.count.as_ref() {
				CountNode::Fixed(count) => Ok(format!("[{item_type}; {}]", count.value)),
				CountNode::Prefixed(_) | CountNode::Remainder(_) => {
					Err(RenderError::UnsupportedType {
						context: context.to_string(),
						kind: r#type.kind(),
						reason: "only fixed-size arrays are POD".to_string(),
					})
				}
			}
		}
		TypeNode::Link(link) => {
			Ok(format!(
				"crate::generated::types::{}",
				pascal(link.name.as_ref())
			))
		}
		unsupported => {
			Err(RenderError::UnsupportedType {
				context: context.to_string(),
				kind: unsupported.kind(),
				reason: "node kind is not supported by pina_codama_renderer yet".to_string(),
			})
		}
	}
}

fn render_pod_collection_type(
	fixed_size: usize,
	inner: &TypeNode,
	context: &str,
) -> Result<Option<String>> {
	if let TypeNode::SizePrefix(size_prefix) = inner
		&& let TypeNode::String(string) = size_prefix.r#type.as_ref()
		&& matches!(string.encoding, BytesEncoding::Utf8)
	{
		let prefix_size = render_prefix_size(size_prefix.prefix.get_nested_type_node(), context)?;
		let capacity = fixed_size.checked_sub(prefix_size).ok_or_else(|| {
			RenderError::UnsupportedType {
				context: context.to_string(),
				kind: inner.kind(),
				reason: "PodString fixed size is smaller than its length prefix".to_string(),
			}
		})?;

		return Ok(Some(format!("pina::PodString<{capacity}, {prefix_size}>")));
	}

	let TypeNode::Array(array) = inner else {
		return Ok(None);
	};
	let CountNode::Prefixed(count) = array.count.as_ref() else {
		return Ok(None);
	};

	let prefix_size = render_prefix_size(count.prefix.get_nested_type_node(), context)?;
	let payload_size = fixed_size.checked_sub(prefix_size).ok_or_else(|| {
		RenderError::UnsupportedType {
			context: context.to_string(),
			kind: inner.kind(),
			reason: "PodVec fixed size is smaller than its length prefix".to_string(),
		}
	})?;
	let item_size = fixed_type_node_size(&array.item).ok_or_else(|| {
		RenderError::UnsupportedType {
			context: context.to_string(),
			kind: array.item.kind(),
			reason: "cannot determine the fixed byte size of the PodVec element".to_string(),
		}
	})?;

	if item_size == 0 || payload_size % item_size != 0 {
		return Err(RenderError::UnsupportedType {
			context: context.to_string(),
			kind: inner.kind(),
			reason: "PodVec fixed size is not an exact number of elements".to_string(),
		});
	}

	let capacity = payload_size / item_size;
	let item_type = render_type_for_pod(&array.item, context)?;

	Ok(Some(format!(
		"pina::PodVec<{item_type}, {capacity}, {prefix_size}>"
	)))
}

fn render_prefix_size(number_type: &NumberTypeNode, context: &str) -> Result<usize> {
	if !matches!(number_type.endian, Endianness::Le) {
		return Err(RenderError::UnsupportedType {
			context: context.to_string(),
			kind: "numberTypeNode",
			reason: "collection length prefixes must be little-endian".to_string(),
		});
	}

	match number_type.format {
		NumberFormat::U8 => Ok(1),
		NumberFormat::U16 => Ok(2),
		NumberFormat::U32 => Ok(4),
		NumberFormat::U64 => Ok(8),
		_ => {
			Err(RenderError::UnsupportedType {
				context: context.to_string(),
				kind: "numberTypeNode",
				reason: "collection length prefixes must be u8, u16, u32, or u64".to_string(),
			})
		}
	}
}

fn fixed_type_node_size(node: &TypeNode) -> Option<usize> {
	match node {
		TypeNode::Number(number) => {
			match number.format {
				NumberFormat::U8 | NumberFormat::I8 => Some(1),
				NumberFormat::U16 | NumberFormat::I16 => Some(2),
				NumberFormat::U32 | NumberFormat::I32 => Some(4),
				NumberFormat::U64 | NumberFormat::I64 => Some(8),
				NumberFormat::U128 | NumberFormat::I128 => Some(16),
				NumberFormat::F32 | NumberFormat::F64 | NumberFormat::ShortU16 => None,
			}
		}
		TypeNode::Boolean(_) => Some(1),
		TypeNode::PublicKey(_) => Some(32),
		TypeNode::FixedSize(fixed) => Some(fixed.size),
		TypeNode::Array(array) => {
			match array.count.as_ref() {
				CountNode::Fixed(count) => {
					fixed_type_node_size(&array.item)
						.and_then(|size| size.checked_mul(count.value as usize))
				}
				CountNode::Prefixed(_) | CountNode::Remainder(_) => None,
			}
		}
		_ => None,
	}
}

fn render_number_type_for_pod(number_type: &NumberTypeNode, context: &str) -> Result<String> {
	if !matches!(number_type.endian, Endianness::Le) {
		return Err(RenderError::UnsupportedType {
			context: context.to_string(),
			kind: "numberTypeNode",
			reason: "only little-endian number types are supported".to_string(),
		});
	}

	match number_type.format {
		NumberFormat::U8 => Ok("u8".to_string()),
		NumberFormat::I8 => Ok("i8".to_string()),
		NumberFormat::U16 => Ok("pina::PodU16".to_string()),
		NumberFormat::I16 => Ok("pina::PodI16".to_string()),
		NumberFormat::U32 => Ok("pina::PodU32".to_string()),
		NumberFormat::I32 => Ok("pina::PodI32".to_string()),
		NumberFormat::U64 => Ok("pina::PodU64".to_string()),
		NumberFormat::I64 => Ok("pina::PodI64".to_string()),
		NumberFormat::U128 => Ok("pina::PodU128".to_string()),
		NumberFormat::I128 => Ok("pina::PodI128".to_string()),
		NumberFormat::F32 | NumberFormat::F64 | NumberFormat::ShortU16 => {
			Err(RenderError::UnsupportedType {
				context: context.to_string(),
				kind: "numberTypeNode",
				reason: format!("format `{:?}` is not POD-compatible", number_type.format),
			})
		}
	}
}

fn render_boolean_type(boolean_type: &BooleanTypeNode, context: &str) -> Result<String> {
	let number_type = boolean_type.size.get_nested_type_node();
	if !matches!(number_type.format, NumberFormat::U8)
		|| !matches!(number_type.endian, Endianness::Le)
	{
		return Err(RenderError::UnsupportedType {
			context: context.to_string(),
			kind: "booleanTypeNode",
			reason: "booleans must be encoded as little-endian u8".to_string(),
		});
	}
	Ok("pina::PodBool".to_string())
}

pub(crate) fn render_defined_type_page(defined_type: &DefinedTypeNode) -> Result<String> {
	let name = pascal(defined_type.name.as_ref());
	let context = format!("defined type `{name}`");
	match defined_type.r#type.as_ref() {
		TypeNode::Struct(struct_type) => {
			render_defined_struct(name.as_str(), struct_type, &defined_type.docs)
		}
		TypeNode::Enum(enum_type) => {
			render_defined_pod_enum(name.as_str(), enum_type, &defined_type.docs)
		}
		TypeNode::Link(link) => {
			Ok(format!(
				"pub type {name} = crate::generated::types::{};",
				pascal(link.name.as_ref())
			))
		}
		other => {
			let ty = render_type_for_pod(other, &context)?;
			Ok(format!("pub type {name} = {ty};"))
		}
	}
}

fn render_defined_pod_enum(name: &str, enum_type: &EnumTypeNode, docs: &Docs) -> Result<String> {
	let enum_name = name.strip_suffix("Zc").ok_or_else(|| {
		RenderError::UnsupportedType {
			context: format!("defined type `{name}`"),
			kind: "enumTypeNode",
			reason: "Pina POD enum defined types must use the `EnumZc` companion name".to_string(),
		}
	})?;
	let number = enum_type.size.get_nested_type_node();
	if !matches!(number.endian, Endianness::Le) {
		return Err(RenderError::UnsupportedType {
			context: format!("defined type `{name}`"),
			kind: "enumTypeNode",
			reason: "Pina POD enums must use little-endian unsigned discriminants".to_string(),
		});
	}
	let repr = match number.format {
		NumberFormat::U8 => "u8",
		NumberFormat::U16 => "u16",
		NumberFormat::U32 => "u32",
		NumberFormat::U64 => "u64",
		_ => {
			return Err(RenderError::UnsupportedType {
				context: format!("defined type `{name}`"),
				kind: "enumTypeNode",
				reason: "Pina POD enums require u8, u16, u32, or u64 discriminants".to_string(),
			});
		}
	};

	let mut lines = render_docs(docs, 0);
	lines.push("#[derive(Clone, Copy, Debug, PartialEq, Eq, pina::PodEnum)]".to_string());
	lines.push(format!("#[repr({repr})]"));
	lines.push(format!("pub enum {enum_name} {{"));
	for (index, variant) in enum_type.variants.iter().enumerate() {
		let EnumVariantTypeNode::Empty(variant) = variant else {
			return Err(RenderError::UnsupportedType {
				context: format!("defined type `{name}`"),
				kind: "enumTypeNode",
				reason: "Pina POD enums support unit variants only".to_string(),
			});
		};
		let variant_name = pascal(variant.name.as_ref());
		let value = variant.discriminator.unwrap_or(index as u32);
		lines.push(format!("\t{variant_name} = {value},"));
	}
	lines.push("}".to_string());
	Ok(lines.join("\n"))
}

fn render_defined_struct(name: &str, struct_type: &StructTypeNode, docs: &Docs) -> Result<String> {
	let mut lines = Vec::new();
	for doc_line in render_docs(docs, 0) {
		lines.push(doc_line);
	}
	lines.push("#[repr(C)]".to_string());
	lines.push("#[derive(Clone, Copy, Debug, PartialEq, Eq)]".to_string());
	lines.push(format!("pub struct {name} {{"));
	let mut ctor_args = Vec::new();
	let mut ctor_inits = Vec::new();
	for field in &struct_type.fields {
		let field_name = snake(field.name.as_ref());
		let field_context = format!("{name}.{field_name}");
		let field_type = render_type_for_pod(&field.r#type, &field_context)?;
		for doc_line in render_docs(&field.docs, 1) {
			lines.push(doc_line);
		}
		lines.push(format!("\tpub {field_name}: {field_type},"));
		ctor_args.push(format!("{field_name}: {field_type}"));
		ctor_inits.push(format!("\t\t\t{field_name},"));
	}
	lines.push("}".to_string());
	lines.push(String::new());
	lines.push(format!("impl {name} {{"));
	lines.push(format!(
		"\tpub const fn new({}) -> Self {{",
		ctor_args.join(", ")
	));
	lines.push("\t\tSelf {".to_string());
	lines.extend(ctor_inits);
	lines.push("\t\t}".to_string());
	lines.push("\t}".to_string());
	lines.push("}".to_string());
	Ok(lines.join("\n"))
}

pub(crate) fn render_defined_types_mod(defined_types: &[DefinedTypeNode]) -> String {
	let mut lines = Vec::new();
	for defined_type in defined_types {
		lines.push(format!(
			"pub(crate) mod r#{};",
			snake(defined_type.name.as_ref())
		));
	}
	lines.push(String::new());
	for defined_type in defined_types {
		lines.push(format!(
			"pub use self::r#{}::*;",
			snake(defined_type.name.as_ref())
		));
	}
	lines.join("\n")
}
