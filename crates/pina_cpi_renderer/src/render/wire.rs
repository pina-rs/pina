//! Instruction-argument ABI planning.
//!
//! Real-world Anchor IDLs describe arguments as links into a shared
//! `definedTypes` table, and those types nest structs, enums, options, and
//! length-prefixed collections. This module resolves each argument down to a
//! shape with a defined instruction-data layout and emits the Rust statements
//! that write it.
//!
//! Encodings advance a local `offset` into a data buffer:
//!
//! ```text
//! let mut offset = 0usize;
//! data[offset..offset + 8].copy_from_slice(&self.amount.to_le_bytes());
//! offset += 8;
//! ```
//!
//! A struct or enum argument resolves to a generated Rust type with its own
//! `encode_into`, so nesting stays recursive instead of unrolling. Every other
//! shape is written inline. A fixed-size encoding knows its width statically,
//! so the generated instruction can return `[u8; LEN]`; a variable-size
//! encoding reports `MAX_LEN` and writes into a caller-provided buffer.

use std::collections::BTreeMap;

use codama_nodes::CountNode;
use codama_nodes::DefinedTypeNode;
use codama_nodes::Endianness;
use codama_nodes::EnumTypeNode;
use codama_nodes::EnumVariantTypeNode;
use codama_nodes::HasKind;
use codama_nodes::NestedTypeNodeTrait;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::StringTypeNode;
use codama_nodes::TypeNode;
use heck::ToUpperCamelCase;

use crate::error::RenderError;
use crate::error::Result;

/// Resolver and registry for `definedTypeLinkNode` references.
///
/// Anchor IDLs route most non-primitive arguments through this table, so every
/// argument type resolves through here. Types that need a generated Rust
/// declaration are recorded as they are found, which is what lets the renderer
/// emit only the types an IDL actually uses.
#[derive(Clone, Debug, Default)]
pub(crate) struct TypeIndex {
	types: BTreeMap<String, TypeNode>,
	/// Referenced named types, in first-seen order.
	named: Vec<String>,
}

impl TypeIndex {
	pub(crate) fn new(defined_types: &[DefinedTypeNode]) -> Self {
		Self {
			types: defined_types
				.iter()
				.map(|defined| {
					(
						defined.name.as_ref().to_string(),
						defined.r#type.as_ref().clone(),
					)
				})
				.collect(),
			named: Vec::new(),
		}
	}

	/// Registered named types, in first-seen order.
	pub(crate) fn named(&self) -> &[String] {
		&self.named
	}

	/// The declared type behind `name`, when the IDL declares it.
	pub(crate) fn declared(&self, name: &str) -> Option<&TypeNode> {
		self.types.get(name)
	}

	/// Registers `name` as a type needing a generated Rust declaration.
	fn register(&mut self, name: &str) {
		if !self.named.iter().any(|existing| existing == name) {
			self.named.push(name.to_string());
		}
	}

	/// Resolves `r#type`, following `definedTypeLinkNode` aliases.
	///
	/// # Errors
	///
	/// Returns an error for a link to an undeclared type or a cyclic alias.
	fn resolve(&self, r#type: &TypeNode, context: &str) -> Result<TypeNode> {
		let mut current = r#type.clone();
		let mut stack: Vec<String> = Vec::new();

		loop {
			let TypeNode::Link(link) = &current else {
				return Ok(current);
			};
			let name = link.name.as_ref().to_string();
			if stack.contains(&name) {
				return Err(unsupported(
					context,
					"definedTypeLinkNode",
					&format!("defined type `{name}` is cyclic"),
				));
			}
			stack.push(name.clone());
			current = self.types.get(&name).cloned().ok_or_else(|| {
				unsupported(
					context,
					"definedTypeLinkNode",
					&format!("defined type `{name}` is not declared in this IDL's `definedTypes`"),
				)
			})?;
		}
	}
}

/// Rust type and ABI encoding for one argument or field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Encoded {
	/// Rust type written into the generated struct.
	pub(crate) rust_type: String,
	/// Byte width when fixed, `None` when it varies.
	pub(crate) fixed_size: Option<usize>,
	/// Largest possible byte width, used to size buffers.
	pub(crate) max_size: usize,
	/// Statements writing the value, advancing a local `offset`.
	pub(crate) encode: String,
	/// Whether the Rust type borrows caller-owned data.
	pub(crate) borrows: bool,
}

impl Encoded {
	pub(crate) const fn is_variable(&self) -> bool {
		self.fixed_size.is_none()
	}

	/// Rebinds the expression the encoding reads from.
	pub(crate) fn with_value(mut self, value: &str) -> Self {
		self.encode = self.encode.replace("self_value", value);
		self
	}
}

/// Plans the encoding of one argument.
///
/// # Errors
///
/// Returns an error naming the unsupported node when the type has no
/// instruction-data layout.
pub(crate) fn plan(r#type: &TypeNode, types: &mut TypeIndex, context: &str) -> Result<Encoded> {
	if let TypeNode::Link(link) = r#type {
		let name = link.name.as_ref().to_string();
		let declared = types.declared(&name).cloned().ok_or_else(|| {
			unsupported(
				context,
				"definedTypeLinkNode",
				&format!("defined type `{name}` is not declared in this IDL's `definedTypes`"),
			)
		})?;

		// Aliases resolve to whatever they ultimately name; structs and enums
		// become generated Rust types with their own encoder.
		let target = types.resolve(&declared, context)?;
		match target {
			TypeNode::Struct(_) | TypeNode::Enum(_) => {
				types.register(&name);
				let size = plan_resolved(&target, types, context)?;

				// A generated type takes `<'a>` only when one of its fields
				// borrows, so the use site has to name that lifetime.
				let mut rust_type = pascal(&name);
				if size.borrows {
					rust_type.push_str("<'argument>");
				}

				return Ok(Encoded {
					rust_type,
					fixed_size: size.fixed_size,
					max_size: size.max_size,
					encode: "self_value.encode_into(&mut data[..], &mut offset)?;".to_string(),
					borrows: size.borrows,
				});
			}
			other => return plan_resolved(&other, types, context),
		}
	}

	plan_resolved(r#type, types, context)
}

/// Plans a type that is not a link.
fn plan_resolved(r#type: &TypeNode, types: &mut TypeIndex, context: &str) -> Result<Encoded> {
	match r#type {
		TypeNode::Number(number) => plan_number(number, context),
		TypeNode::Boolean(_) => {
			Ok(Encoded {
				rust_type: "bool".to_string(),
				fixed_size: Some(1),
				max_size: 1,
				encode: "data[offset] = u8::from(*self_value);\noffset += 1;".to_string(),
				borrows: false,
			})
		}
		TypeNode::PublicKey(_) => {
			Ok(Encoded {
				rust_type: "Address".to_string(),
				fixed_size: Some(32),
				max_size: 32,
				encode: "data[offset..offset + 32].copy_from_slice(self_value.as_ref());\noffset \
				         += 32;"
					.to_string(),
				borrows: false,
			})
		}
		TypeNode::Bytes(_) => {
			Err(unsupported(
				context,
				"bytesTypeNode",
				"a bare byte slice carries no length prefix, so a reader cannot tell where it \
				 ends; wrap it in a `sizePrefixTypeNode` or `fixedSizeTypeNode`",
			))
		}
		TypeNode::String(_) => {
			Err(unsupported(
				context,
				"stringTypeNode",
				"a bare string carries no length prefix, so a reader cannot tell where it ends; \
				 wrap it in a `sizePrefixTypeNode`",
			))
		}
		TypeNode::FixedSize(fixed) => plan_fixed_size(&fixed.r#type, fixed.size, types, context),
		TypeNode::Array(array) => {
			let count = array.count.clone();
			let item = plan(&array.item, types, context)?;
			plan_array(&count, &item, context)
		}
		TypeNode::Option(option) => {
			let prefix = option.prefix.get_nested_type_node().clone();
			let item = plan(&option.item, types, context)?;
			plan_option(&prefix, &item, option.fixed, context)
		}
		TypeNode::SizePrefix(prefix) => {
			let width = prefix_width(prefix.prefix.get_nested_type_node(), context)?;
			let inner = prefix.r#type.as_ref().clone();
			plan_size_prefix(&inner, width, types, context)
		}
		TypeNode::Struct(structure) => {
			let fields = structure.fields.clone();
			plan_struct(&fields, types, context)
		}
		TypeNode::Enum(enumeration) => plan_enum(enumeration, types, context),
		TypeNode::Map(map) => {
			let count = map.count.clone();
			let key = plan(&map.key, types, &format!("{context} map key"))?;
			let value = plan(&map.value, types, &format!("{context} map value"))?;
			plan_map(&count, &key, &value, context)
		}
		TypeNode::Tuple(tuple) => {
			let mut planned = Vec::new();
			for (index, item) in tuple.items.iter().enumerate() {
				planned.push(plan(
					item,
					types,
					&format!("{context} tuple element {index}"),
				)?);
			}
			Ok(combine(&planned))
		}
		TypeNode::Link(_) => {
			Err(unsupported(
				context,
				"linkNode",
				"only `definedTypeLinkNode` references can be resolved from `definedTypes`",
			))
		}
		other => {
			Err(unsupported(
				context,
				other.kind(),
				"this node has no instruction-data encoding",
			))
		}
	}
}

fn plan_number(number: &NumberTypeNode, context: &str) -> Result<Encoded> {
	let size = integer_width(number, context)?;
	let rust_type = integer_type_name(number, context)?;

	Ok(Encoded {
		rust_type: rust_type.to_string(),
		fixed_size: Some(size),
		max_size: size,
		encode: format!(
			"data[offset..offset + {size}].copy_from_slice(&(*self_value).to_le_bytes());\noffset \
			 += {size};"
		),
		borrows: false,
	})
}

fn plan_fixed_size(
	inner: &TypeNode,
	size: usize,
	types: &mut TypeIndex,
	context: &str,
) -> Result<Encoded> {
	let resolved = types.resolve(inner, context)?;

	match &resolved {
		TypeNode::Bytes(_) => {
			Ok(Encoded {
				rust_type: format!("[u8; {size}]"),
				fixed_size: Some(size),
				max_size: size,
				encode: format!(
					"data[offset..offset + {size}].copy_from_slice(&self_value[..]);\noffset += \
					 {size};"
				),
				borrows: false,
			})
		}
		// A PinaPod-style field: the length prefix and payload share one fixed
		// window, so the window's width is authoritative.
		TypeNode::SizePrefix(_) | TypeNode::Array(_) => {
			let planned = plan_resolved(&resolved, types, context)?;
			match planned.fixed_size {
				Some(inner_size) if inner_size == size => Ok(planned),
				_ if planned.max_size <= size => {
					Ok(Encoded {
						max_size: size,
						..planned
					})
				}
				_ => {
					Err(unsupported(
						context,
						"fixedSizeTypeNode",
						&format!(
							"a {size}-byte window cannot hold a payload of at most {} bytes",
							planned.max_size
						),
					))
				}
			}
		}
		_ => {
			let planned = plan_resolved(&resolved, types, context)?;
			match planned.fixed_size {
				Some(inner_size) if inner_size == size => Ok(planned),
				Some(inner_size) => {
					Err(unsupported(
						context,
						"fixedSizeTypeNode",
						&format!(
							"declared size {size} does not match the {inner_size}-byte payload it \
							 wraps"
						),
					))
				}
				None => {
					Err(unsupported(
						context,
						"fixedSizeTypeNode",
						"a fixed-size window cannot hold a variable-length payload",
					))
				}
			}
		}
	}
}

fn plan_array(count: &CountNode, item: &Encoded, context: &str) -> Result<Encoded> {
	match count {
		CountNode::Fixed(count) => {
			let Some(item_size) = item.fixed_size else {
				return Err(unsupported(
					context,
					"arrayTypeNode",
					"a fixed-count array cannot hold a variable-length element",
				));
			};
			let total = item_size.checked_mul(count.value as usize).ok_or_else(|| {
				unsupported(context, "arrayTypeNode", "array size overflows `usize`")
			})?;
			let item = item.clone().with_value("item");

			Ok(Encoded {
				rust_type: format!("[{}; {}]", item.rust_type, count.value),
				fixed_size: Some(total),
				max_size: total,
				encode: format!(
					"for item in self_value.iter() {{\n\t{}\n}}",
					indent(&item.encode, 1)
				),
				borrows: false,
			})
		}
		CountNode::Prefixed(count) => {
			let width = prefix_width(count.prefix.get_nested_type_node(), context)?;
			let prefix_type = integer_type_name(count.prefix.get_nested_type_node(), context)?;
			let maximum = max_for_prefix(count.prefix.get_nested_type_node(), context)?;
			let item = item.clone().with_value("item");

			Ok(Encoded {
				rust_type: format!("&'argument [{}]", item.rust_type),
				fixed_size: None,
				max_size: width.saturating_add(maximum.saturating_mul(item.max_size)),
				encode: format!(
					"if self_value.len() > {maximum} {{\n\treturn \
					 Err(ProgramError::InvalidInstructionData);\n}}\ndata[offset..offset + \
					 {width}].copy_from_slice(&(self_value.len() as \
					 {prefix_type}).to_le_bytes());\noffset += {width};\nfor item in \
					 self_value.iter() {{\n\t{}\n}}",
					indent(&item.encode, 1)
				),
				borrows: true,
			})
		}
		other => {
			Err(unsupported(
				context,
				other.kind(),
				"only fixed-count and length-prefixed arrays have an instruction-data layout",
			))
		}
	}
}

/// Borsh maps are a count prefix followed by key/value pairs, so they share the
/// prefixed-array layout with both items interleaved.
fn plan_map(count: &CountNode, key: &Encoded, value: &Encoded, context: &str) -> Result<Encoded> {
	let CountNode::Prefixed(count) = count else {
		return Err(unsupported(
			context,
			"mapTypeNode",
			"a Borsh map is written as a length-prefixed sequence of entries, so only a prefixed \
			 count can be encoded",
		));
	};

	let prefix = count.prefix.get_nested_type_node();
	let width = prefix_width(prefix, context)?;
	let prefix_type = integer_type_name(prefix, context)?;
	let maximum = max_for_prefix(prefix, context)?;
	let entry = combine(&[
		key.clone().with_value("key"),
		value.clone().with_value("value"),
	]);
	let entry_max = entry.max_size;
	let entry_write = format!(
		"for (key, value) in self_value.iter() {{\n\t{}\n}}",
		indent(&entry.encode, 1)
	);

	Ok(Encoded {
		rust_type: format!("&'argument [({}, {})]", key.rust_type, value.rust_type),
		fixed_size: None,
		max_size: width.saturating_add(maximum.saturating_mul(entry_max)),
		encode: format!(
			"if self_value.len() > {maximum} {{\n\treturn \
			 Err(ProgramError::InvalidInstructionData);\n}}\ndata[offset..offset + \
			 {width}].copy_from_slice(&(self_value.len() as \
			 {prefix_type}).to_le_bytes());\noffset += {width};\n{entry_write}"
		),
		borrows: true,
	})
}

fn plan_option(
	prefix: &NumberTypeNode,
	item: &Encoded,
	fixed: Option<bool>,
	context: &str,
) -> Result<Encoded> {
	let width = prefix_width(prefix, context)?;
	// The arm binds `value` from a borrowed `Option`, so it is already a
	// reference, matching the convention every encoder assumes.
	let item = item.clone().with_value("value");
	let present = "data[offset] = 1;\noffset += 1;";
	let absent = "data[offset] = 0;\noffset += 1;";

	Ok(Encoded {
		rust_type: format!("Option<{}>", item.rust_type),
		fixed_size: fixed
			.filter(|fixed| *fixed)
			.map(|_| width.saturating_add(item.max_size)),
		max_size: width.saturating_add(item.max_size),
		encode: if fixed == Some(true) {
			format!(
				"match self_value {{\n\tNone => {{\n\t\tdata[offset..offset + \
				 {width}].fill(0);\n\t\toffset += {width};\n\t}}\n\tSome(value) => \
				 {{\n\t\t{present}\n\t\t{}\n\t}}\n}}",
				indent(&item.encode, 2)
			)
		} else {
			format!(
				"match self_value {{\n\tNone => {{\n\t\t{absent}\n\t}}\n\tSome(value) => \
				 {{\n\t\t{present}\n\t\t{}\n\t}}\n}}",
				indent(&item.encode, 2)
			)
		},
		borrows: item.borrows,
	})
}

fn plan_size_prefix(
	inner: &TypeNode,
	width: usize,
	types: &mut TypeIndex,
	context: &str,
) -> Result<Encoded> {
	let resolved = types.resolve(inner, context)?;
	let prefix_type = "u32";

	let (rust_type, payload_max, encode) = match &resolved {
		TypeNode::String(StringTypeNode {
			encoding: codama_nodes::BytesEncoding::Utf8,
			..
		}) => {
			(
				"&'argument str".to_string(),
				max_for_prefix_width(width),
				format!(
					"let bytes = self_value.as_bytes();\ndata[offset..offset + \
					 {width}].copy_from_slice(&(bytes.len() as \
					 {prefix_type}).to_le_bytes());\noffset += {width};\ndata[offset..offset + \
					 bytes.len()].copy_from_slice(bytes);\noffset += bytes.len();"
				),
			)
		}
		TypeNode::String(_) => {
			return Err(unsupported(
				context,
				"stringTypeNode",
				"only UTF-8 strings can be encoded as instruction arguments",
			));
		}
		TypeNode::Bytes(_) => {
			(
				"&'argument [u8]".to_string(),
				max_for_prefix_width(width),
				format!(
					"data[offset..offset + {width}].copy_from_slice(&(self_value.len() as \
					 {prefix_type}).to_le_bytes());\noffset += {width};\ndata[offset..offset + \
					 self_value.len()].copy_from_slice(self_value);\noffset += self_value.len();"
				),
			)
		}
		TypeNode::Array(array) => {
			let CountNode::Prefixed(count) = array.count.as_ref() else {
				return Err(unsupported(
					context,
					"sizePrefixTypeNode",
					"a length-prefixed array must itself carry a count prefix",
				));
			};
			let count_width = prefix_width(count.prefix.get_nested_type_node(), context)?;
			let count_type = integer_type_name(count.prefix.get_nested_type_node(), context)?;
			let count_max = max_for_prefix(count.prefix.get_nested_type_node(), context)?;
			let item = plan(&array.item, types, context)?;
			let Some(item_size) = item.fixed_size else {
				return Err(unsupported(
					context,
					"sizePrefixTypeNode",
					"a length-prefixed array cannot hold a variable-length element",
				));
			};
			let item = item.with_value("item");
			(
				format!("&'argument [{}]", item.rust_type.clone()),
				count_max.saturating_mul(item_size),
				format!(
					"if self_value.len() > {count_max} {{\n\treturn \
					 Err(ProgramError::InvalidInstructionData);\n}}\nlet payload_len = \
					 self_value.len() * {item_size};\ndata[offset..offset + \
					 {width}].copy_from_slice(&(payload_len as \
					 {prefix_type}).to_le_bytes());\noffset += {width};\ndata[offset..offset + \
					 {count_width}].copy_from_slice(&(self_value.len() as \
					 {count_type}).to_le_bytes());\noffset += {count_width};\nfor item in \
					 self_value.iter() {{\n\t{}\n}}",
					indent(&item.encode, 1)
				),
			)
		}
		other => {
			return Err(unsupported(
				context,
				other.kind(),
				"a length-prefixed argument must wrap a string, byte slice, or array",
			));
		}
	};

	Ok(Encoded {
		rust_type,
		fixed_size: None,
		max_size: width.saturating_add(payload_max),
		encode,
		borrows: true,
	})
}

fn plan_struct(
	fields: &[codama_nodes::StructFieldTypeNode],
	types: &mut TypeIndex,
	context: &str,
) -> Result<Encoded> {
	let mut planned = Vec::new();
	for field in fields {
		if is_omitted(field) {
			continue;
		}
		planned.push(plan(
			&field.r#type,
			types,
			&format!("{context} field `{}`", field.name.as_ref()),
		)?);
	}

	Ok(combine(&planned))
}

fn plan_enum(enumeration: &EnumTypeNode, types: &mut TypeIndex, context: &str) -> Result<Encoded> {
	let tag = enumeration.size.get_nested_type_node().clone();
	let tag_width = prefix_width(&tag, context)?;
	let mut payload_max = 0usize;
	let mut payload_fixed: Option<usize> = Some(0);
	let mut borrows = false;

	for variant in &enumeration.variants {
		let payload = plan_variant_payload(variant, types, context)?;
		let combined = combine(&payload);
		payload_max = payload_max.max(combined.max_size);
		payload_fixed = match (payload_fixed, combined.fixed_size) {
			(Some(current), Some(size)) if current == size => Some(size),
			_ => None,
		};
		borrows |= combined.borrows;
	}

	// Borsh writes the tag followed by the variant payload, which is why an
	// enum whose variants differ in width is variable-length rather than
	// unsupported.
	let uniform = payload_fixed.is_some();

	Ok(Encoded {
		rust_type: String::new(),
		fixed_size: if uniform {
			payload_fixed.map(|size| tag_width.saturating_add(size))
		} else {
			None
		},
		max_size: tag_width.saturating_add(payload_max),
		encode: String::new(),
		borrows,
	})
}

/// Plans the payload of one enum variant.
pub(crate) fn plan_variant_payload(
	variant: &EnumVariantTypeNode,
	types: &mut TypeIndex,
	context: &str,
) -> Result<Vec<Encoded>> {
	let variant_name = variant_name(variant);

	match variant {
		EnumVariantTypeNode::Empty(_) => Ok(Vec::new()),
		EnumVariantTypeNode::Struct(variant) => {
			let fields = variant.r#struct.get_nested_type_node().fields.clone();
			let mut planned = Vec::new();
			for field in &fields {
				if is_omitted(field) {
					continue;
				}
				planned.push(plan(
					&field.r#type,
					types,
					&format!(
						"{context} variant `{variant_name}` field `{}`",
						field.name.as_ref()
					),
				)?);
			}
			Ok(planned)
		}
		EnumVariantTypeNode::Tuple(variant) => {
			let items = variant.tuple.get_nested_type_node().items.clone();
			items
				.iter()
				.enumerate()
				.map(|(index, item)| {
					plan(
						item,
						types,
						&format!("{context} variant `{variant_name}` field {index}"),
					)
				})
				.collect()
		}
	}
}

fn is_omitted(field: &codama_nodes::StructFieldTypeNode) -> bool {
	matches!(
		field.default_value_strategy,
		Some(codama_nodes::DefaultValueStrategy::Omitted)
	)
}

fn combine(fields: &[Encoded]) -> Encoded {
	Encoded {
		rust_type: String::new(),
		fixed_size: fields.iter().try_fold(0usize, |total, field| {
			field.fixed_size.map(|size| total.saturating_add(size))
		}),
		max_size: fields
			.iter()
			.fold(0usize, |total, field| total.saturating_add(field.max_size)),
		encode: fields
			.iter()
			.map(|field| field.encode.as_str())
			.collect::<Vec<_>>()
			.join("\n"),
		borrows: fields.iter().any(|field| field.borrows),
	}
}

/// The declared name of an enum variant.
pub(crate) fn variant_name(variant: &EnumVariantTypeNode) -> String {
	match variant {
		EnumVariantTypeNode::Empty(variant) => variant.name.as_ref().to_string(),
		EnumVariantTypeNode::Struct(variant) => variant.name.as_ref().to_string(),
		EnumVariantTypeNode::Tuple(variant) => variant.name.as_ref().to_string(),
	}
}

/// The Rust identifier for an enum variant.
pub(crate) fn enum_variant_name(name: &str) -> String {
	name.to_upper_camel_case()
}

fn integer_width(number: &NumberTypeNode, context: &str) -> Result<usize> {
	Ok(match integer_type_name(number, context)? {
		"u8" | "i8" => 1,
		"u16" | "i16" => 2,
		"u32" | "i32" => 4,
		"u64" | "i64" => 8,
		_ => 16,
	})
}

fn integer_type_name(number: &NumberTypeNode, context: &str) -> Result<&'static str> {
	if number.endian != Endianness::Le {
		return Err(unsupported(
			context,
			"numberTypeNode",
			"only little-endian numbers are supported as instruction arguments",
		));
	}

	Ok(match number.format {
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
		NumberFormat::F32 | NumberFormat::F64 => {
			return Err(unsupported(
				context,
				"numberTypeNode",
				"floating-point arguments are rejected: a no_std crate has no float ABI \
				 conversion, so the generated writer would have to reinterpret raw bits and \
				 silently disagree with a reader expecting a float; scale to an integer on the \
				 caller side instead",
			));
		}
		NumberFormat::ShortU16 => {
			return Err(unsupported(
				context,
				"numberTypeNode",
				"`shortU16` is rejected: Anchor encodes it as a 1-3 byte variable-length prefix \
				 and reserves it for on-chain account lengths, so a fixed-width little-endian \
				 write would disagree with Anchor's reader; declare the field as `u16`, or as a \
				 length-prefixed collection when it is a size",
			));
		}
	})
}

fn prefix_width(number: &NumberTypeNode, context: &str) -> Result<usize> {
	Ok(match integer_type_name(number, context)? {
		"u8" | "i8" => 1,
		"u16" | "i16" => 2,
		"u32" | "i32" => 4,
		"u64" | "i64" => 8,
		_ => {
			return Err(unsupported(
				context,
				"numberTypeNode",
				"length prefixes must be an integer of at most 8 bytes",
			));
		}
	})
}

/// Largest payload a length prefix of `width` bytes can describe.
const fn max_for_prefix_width(width: usize) -> usize {
	match width {
		1 => u8::MAX as usize,
		2 => u16::MAX as usize,
		4 => u32::MAX as usize,
		_ => usize::MAX,
	}
}

fn max_for_prefix(number: &NumberTypeNode, context: &str) -> Result<usize> {
	Ok(match prefix_width(number, context)? {
		1 => u8::MAX as usize,
		2 => u16::MAX as usize,
		4 => u32::MAX as usize,
		_ => usize::MAX,
	})
}

fn pascal(value: &str) -> String {
	value.to_upper_camel_case()
}

fn indent(body: &str, levels: usize) -> String {
	let tab = "\t".repeat(levels);

	body.lines()
		.map(|line| format!("{tab}{line}"))
		.collect::<Vec<_>>()
		.join("\n")
}

pub(crate) fn unsupported(context: &str, kind: &'static str, reason: &str) -> RenderError {
	RenderError::UnsupportedType {
		context: context.to_string(),
		kind,
		reason: reason.to_string(),
	}
}

#[cfg(test)]
mod tests {
	use codama_nodes::ArrayTypeNode;
	use codama_nodes::BooleanTypeNode;
	use codama_nodes::BytesTypeNode;
	use codama_nodes::DefinedTypeLinkNode;
	use codama_nodes::EnumEmptyVariantTypeNode;
	use codama_nodes::EnumStructVariantTypeNode;
	use codama_nodes::FixedSizeTypeNode;
	use codama_nodes::NumberFormat;
	use codama_nodes::NumberTypeNode;
	use codama_nodes::PublicKeyTypeNode;
	use codama_nodes::StructFieldTypeNode;
	use codama_nodes::StructTypeNode;

	use super::*;

	fn index(defined: Vec<DefinedTypeNode>) -> TypeIndex {
		TypeIndex::new(&defined)
	}

	#[test]
	fn rejects_short_u16_and_floats_with_actionable_reasons() {
		let error = plan(
			&NumberTypeNode::le(NumberFormat::ShortU16).into(),
			&mut index(Vec::new()),
			"test",
		)
		.expect_err("shortU16 must be rejected");
		let message = error.to_string();
		assert!(message.contains("variable-length prefix"));
		assert!(message.contains("declare the field as `u16`"));

		for format in [NumberFormat::F32, NumberFormat::F64] {
			let error = plan(
				&NumberTypeNode::le(format).into(),
				&mut index(Vec::new()),
				"test",
			)
			.expect_err("floats must be rejected");
			assert!(error.to_string().contains("floating-point"));
		}
	}

	#[test]
	fn resolves_aliases_and_registers_structs_and_enums() {
		let mut types = index(vec![
			DefinedTypeNode::new("alias", DefinedTypeLinkNode::new("amounts")),
			DefinedTypeNode::new("amounts", NumberTypeNode::le(NumberFormat::U64)),
			DefinedTypeNode::new(
				"params",
				StructTypeNode::new(vec![StructFieldTypeNode::new(
					"amount",
					NumberTypeNode::le(NumberFormat::U64),
				)]),
			),
		]);

		let alias = plan(
			&DefinedTypeLinkNode::new("alias").into(),
			&mut types,
			"test",
		)
		.unwrap_or_else(|error| panic!("alias should resolve: {error}"));
		assert_eq!(alias.fixed_size, Some(8));
		assert_eq!(alias.rust_type, "u64");

		let params = plan(
			&DefinedTypeLinkNode::new("params").into(),
			&mut types,
			"test",
		)
		.unwrap_or_else(|error| panic!("struct should plan: {error}"));
		assert_eq!(params.rust_type, "Params");
		assert_eq!(params.fixed_size, Some(8));
		assert!(params.encode.contains("encode_into"));
		// Only the struct needs a declaration; the alias resolved to a scalar.
		assert_eq!(types.named(), ["params"]);
	}

	#[test]
	fn reports_undeclared_and_cyclic_links() {
		let error = plan(
			&DefinedTypeLinkNode::new("missing").into(),
			&mut index(Vec::new()),
			"test",
		)
		.expect_err("undeclared links must be rejected");
		assert!(error.to_string().contains("not declared"));

		let mut types = index(vec![
			DefinedTypeNode::new("first", DefinedTypeLinkNode::new("second")),
			DefinedTypeNode::new("second", DefinedTypeLinkNode::new("first")),
		]);
		let error = plan(
			&DefinedTypeLinkNode::new("first").into(),
			&mut types,
			"test",
		)
		.expect_err("cyclic links must be rejected");
		assert!(error.to_string().contains("cyclic"));
	}

	#[test]
	fn encodes_fixed_structs() {
		let structure = StructTypeNode::new(vec![
			StructFieldTypeNode::new("amount", NumberTypeNode::le(NumberFormat::U64)),
			StructFieldTypeNode::new("enabled", BooleanTypeNode::default()),
			StructFieldTypeNode::new("owner", PublicKeyTypeNode::new()),
		]);
		let planned = plan(&structure.into(), &mut index(Vec::new()), "test")
			.unwrap_or_else(|error| panic!("struct should plan: {error}"));

		assert_eq!(planned.fixed_size, Some(41));
		assert_eq!(planned.max_size, 41);
		assert!(planned.encode.contains("offset += 8"));
		assert!(!planned.borrows);
	}

	#[test]
	fn struct_borrow_state_follows_its_fields() {
		let structure = StructTypeNode::new(vec![StructFieldTypeNode::new(
			"uri",
			codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
				StringTypeNode::utf8(),
				NumberTypeNode::le(NumberFormat::U32),
			),
		)]);
		let planned = plan(&structure.into(), &mut index(Vec::new()), "test")
			.unwrap_or_else(|error| panic!("struct should plan: {error}"));

		assert!(planned.borrows);
		assert!(planned.is_variable());
	}

	#[test]
	fn sizes_the_payload_of_mixed_enums() {
		let enumeration = EnumTypeNode::new(vec![
			EnumEmptyVariantTypeNode::new("none").into(),
			EnumStructVariantTypeNode::new(
				"some",
				StructTypeNode::new(vec![StructFieldTypeNode::new(
					"amount",
					NumberTypeNode::le(NumberFormat::U64),
				)]),
			)
			.into(),
		]);
		let planned = plan(&enumeration.into(), &mut index(Vec::new()), "test")
			.unwrap_or_else(|error| panic!("enum should plan: {error}"));

		// Mixed payloads have no single layout, so the enum reports the largest.
		assert!(planned.is_variable());
		assert_eq!(planned.max_size, 9);
	}

	#[test]
	fn plans_prefixed_arrays_as_variable_borrows() {
		let array = ArrayTypeNode::prefixed(
			NumberTypeNode::le(NumberFormat::U64),
			NumberTypeNode::le(NumberFormat::U16),
		);
		let planned = plan(&array.into(), &mut index(Vec::new()), "test")
			.unwrap_or_else(|error| panic!("prefixed array should plan: {error}"));

		assert!(planned.is_variable());
		assert_eq!(planned.rust_type, "&'argument [u64]");
		assert!(planned.borrows);
		assert!(
			planned
				.encode
				.contains("(self_value.len() as u16).to_le_bytes()")
		);
		assert!(planned.encode.contains("InvalidInstructionData"));
	}

	#[test]
	fn rejects_bare_strings_and_bytes() {
		for node in [
			TypeNode::String(StringTypeNode::utf8()),
			TypeNode::Bytes(BytesTypeNode {}),
		] {
			let error = plan(&node, &mut index(Vec::new()), "test")
				.expect_err("unprefixed variable data must be rejected");
			assert!(error.to_string().contains("length prefix"));
		}
	}

	#[test]
	fn rejects_fixed_windows_that_cannot_hold_the_payload() {
		let too_small = FixedSizeTypeNode::new(NumberTypeNode::le(NumberFormat::U32), 2);
		assert!(plan(&too_small.into(), &mut index(Vec::new()), "test").is_err());

		let prefixed = ArrayTypeNode::prefixed(
			NumberTypeNode::le(NumberFormat::U64),
			NumberTypeNode::le(NumberFormat::U8),
		);
		let window = FixedSizeTypeNode::new(prefixed, 4);
		let error = plan(&window.into(), &mut index(Vec::new()), "test")
			.expect_err("an undersized window must be rejected");
		assert!(error.to_string().contains("cannot hold a payload"));
	}

	#[test]
	fn sizes_every_native_integer() {
		for (format, size) in [
			(NumberFormat::U8, 1),
			(NumberFormat::U16, 2),
			(NumberFormat::U32, 4),
			(NumberFormat::U64, 8),
			(NumberFormat::U128, 16),
			(NumberFormat::I8, 1),
			(NumberFormat::I16, 2),
			(NumberFormat::I32, 4),
			(NumberFormat::I64, 8),
			(NumberFormat::I128, 16),
		] {
			let planned = plan(
				&NumberTypeNode::le(format).into(),
				&mut index(Vec::new()),
				"test",
			)
			.unwrap_or_else(|error| panic!("native integer should plan: {error}"));
			assert_eq!(planned.fixed_size, Some(size));
			assert!(!planned.borrows);
		}
	}

	#[test]
	fn rejects_big_endian_numbers_and_prefixes() {
		assert!(
			plan(
				&NumberTypeNode::be(NumberFormat::U16).into(),
				&mut index(Vec::new()),
				"test"
			)
			.is_err()
		);
		let big_endian = ArrayTypeNode::prefixed(
			NumberTypeNode::le(NumberFormat::U8),
			NumberTypeNode::be(NumberFormat::U16),
		);
		assert!(plan(&big_endian.into(), &mut index(Vec::new()), "test").is_err());
	}

	#[test]
	fn plans_size_prefixed_strings_and_byte_slices() {
		let mut types = index(Vec::new());
		for (inner, expected) in [
			(TypeNode::String(StringTypeNode::utf8()), "&'argument str"),
			(TypeNode::Bytes(BytesTypeNode {}), "&'argument [u8]"),
		] {
			let node = codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
				inner,
				NumberTypeNode::le(NumberFormat::U32),
			);
			let planned = plan(&node.into(), &mut types, "test")
				.unwrap_or_else(|error| panic!("prefixed value should plan: {error}"));
			assert_eq!(planned.rust_type, expected);
			assert!(planned.is_variable());
			assert!(planned.borrows);
		}
	}
}
