use codama_nodes::BooleanTypeNode;
use codama_nodes::CountNode;
use codama_nodes::DefinedTypeNode;
use codama_nodes::Docs;
use codama_nodes::Endian;
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
			} else {
				let inner = render_type_for_pod(&fixed_size.r#type, context)?;
				Ok(format!("[{inner}; {}]", fixed_size.size))
			}
		}
		TypeNode::Array(array_type) => {
			let item_type = render_type_for_pod(&array_type.item, context)?;
			match &array_type.count {
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

fn render_number_type_for_pod(number_type: &NumberTypeNode, context: &str) -> Result<String> {
	if !matches!(number_type.endian, Endian::Little) {
		return Err(RenderError::UnsupportedType {
			context: context.to_string(),
			kind: "numberTypeNode",
			reason: "only little-endian number types are supported".to_string(),
		});
	}

	match number_type.format {
		NumberFormat::U8 => Ok("u8".to_string()),
		NumberFormat::I8 => Ok("i8".to_string()),
		NumberFormat::U16 => Ok("pina_pod_primitives::PodU16".to_string()),
		NumberFormat::I16 => Ok("pina_pod_primitives::PodI16".to_string()),
		NumberFormat::U32 => Ok("pina_pod_primitives::PodU32".to_string()),
		NumberFormat::I32 => Ok("pina_pod_primitives::PodI32".to_string()),
		NumberFormat::U64 => Ok("pina_pod_primitives::PodU64".to_string()),
		NumberFormat::I64 => Ok("pina_pod_primitives::PodI64".to_string()),
		NumberFormat::U128 => Ok("pina_pod_primitives::PodU128".to_string()),
		NumberFormat::I128 => Ok("pina_pod_primitives::PodI128".to_string()),
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
		|| !matches!(number_type.endian, Endian::Little)
	{
		return Err(RenderError::UnsupportedType {
			context: context.to_string(),
			kind: "booleanTypeNode",
			reason: "booleans must be encoded as little-endian u8".to_string(),
		});
	}
	Ok("pina_pod_primitives::PodBool".to_string())
}

pub(crate) fn render_defined_type_page(defined_type: &DefinedTypeNode) -> Result<String> {
	let name = pascal(defined_type.name.as_ref());
	let context = format!("defined type `{name}`");
	match &defined_type.r#type {
		TypeNode::Struct(struct_type) => {
			render_defined_struct(name.as_str(), struct_type, &defined_type.docs)
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

fn render_defined_struct(name: &str, struct_type: &StructTypeNode, docs: &Docs) -> Result<String> {
	let mut lines = Vec::new();
	for doc_line in render_docs(docs, 0) {
		lines.push(doc_line);
	}
	lines.push("#[repr(C)]".to_string());
	lines.push(
		"#[derive(Clone, Copy, Debug, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]"
			.to_string(),
	);
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
