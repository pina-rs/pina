//! Instruction argument rendering: field types, wire sizes, and data writes.

use codama_nodes::ArrayTypeNode;
use codama_nodes::CountNode;
use codama_nodes::Endianness;
use codama_nodes::HasKind;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::TypeNode;
use heck::ToSnakeCase;

use crate::error::RenderError;
use crate::error::Result;

/// How one instruction argument appears in the generated builder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RenderedArgument {
	/// Rust field name in the builder struct.
	pub(crate) field: String,
	/// Rust field type (owned; builder fields hold the value or a reference).
	pub(crate) rust_type: String,
	/// Byte width of the argument on the wire.
	pub(crate) wire_size: usize,
	/// Statement writing the argument into the data buffer at `offset`.
	pub(crate) write: String,
	/// Docs attached to the argument, already indented.
	pub(crate) docs: Vec<String>,
}

/// Renders a non-omitted instruction argument.
pub(crate) fn render_argument(
	name: &str,
	argument_type: &TypeNode,
	context: &str,
) -> Result<RenderedArgument> {
	let field = name.to_snake_case();
	match argument_type {
		TypeNode::Number(number_type) => render_number_argument(&field, number_type, context),
		TypeNode::Boolean(_) => {
			Ok(RenderedArgument {
				field,
				rust_type: "bool".to_string(),
				wire_size: 1,
				write: "data[{offset}] = u8::from(self.{field});".to_string(),
				docs: Vec::new(),
			})
		}
		TypeNode::PublicKey(_) => {
			Ok(RenderedArgument {
				write: format!(
					"data[{{offset}}..{{offset_end}}].copy_from_slice(self.{field}.as_ref());"
				),
				field,
				rust_type: "Address".to_string(),
				wire_size: 32,
				docs: Vec::new(),
			})
		}
		TypeNode::Array(array_type) => render_array_argument(&field, array_type, context),
		TypeNode::FixedSize(fixed_size) => render_fixed_size_argument(&field, fixed_size, context),
		other => {
			Err(RenderError::UnsupportedType {
				context: context.to_string(),
				kind: other.kind(),
				reason: "only little-endian numbers, booleans, public keys, and fixed u8 arrays \
				         are supported as instruction arguments"
					.to_string(),
			})
		}
	}
}

fn render_number_argument(
	field: &str,
	number_type: &NumberTypeNode,
	context: &str,
) -> Result<RenderedArgument> {
	if !matches!(number_type.endian, Endianness::Le) {
		return Err(RenderError::UnsupportedType {
			context: context.to_string(),
			kind: "numberTypeNode",
			reason: "only little-endian numbers are supported as instruction arguments".to_string(),
		});
	}

	let (rust_type, wire_size) = match number_type.format {
		NumberFormat::U8 => ("u8", 1),
		NumberFormat::U16 => ("u16", 2),
		NumberFormat::U32 => ("u32", 4),
		NumberFormat::U64 => ("u64", 8),
		NumberFormat::U128 => ("u128", 16),
		NumberFormat::I8
		| NumberFormat::I16
		| NumberFormat::I32
		| NumberFormat::I64
		| NumberFormat::I128
		| NumberFormat::F32
		| NumberFormat::F64
		| NumberFormat::ShortU16 => {
			return Err(RenderError::UnsupportedType {
				context: context.to_string(),
				kind: "numberTypeNode",
				reason: format!("unsupported argument format `{:?}`", number_type.format),
			});
		}
	};

	let write =
		format!("data[{{offset}}..{{offset_end}}].copy_from_slice(&self.{field}.to_le_bytes());");

	Ok(RenderedArgument {
		field: field.to_string(),
		rust_type: rust_type.to_string(),
		wire_size,
		write,
		docs: Vec::new(),
	})
}

fn render_fixed_size_argument(
	field: &str,
	fixed_size: &codama_nodes::FixedSizeTypeNode<TypeNode>,
	context: &str,
) -> Result<RenderedArgument> {
	if !matches!(fixed_size.r#type.as_ref(), TypeNode::Bytes(_)) {
		return Err(RenderError::UnsupportedType {
			context: context.to_string(),
			kind: "fixedSizeTypeNode",
			reason: "only fixed-size byte types are supported as instruction arguments".to_string(),
		});
	}

	let write = format!("data[{{offset}}..{{offset_end}}].copy_from_slice(&self.{field});");

	Ok(RenderedArgument {
		write,
		field: field.to_string(),
		rust_type: format!("[u8; {}]", fixed_size.size),
		wire_size: fixed_size.size,
		docs: Vec::new(),
	})
}

fn render_array_argument(
	field: &str,
	array_type: &ArrayTypeNode,
	context: &str,
) -> Result<RenderedArgument> {
	let CountNode::Fixed(count) = array_type.count.as_ref() else {
		return Err(RenderError::UnsupportedType {
			context: context.to_string(),
			kind: array_type.count.kind(),
			reason: "only fixed-size u8 arrays are supported as instruction arguments".to_string(),
		});
	};

	if !matches!(
		array_type.item.as_ref(),
		TypeNode::Number(NumberTypeNode {
			format: NumberFormat::U8,
			endian: Endianness::Le,
			..
		})
	) {
		return Err(RenderError::UnsupportedType {
			context: context.to_string(),
			kind: "arrayTypeNode",
			reason: "only fixed-size u8 arrays are supported as instruction arguments".to_string(),
		});
	}

	let count = usize::try_from(count.value).map_err(|_| {
		RenderError::UnsupportedType {
			context: context.to_string(),
			kind: "fixedCountNode",
			reason: "array length does not fit in a usize".to_string(),
		}
	})?;

	let write = format!("data[{{offset}}..{{offset_end}}].copy_from_slice(&self.{field});");

	Ok(RenderedArgument {
		field: field.to_string(),
		rust_type: format!("[u8; {count}]"),
		wire_size: count,
		write,
		docs: Vec::new(),
	})
}
