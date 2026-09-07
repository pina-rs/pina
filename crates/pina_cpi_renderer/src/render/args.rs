//! Instruction argument rendering: field types, wire sizes, and data writes.

use codama_nodes::ArrayTypeNode;
use codama_nodes::CountNode;
use codama_nodes::Endianness;
use codama_nodes::HasKind;
use codama_nodes::NestedTypeNodeTrait;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::TypeNode;
use heck::ToSnakeCase;

use crate::error::RenderError;
use crate::error::Result;
use crate::render::helpers::rust_identifier;

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
	/// Whether the generated field borrows caller-owned data.
	pub(crate) borrows: bool,
	/// Docs attached to the argument, already indented.
	pub(crate) docs: Vec<String>,
}

/// Renders a non-omitted instruction argument.
pub(crate) fn render_argument(
	name: &str,
	argument_type: &TypeNode,
	context: &str,
) -> Result<RenderedArgument> {
	let field = rust_identifier(&name.to_snake_case(), context)?;
	match argument_type {
		TypeNode::Number(number_type) => render_number_argument(&field, number_type, context),
		TypeNode::Boolean(_) => {
			Ok(RenderedArgument {
				field,
				rust_type: "bool".to_string(),
				wire_size: 1,
				write: "data[{offset}] = u8::from(self.{field});".to_string(),
				borrows: false,
				docs: Vec::new(),
			})
		}
		TypeNode::PublicKey(_) => {
			Ok(RenderedArgument {
				write: format!(
					"data[{{offset}}..{{offset_end}}].copy_from_slice(self.{field}.as_ref());"
				),
				field,
				rust_type: "&'argument Address".to_string(),
				borrows: true,
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
		borrows: false,
		docs: Vec::new(),
	})
}

fn render_fixed_size_argument(
	field: &str,
	fixed_size: &codama_nodes::FixedSizeTypeNode<TypeNode>,
	context: &str,
) -> Result<RenderedArgument> {
	match fixed_size.r#type.as_ref() {
		TypeNode::Bytes(_) => render_fixed_bytes_argument(field, fixed_size.size),
		TypeNode::SizePrefix(prefix) if matches!(prefix.r#type.as_ref(), TypeNode::String(_)) => {
			render_pinapod_string_argument(field, fixed_size.size, prefix, context)
		}
		TypeNode::Array(array) => {
			render_pinapod_vec_argument(field, fixed_size.size, array, context)
		}
		_ => {
			Err(RenderError::UnsupportedType {
				context: context.to_string(),
				kind: "fixedSizeTypeNode",
				reason: "only fixed-size bytes and PinaPod String/Vec representations are \
				         supported as instruction arguments"
					.to_string(),
			})
		}
	}
}

fn render_fixed_bytes_argument(field: &str, wire_size: usize) -> Result<RenderedArgument> {
	Ok(RenderedArgument {
		write: format!("data[{{offset}}..{{offset_end}}].copy_from_slice(&self.{field});"),
		field: field.to_string(),
		rust_type: format!("[u8; {wire_size}]"),
		wire_size,
		borrows: false,
		docs: Vec::new(),
	})
}

fn render_pinapod_string_argument(
	field: &str,
	wire_size: usize,
	prefix: &codama_nodes::SizePrefixTypeNode<TypeNode>,
	context: &str,
) -> Result<RenderedArgument> {
	let prefix_size = number_size(prefix.prefix.get_nested_type_node()).ok_or_else(|| {
		RenderError::UnsupportedType {
			context: context.to_string(),
			kind: "sizePrefixTypeNode",
			reason: "PinaPod String prefixes must be little-endian u8, u16, u32, or u64"
				.to_string(),
		}
	})?;
	let capacity = wire_size.checked_sub(prefix_size).ok_or_else(|| {
		RenderError::UnsupportedType {
			context: context.to_string(),
			kind: "fixedSizeTypeNode",
			reason: "PinaPod String fixed size is smaller than its length prefix".to_string(),
		}
	})?;
	validate_prefix_capacity(prefix.prefix.get_nested_type_node(), capacity, context)?;
	let prefix_type = unsigned_prefix_type(prefix.prefix.get_nested_type_node(), context)?;
	let write = format!(
		"let value = self.{field}.as_bytes();\n\t\tif value.len() > {capacity} {{\n\t\t\treturn \
		 Err(ProgramError::InvalidInstructionData);\n\t\t}}\n\t\tdata[{{offset}}..{{offset}} + \
		 {prefix_size}].copy_from_slice(&(value.len() as \
		 {prefix_type}).to_le_bytes());\n\t\tdata[{{offset}} + {prefix_size}..{{offset}} + \
		 {prefix_size} + value.len()].copy_from_slice(value);"
	);

	Ok(RenderedArgument {
		field: field.to_string(),
		rust_type: "&'argument str".to_string(),
		wire_size,
		write,
		borrows: true,
		docs: Vec::new(),
	})
}

fn render_pinapod_vec_argument(
	field: &str,
	wire_size: usize,
	array: &ArrayTypeNode,
	context: &str,
) -> Result<RenderedArgument> {
	let CountNode::Prefixed(count) = array.count.as_ref() else {
		return Err(RenderError::UnsupportedType {
			context: context.to_string(),
			kind: array.count.kind(),
			reason: "PinaPod Vec requires a fixed-width count prefix".to_string(),
		});
	};
	let prefix_size = number_size(count.prefix.get_nested_type_node()).ok_or_else(|| {
		RenderError::UnsupportedType {
			context: context.to_string(),
			kind: "prefixedCountNode",
			reason: "PinaPod Vec prefixes must be little-endian u8, u16, u32, or u64".to_string(),
		}
	})?;
	let prefix_type = unsigned_prefix_type(count.prefix.get_nested_type_node(), context)?;
	let item = render_vec_item(&array.item, context)?;
	let payload_size = wire_size.checked_sub(prefix_size).ok_or_else(|| {
		RenderError::UnsupportedType {
			context: context.to_string(),
			kind: "fixedSizeTypeNode",
			reason: "PinaPod Vec fixed size is smaller than its count prefix".to_string(),
		}
	})?;
	if item.wire_size == 0 || payload_size % item.wire_size != 0 {
		return Err(RenderError::UnsupportedType {
			context: context.to_string(),
			kind: "fixedSizeTypeNode",
			reason: "PinaPod Vec fixed size is not an exact number of fixed-size elements"
				.to_string(),
		});
	}
	let capacity = payload_size / item.wire_size;
	validate_prefix_capacity(count.prefix.get_nested_type_node(), capacity, context)?;
	let item_write = item
		.write
		.replace("{item}", "value")
		.replace("{item_offset}", "item_offset")
		.replace("{item_end}", &format!("item_offset + {}", item.wire_size));
	let write = format!(
		"if self.{field}.len() > {capacity} {{\n\t\t\treturn \
		 Err(ProgramError::InvalidInstructionData);\n\t\t}}\n\t\tdata[{{offset}}..{{offset}} + \
		 {prefix_size}].copy_from_slice(&(self.{field}.len() as \
		 {prefix_type}).to_le_bytes());\n\t\tfor (index, value) in \
		 self.{field}.iter().enumerate() {{\n\t\t\tlet item_offset = {{offset}} + {prefix_size} + \
		 index * {};\n\t\t\t{item_write}\n\t\t}}",
		item.wire_size,
	);

	Ok(RenderedArgument {
		field: field.to_string(),
		rust_type: format!("&'argument [{}]", item.rust_type),
		wire_size,
		write,
		borrows: true,
		docs: Vec::new(),
	})
}

struct RenderedVecItem {
	rust_type: String,
	wire_size: usize,
	write: String,
}

fn render_vec_item(r#type: &TypeNode, context: &str) -> Result<RenderedVecItem> {
	match r#type {
		TypeNode::Number(number) => {
			let wire_size =
				number_size(number).ok_or_else(|| unsupported_vec_item(r#type, context))?;
			let rust_type = match number.format {
				NumberFormat::U8 => "u8",
				NumberFormat::U16 => "u16",
				NumberFormat::U32 => "u32",
				NumberFormat::U64 => "u64",
				NumberFormat::U128 => "u128",
				NumberFormat::I8 => "i8",
				NumberFormat::I16 => "i16",
				NumberFormat::I32 => "i32",
				NumberFormat::I64 => "i64",
				NumberFormat::I128 => "i128",
				NumberFormat::F32 | NumberFormat::F64 | NumberFormat::ShortU16 => {
					return Err(unsupported_vec_item(r#type, context));
				}
			};
			Ok(RenderedVecItem {
				rust_type: rust_type.to_string(),
				wire_size,
				write: "data[{item_offset}..{item_end}].copy_from_slice(&{item}.to_le_bytes());"
					.to_string(),
			})
		}
		TypeNode::Boolean(_) => {
			Ok(RenderedVecItem {
				rust_type: "bool".to_string(),
				wire_size: 1,
				write: "data[{item_offset}] = u8::from(*{item});".to_string(),
			})
		}
		TypeNode::PublicKey(_) => {
			Ok(RenderedVecItem {
				rust_type: "Address".to_string(),
				wire_size: 32,
				write: "data[{item_offset}..{item_end}].copy_from_slice({item}.as_ref());"
					.to_string(),
			})
		}
		TypeNode::FixedSize(fixed) if matches!(fixed.r#type.as_ref(), TypeNode::Bytes(_)) => {
			Ok(RenderedVecItem {
				rust_type: format!("[u8; {}]", fixed.size),
				wire_size: fixed.size,
				write: "data[{item_offset}..{item_end}].copy_from_slice({item});".to_string(),
			})
		}
		_ => Err(unsupported_vec_item(r#type, context)),
	}
}

fn unsupported_vec_item(r#type: &TypeNode, context: &str) -> RenderError {
	RenderError::UnsupportedType {
		context: context.to_string(),
		kind: r#type.kind(),
		reason: "PinaPod Vec CPI arguments currently support native integers, booleans, public \
		         keys, and fixed byte arrays"
			.to_string(),
	}
}

fn unsigned_prefix_type(number: &NumberTypeNode, context: &str) -> Result<&'static str> {
	if number.endian != Endianness::Le {
		return Err(RenderError::UnsupportedType {
			context: context.to_string(),
			kind: "numberTypeNode",
			reason: "PinaPod collection prefixes must be little-endian unsigned integers"
				.to_string(),
		});
	}
	match number.format {
		NumberFormat::U8 => Ok("u8"),
		NumberFormat::U16 => Ok("u16"),
		NumberFormat::U32 => Ok("u32"),
		NumberFormat::U64 => Ok("u64"),
		_ => {
			Err(RenderError::UnsupportedType {
				context: context.to_string(),
				kind: "numberTypeNode",
				reason: "PinaPod collection prefixes must use u8, u16, u32, or u64".to_string(),
			})
		}
	}
}

fn validate_prefix_capacity(number: &NumberTypeNode, capacity: usize, context: &str) -> Result<()> {
	let maximum = match number.format {
		NumberFormat::U8 => u8::MAX as usize,
		NumberFormat::U16 => u16::MAX as usize,
		NumberFormat::U32 => usize::try_from(u32::MAX).unwrap_or(usize::MAX),
		NumberFormat::U64 => usize::MAX,
		_ => return unsigned_prefix_type(number, context).map(|_| ()),
	};
	if capacity > maximum {
		return Err(RenderError::UnsupportedType {
			context: context.to_string(),
			kind: "fixedSizeTypeNode",
			reason: format!(
				"PinaPod collection capacity {capacity} exceeds the maximum {maximum} \
				 representable by its prefix"
			),
		});
	}
	Ok(())
}

fn number_size(number: &NumberTypeNode) -> Option<usize> {
	if number.endian != Endianness::Le {
		return None;
	}

	match number.format {
		NumberFormat::U8 | NumberFormat::I8 => Some(1),
		NumberFormat::U16 | NumberFormat::I16 => Some(2),
		NumberFormat::U32 | NumberFormat::I32 | NumberFormat::F32 => Some(4),
		NumberFormat::U64 | NumberFormat::I64 | NumberFormat::F64 => Some(8),
		NumberFormat::U128 | NumberFormat::I128 => Some(16),
		NumberFormat::ShortU16 => None,
	}
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

	let count = count.value as usize;

	let write = format!("data[{{offset}}..{{offset_end}}].copy_from_slice(&self.{field});");

	Ok(RenderedArgument {
		field: field.to_string(),
		rust_type: format!("[u8; {count}]"),
		wire_size: count,
		write,
		borrows: false,
		docs: Vec::new(),
	})
}

#[cfg(test)]
mod tests {
	use codama_nodes::BooleanTypeNode;
	use codama_nodes::BytesTypeNode;
	use codama_nodes::F32;
	use codama_nodes::F64;
	use codama_nodes::I8;
	use codama_nodes::I16;
	use codama_nodes::I32;
	use codama_nodes::I64;
	use codama_nodes::I128;
	use codama_nodes::ShortU16;
	use codama_nodes::SizePrefixTypeNode;
	use codama_nodes::StringTypeNode;
	use codama_nodes::U8;
	use codama_nodes::U16;
	use codama_nodes::U32;
	use codama_nodes::U64;
	use codama_nodes::U128;

	use super::*;

	#[test]
	fn rejects_unsupported_argument_shapes() {
		assert!(render_argument("name", &StringTypeNode::utf8().into(), "test").is_err());
		assert!(render_argument("name", &NumberTypeNode::be(U16).into(), "test").is_err());

		for format in [I8, I16, I32, I64, I128, F32, F64, ShortU16] {
			assert!(render_argument("name", &NumberTypeNode::le(format).into(), "test").is_err());
		}

		let fixed_boolean = codama_nodes::FixedSizeTypeNode::new(BooleanTypeNode::default(), 4);
		assert!(render_argument("name", &fixed_boolean.into(), "test").is_err());
		let remainder = ArrayTypeNode::remainder(NumberTypeNode::le(U8));
		assert!(render_argument("name", &remainder.into(), "test").is_err());
		let wrong_item = ArrayTypeNode::fixed(NumberTypeNode::le(U16), 4);
		assert!(render_argument("name", &wrong_item.into(), "test").is_err());
	}

	#[test]
	fn renders_every_supported_argument_shape() {
		for (format, rust_type, wire_size) in [
			(U8, "u8", 1),
			(U16, "u16", 2),
			(U32, "u32", 4),
			(U64, "u64", 8),
			(U128, "u128", 16),
		] {
			let rendered = render_argument("someValue", &NumberTypeNode::le(format).into(), "test")
				.unwrap_or_else(|error| panic!("number should render: {error}"));
			assert_eq!(rendered.field, "some_value");
			assert_eq!(rendered.rust_type, rust_type);
			assert_eq!(rendered.wire_size, wire_size);
		}

		let fixed = codama_nodes::FixedSizeTypeNode::new(BytesTypeNode {}, 12);
		let rendered = render_argument("bytes", &fixed.into(), "test")
			.unwrap_or_else(|error| panic!("fixed bytes should render: {error}"));
		assert_eq!(rendered.rust_type, "[u8; 12]");
	}

	#[test]
	fn renders_semantic_pinapod_collections_at_the_cpi_edge() {
		let string =
			SizePrefixTypeNode::<TypeNode>::new(StringTypeNode::utf8(), NumberTypeNode::le(U8));
		let string = codama_nodes::FixedSizeTypeNode::new(string, 33);
		let rendered = render_argument("name", &string.into(), "test")
			.unwrap_or_else(|error| panic!("PinaPod String should render: {error}"));
		assert_eq!(rendered.rust_type, "&'argument str");
		assert_eq!(rendered.wire_size, 33);
		assert!(rendered.borrows);
		assert!(rendered.write.contains("value.len() > 32"));
		assert!(rendered.write.contains("InvalidInstructionData"));

		let vector = ArrayTypeNode::prefixed(NumberTypeNode::le(U64), NumberTypeNode::le(U16));
		let vector = codama_nodes::FixedSizeTypeNode::new(vector, 66);
		let rendered = render_argument("tags", &vector.into(), "test")
			.unwrap_or_else(|error| panic!("PinaPod Vec should render: {error}"));
		assert_eq!(rendered.rust_type, "&'argument [u64]");
		assert_eq!(rendered.wire_size, 66);
		assert!(rendered.write.contains("self.tags.len() > 8"));
		assert!(rendered.write.contains("value.to_le_bytes()"));
	}

	#[test]
	fn rejects_malformed_or_unsupported_semantic_fixed_containers() {
		let string =
			SizePrefixTypeNode::<TypeNode>::new(StringTypeNode::utf8(), NumberTypeNode::le(U16));
		let too_small = codama_nodes::FixedSizeTypeNode::new(string, 1);
		let error = render_argument("name", &too_small.into(), "test")
			.expect_err("a fixed string cannot be smaller than its prefix");
		assert!(error.to_string().contains("smaller than its length prefix"));

		let nested = ArrayTypeNode::prefixed(StringTypeNode::utf8(), NumberTypeNode::le(U8));
		let nested = codama_nodes::FixedSizeTypeNode::new(nested, 10);
		let error = render_argument("values", &nested.into(), "test")
			.expect_err("variable-size Vec elements must be rejected");
		assert!(
			error
				.to_string()
				.contains("currently support native integers")
		);
	}

	#[test]
	fn rejects_semantic_capacities_that_do_not_fit_the_prefix() {
		let string =
			SizePrefixTypeNode::<TypeNode>::new(StringTypeNode::utf8(), NumberTypeNode::le(U8));
		let string = codama_nodes::FixedSizeTypeNode::new(string, 257);
		let error = render_argument("name", &string.into(), "test")
			.expect_err("a u8 String prefix cannot encode capacity 256");
		assert!(
			error
				.to_string()
				.contains("capacity 256 exceeds the maximum 255")
		);

		let vector = ArrayTypeNode::prefixed(NumberTypeNode::le(U8), NumberTypeNode::le(U8));
		let vector = codama_nodes::FixedSizeTypeNode::new(vector, 257);
		let error = render_argument("values", &vector.into(), "test")
			.expect_err("a u8 Vec prefix cannot encode capacity 256");
		assert!(
			error
				.to_string()
				.contains("capacity 256 exceeds the maximum 255")
		);
	}
}
