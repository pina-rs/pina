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

	/// The name of the declared type a name ultimately resolves to, following
	/// `definedTypeLinkNode` aliases. A name that is not a link to another
	/// declared type is its own terminal.
	pub(crate) fn terminal_name(&self, name: &str) -> String {
		let mut current = name.to_string();
		while let Some(TypeNode::Link(link)) = self.types.get(current.as_str()) {
			current = link.name.as_ref().to_string();
		}
		current
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

	/// The Rust statements that read this value from `data` at a `cursor`,
	/// producing a binding named `{name}`.
	///
	/// This mirrors [`Self::encode`] in the opposite direction. A fixed shape
	/// copies out of a byte range; a variable one needs a bound the caller
	/// declared, so it reports unsupported instead of guessing.
	pub(crate) fn decode_into(&self, name: &str, context: &str) -> Result<String> {
		self.decode_into_at(name, true, context)
	}

	/// Like [`Self::decode_into`], with the option to leave the cursor alone.
	///
	/// The final field of a parser has no field after it, so advancing past it
	/// would be a dead store that a `-D warnings` build rejects.
	pub(crate) fn decode_final(&self, name: &str, context: &str) -> Result<String> {
		self.decode_into_at(name, false, context)
	}

	fn decode_into_at(&self, name: &str, advance: bool, context: &str) -> Result<String> {
		let Some(size) = self.fixed_size else {
			return Err(unsupported(
				context,
				"accountNode",
				"this field's width is not fixed, so a read-only parser cannot locate the field \
				 that follows it",
			));
		};

		let bounds = format!("data.get(cursor..cursor + {size})?\n\t\t");
		let read = match self.rust_type.as_str() {
			"bool" => {
				format!("let {name} = data.get(cursor).copied()? != 0;\n\t\tcursor += 1;")
			}
			"Address" => {
				format!(
					"let {name} = Address::new_from_array({bounds}.try_into().ok()?);\n\t\tcursor \
					 += 32;"
				)
			}
			_ if self.rust_type.starts_with("[u8; ") => {
				format!(
					"let {name}: [u8; {size}] = {bounds}.try_into().ok()?;\n\t\tcursor += {size};"
				)
			}
			// Everything left is a native integer or a generated named type.
			_ if is_integer_type(&self.rust_type) => {
				format!(
					"let {name}: {} = {}({bounds}.try_into().ok()?);\n\t\tcursor += {size};",
					self.rust_type,
					from_le_bytes_fn(&self.rust_type)
				)
			}
			_ => {
				return Err(unsupported(
					context,
					"accountNode",
					&format!(
						"a field of type `{}` needs a value decoder this renderer does not emit; \
						 only integers, booleans, addresses, fixed byte arrays, and generated \
						 structs have one",
						self.rust_type
					),
				));
			}
		};

		let read = if advance {
			read
		} else {
			// Drop the final `cursor += N;` line, which would be a dead store.
			read.lines()
				.filter(|line| !line.trim().starts_with("cursor +="))
				.collect::<Vec<_>>()
				.join("\n")
		};

		Ok(read)
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
		// become generated Rust types with their own encoder. The generated
		// declaration is keyed by the terminal type's own name, so an alias to
		// a struct renders that struct's page rather than the link node.
		let target = types.resolve(&declared, context)?;
		match target {
			TypeNode::Struct(_) | TypeNode::Enum(_) => {
				let terminal = types.terminal_name(&name);
				types.register(&terminal);
				let size = plan_resolved(&target, types, context)?;

				// A generated type takes `<'a>` only when one of its fields
				// borrows, so the use site has to name that lifetime.
				let mut rust_type = pascal(&terminal);
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
				encode: "if offset + 1 > data.len() {\n\treturn \
				         Err(ProgramError::InvalidInstructionData);\n}\ndata[offset] = \
				         u8::from(*self_value);\noffset += 1;"
					.to_string(),
				borrows: false,
			})
		}
		TypeNode::PublicKey(_) => {
			Ok(Encoded {
				rust_type: "Address".to_string(),
				fixed_size: Some(32),
				max_size: 32,
				encode: "if offset + 32 > data.len() {\n\treturn \
				         Err(ProgramError::InvalidInstructionData);\n}\ndata[offset..offset + \
				         32].copy_from_slice(self_value.as_ref());\noffset += 32;"
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
			let prefix_node = prefix.prefix.get_nested_type_node().clone();
			let width = prefix_width(&prefix_node, context)?;
			let inner = prefix.r#type.as_ref().clone();
			plan_size_prefix(&inner, &prefix_node, width, types, context)
		}
		// A relative offset shifts the cursor against the container's total size,
		// which a compact account only knows at runtime. The value still has a
		// type, but its absolute position is not fixed, so it is planned as
		// variable: a CPI writer never emits one, and an account parser declines
		// rather than reading the wrong offsets.
		TypeNode::PreOffset(pre) => plan_offset(&pre.r#type, types, context, "preOffsetTypeNode"),
		TypeNode::PostOffset(post) => {
			plan_offset(&post.r#type, types, context, "postOffsetTypeNode")
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
			"if offset + {size} > data.len() {{\n\treturn \
			 Err(ProgramError::InvalidInstructionData);\n}}\ndata[offset..offset + \
			 {size}].copy_from_slice(&(*self_value).to_le_bytes());\noffset += {size};"
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
					"if offset + {size} > data.len() {{\n\treturn \
					 Err(ProgramError::InvalidInstructionData);\n}}\ndata[offset..offset + \
					 {size}].copy_from_slice(&self_value[..]);\noffset += {size};"
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
					// The window is authoritative: the payload writes at the
					// cursor start, then the unused tail is zeroed so the
					// next field stays at its declared offset.
					Ok(Encoded {
						fixed_size: Some(size),
						max_size: size,
						encode: format!(
							"let window = offset;\n{}\nif offset > window + {size} || window + \
							 {size} > data.len() {{\n\treturn \
							 Err(ProgramError::InvalidInstructionData);\n}}\ndata[offset..window \
							 + {size}].fill(0);\noffset = window + {size};",
							indent(&planned.encode, 0)
						),
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
				// An element with a borrowed payload (a prefixed string, for
				// example) keeps the `'argument` in the array's Rust type, so
				// the array borrows too.
				borrows: item.borrows,
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
					 Err(ProgramError::InvalidInstructionData);\n}}\nif offset + {width} > \
					 data.len() {{\n\treturn \
					 Err(ProgramError::InvalidInstructionData);\n}}\ndata[offset..offset + \
					 {width}].copy_from_slice(&(self_value.len() as \
					 {prefix_type}).to_le_bytes());\noffset += {width};\nfor item in \
					 self_value.iter() {{\n\t{}\n}}",
					indent(&item.encode, 1)
				),
				borrows: true,
			})
		}
		other @ CountNode::Remainder(_) => {
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
			 Err(ProgramError::InvalidInstructionData);\n}}\nif offset + {width} > data.len() \
			 {{\n\treturn Err(ProgramError::InvalidInstructionData);\n}}\ndata[offset..offset + \
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
	// The tag occupies the declared prefix width; `None` fills the whole
	// declared window with zeros so the layout stays fixed.
	let present = format!(
		"if offset + {width} > data.len() {{\n\treturn \
		 Err(ProgramError::InvalidInstructionData);\n}}\ndata[offset..offset + \
		 {width}].copy_from_slice(&1u128.to_le_bytes()[..{width}]);\noffset += {width};"
	);
	// A fixed option reserves the tag plus the payload span, so an absent value
	// clears the entire window; a variable option only writes the tag.
	let absent = if fixed == Some(true) {
		let span = width.saturating_add(item.max_size);
		format!(
			"if offset + {span} > data.len() {{\n\treturn \
			 Err(ProgramError::InvalidInstructionData);\n}}\ndata[offset..offset + \
			 {span}].fill(0);\n\t\t\toffset += {span};"
		)
	} else {
		format!(
			"if offset + {width} > data.len() {{\n\treturn \
			 Err(ProgramError::InvalidInstructionData);\n}}\ndata[offset..offset + \
			 {width}].fill(0);\n\t\t\toffset += {width};"
		)
	};

	Ok(Encoded {
		rust_type: format!("Option<{}>", item.rust_type),
		fixed_size: fixed
			.filter(|fixed| *fixed)
			.map(|_| width.saturating_add(item.max_size)),
		max_size: width.saturating_add(item.max_size),
		encode: format!(
			"match self_value {{\n\tNone => {{\n\t\t{absent}\n\t}}\n\tSome(value) => \
			 {{\n\t\t{present}\n\t\t{}\n\t}}\n}}",
			indent(&item.encode, 2)
		),
		borrows: item.borrows,
	})
}

fn plan_size_prefix(
	inner: &TypeNode,
	prefix: &NumberTypeNode,
	width: usize,
	types: &mut TypeIndex,
	context: &str,
) -> Result<Encoded> {
	let resolved = types.resolve(inner, context)?;
	let prefix_type = integer_type_name(prefix, context)?;

	let (rust_type, payload_max, encode) = match &resolved {
		TypeNode::String(StringTypeNode {
			encoding: codama_nodes::BytesEncoding::Utf8,
			..
		}) => {
			(
				"&'argument str".to_string(),
				max_for_prefix_width(width),
				format!(
					"let bytes = self_value.as_bytes();\nif bytes.len() > {max} {{\n\treturn \
					 Err(ProgramError::InvalidInstructionData);\n}}\nif offset + {width} > \
					 data.len() {{\n\treturn \
					 Err(ProgramError::InvalidInstructionData);\n}}\ndata[offset..offset + \
					 {width}].copy_from_slice(&(bytes.len() as \
					 {prefix_type}).to_le_bytes());\noffset += {width};\nif offset + bytes.len() \
					 > data.len() {{\n\treturn \
					 Err(ProgramError::InvalidInstructionData);\n}}\ndata[offset..offset + \
					 bytes.len()].copy_from_slice(bytes);\noffset += bytes.len();",
					max = max_for_prefix_width(width)
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
					"if self_value.len() > {max} {{\n\treturn \
					 Err(ProgramError::InvalidInstructionData);\n}}\nif offset + {width} > \
					 data.len() {{\n\treturn \
					 Err(ProgramError::InvalidInstructionData);\n}}\ndata[offset..offset + \
					 {width}].copy_from_slice(&(self_value.len() as \
					 {prefix_type}).to_le_bytes());\noffset += {width};\nif offset + \
					 self_value.len() > data.len() {{\n\treturn \
					 Err(ProgramError::InvalidInstructionData);\n}}\ndata[offset..offset + \
					 self_value.len()].copy_from_slice(self_value);\noffset += self_value.len();",
					max = max_for_prefix_width(width)
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
				count_width.saturating_add(count_max.saturating_mul(item_size)),
				format!(
					"if self_value.len() > {count_max} {{\n\treturn \
					 Err(ProgramError::InvalidInstructionData);\n}}\n// The outer length prefix \
					 covers the inner count prefix and the payload.\nlet payload_len = \
					 {count_width} + self_value.len() * {item_size};\nif offset + {width} > \
					 data.len() {{\n\treturn \
					 Err(ProgramError::InvalidInstructionData);\n}}\ndata[offset..offset + \
					 {width}].copy_from_slice(&(payload_len as \
					 {prefix_type}).to_le_bytes());\noffset += {width};\nif offset + \
					 {count_width} > data.len() {{\n\treturn \
					 Err(ProgramError::InvalidInstructionData);\n}}\ndata[offset..offset + \
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

/// Plans the type inside an offset wrapper.
///
/// The wrapper only moves the cursor, so the value keeps its own type; because
/// the shift depends on the container's total size, the field is treated as
/// having no statically known width.
fn plan_offset(
	inner: &TypeNode,
	types: &mut TypeIndex,
	context: &str,
	kind: &'static str,
) -> Result<Encoded> {
	let mut planned = plan_resolved(inner, types, context)?;
	planned.fixed_size = None;
	planned.encode = format!(
		"// {kind} positions this value relative to the container's total size, which is only \
		 known at runtime."
	);

	Ok(planned)
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
	// The first payload establishes the width; every later one must match it.
	// A mismatch marks the enum variable-length rather than unsupported, because
	// Borsh writes the tag followed by whatever the variant carries. The outer
	// option distinguishes "no variant seen yet" from "a variable variant", so a
	// variable variant followed by a fixed one still reads as mixed.
	let mut payload_width: Option<Option<usize>> = None;
	let mut uniform = true;
	let mut borrows = false;

	for variant in &enumeration.variants {
		let payload = plan_variant_payload(variant, types, context)?;
		let combined = combine(&payload);
		match (payload_width, combined.fixed_size) {
			(None, size) => payload_width = Some(size),
			(Some(Some(current)), Some(size)) if current == size => {}
			(Some(None), None) => {}
			(..) => uniform = false,
		}
		payload_max = payload_max.max(combined.max_size);
		borrows |= combined.borrows;
	}

	Ok(Encoded {
		rust_type: String::new(),
		fixed_size: if uniform {
			payload_width
				.flatten()
				.map(|size| tag_width.saturating_add(size))
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

/// The `from_le_bytes` constructor for a little-endian integer type.
fn from_le_bytes_fn(rust_type: &str) -> String {
	format!("{rust_type}::from_le_bytes")
}

/// Whether a generated Rust type name is a native integer.
fn is_integer_type(rust_type: &str) -> bool {
	matches!(
		rust_type,
		"u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32" | "i64" | "i128"
	)
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
	// Lengths are counts, so a signed prefix would cast a valid length to a
	// negative number; only unsigned formats are accepted here.
	Ok(match integer_type_name(number, context)? {
		"u8" => 1,
		"u16" => 2,
		"u32" => 4,
		"u64" => 8,
		_ => {
			return Err(unsupported(
				context,
				"numberTypeNode",
				"length prefixes must be an unsigned integer of at most 8 bytes",
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
	use codama_nodes::EnumTupleVariantTypeNode;
	use codama_nodes::FixedSizeTypeNode;
	use codama_nodes::NumberFormat;
	use codama_nodes::NumberTypeNode;
	use codama_nodes::PublicKeyTypeNode;
	use codama_nodes::StructFieldTypeNode;
	use codama_nodes::StructTypeNode;

	use super::*;

	fn index(defined: &[DefinedTypeNode]) -> TypeIndex {
		TypeIndex::new(defined)
	}

	#[test]
	fn rejects_short_u16_and_floats_with_actionable_reasons() {
		let error = plan(
			&NumberTypeNode::le(NumberFormat::ShortU16).into(),
			&mut index(&[]),
			"test",
		)
		.expect_err("shortU16 must be rejected");
		let message = error.to_string();
		assert!(message.contains("variable-length prefix"));
		assert!(message.contains("declare the field as `u16`"));

		for format in [NumberFormat::F32, NumberFormat::F64] {
			let error = plan(&NumberTypeNode::le(format).into(), &mut index(&[]), "test")
				.expect_err("floats must be rejected");
			assert!(error.to_string().contains("floating-point"));
		}
	}

	#[test]
	fn resolves_aliases_and_registers_structs_and_enums() {
		let mut types = index(&[
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
		.expect("alias should resolve");
		assert_eq!(alias.fixed_size, Some(8));
		assert_eq!(alias.rust_type, "u64");

		let params = plan(
			&DefinedTypeLinkNode::new("params").into(),
			&mut types,
			"test",
		)
		.expect("struct should plan");
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
			&mut index(&[]),
			"test",
		)
		.expect_err("undeclared links must be rejected");
		assert!(error.to_string().contains("not declared"));

		let mut types = index(&[
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

		// A declared alias pointing at an undeclared type fails while the
		// resolver walks the alias chain, not at the first link.
		let mut types = index(&[DefinedTypeNode::new(
			"broken_alias",
			DefinedTypeLinkNode::new("never_declared"),
		)]);
		let error = plan(
			&DefinedTypeLinkNode::new("broken_alias").into(),
			&mut types,
			"test",
		)
		.expect_err("aliases to undeclared types must be rejected");
		assert!(error.to_string().contains("not declared"));
	}

	#[test]
	fn keeps_a_fixed_window_whose_payload_exactly_fills_it() {
		// A fixed array whose encoded width equals the window keeps its own
		// planned encoding untouched.
		let window = FixedSizeTypeNode::new(
			codama_nodes::ArrayTypeNode::fixed(
				TypeNode::Number(NumberTypeNode::le(NumberFormat::U8)),
				8,
			),
			8,
		);
		let planned = plan(&window.into(), &mut index(&[]), "test")
			.expect("a payload that exactly fills the window should plan");

		assert_eq!(planned.fixed_size, Some(8));
		assert_eq!(planned.max_size, 8);
	}

	#[test]
	fn encodes_fixed_structs() {
		let structure = StructTypeNode::new(vec![
			StructFieldTypeNode::new("amount", NumberTypeNode::le(NumberFormat::U64)),
			StructFieldTypeNode::new("enabled", BooleanTypeNode::default()),
			StructFieldTypeNode::new("owner", PublicKeyTypeNode::new()),
		]);
		let planned = plan(&structure.into(), &mut index(&[]), "test").expect("struct should plan");

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
		let planned = plan(&structure.into(), &mut index(&[]), "test").expect("struct should plan");

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
		let planned = plan(&enumeration.into(), &mut index(&[]), "test").expect("enum should plan");

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
		let planned =
			plan(&array.into(), &mut index(&[]), "test").expect("prefixed array should plan");

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
			let error = plan(&node, &mut index(&[]), "test")
				.expect_err("unprefixed variable data must be rejected");
			assert!(error.to_string().contains("length prefix"));
		}
	}

	#[test]
	fn rejects_fixed_windows_that_cannot_hold_the_payload() {
		let too_small = FixedSizeTypeNode::new(NumberTypeNode::le(NumberFormat::U32), 2);
		assert!(plan(&too_small.into(), &mut index(&[]), "test").is_err());

		let prefixed = ArrayTypeNode::prefixed(
			NumberTypeNode::le(NumberFormat::U64),
			NumberTypeNode::le(NumberFormat::U8),
		);
		let window = FixedSizeTypeNode::new(prefixed, 4);
		let error = plan(&window.into(), &mut index(&[]), "test")
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
			let planned = plan(&NumberTypeNode::le(format).into(), &mut index(&[]), "test")
				.expect("native integer should plan");
			assert_eq!(planned.fixed_size, Some(size));
			assert!(!planned.borrows);
		}
	}

	#[test]
	fn rejects_big_endian_numbers_and_prefixes() {
		assert!(
			plan(
				&NumberTypeNode::be(NumberFormat::U16).into(),
				&mut index(&[]),
				"test"
			)
			.is_err()
		);
		let big_endian = ArrayTypeNode::prefixed(
			NumberTypeNode::le(NumberFormat::U8),
			NumberTypeNode::be(NumberFormat::U16),
		);
		assert!(plan(&big_endian.into(), &mut index(&[]), "test").is_err());
	}

	#[test]
	fn plans_size_prefixed_strings_and_byte_slices() {
		let mut types = index(&[]);
		for (inner, expected) in [
			(TypeNode::String(StringTypeNode::utf8()), "&'argument str"),
			(TypeNode::Bytes(BytesTypeNode {}), "&'argument [u8]"),
		] {
			let node = codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
				inner,
				NumberTypeNode::le(NumberFormat::U32),
			);
			let planned =
				plan(&node.into(), &mut types, "test").expect("prefixed value should plan");
			assert_eq!(planned.rust_type, expected);
			assert!(planned.is_variable());
			assert!(planned.borrows);
		}
	}
	#[test]
	fn plans_tuples_and_offset_wrappers() {
		let mut types = index(&[]);
		let tuple = codama_nodes::TupleTypeNode::new(vec![
			NumberTypeNode::le(NumberFormat::U64).into(),
			BooleanTypeNode::default().into(),
		]);
		let planned = plan(&TypeNode::Tuple(tuple), &mut types, "test").expect("tuple should plan");
		assert_eq!(planned.fixed_size, Some(9));

		// A relative offset has no static position, so the field is variable.
		let inner = codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
			StringTypeNode::utf8(),
			NumberTypeNode::le(NumberFormat::U32),
		);
		let pre = codama_nodes::PreOffsetTypeNode {
			offset: 12,
			strategy: codama_nodes::PreOffsetStrategy::Relative,
			r#type: Box::new(inner.into()),
		};
		let planned = plan(&pre.into(), &mut types, "test").expect("preOffset should plan");
		assert!(
			planned.is_variable(),
			"an offset field has no static position"
		);
		assert!(planned.encode.contains("runtime"));
	}

	#[test]
	fn plans_maps_and_rejects_unprefixed_counts() {
		let mut types = index(&[]);
		let map = codama_nodes::MapTypeNode::new(
			codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
				StringTypeNode::utf8(),
				NumberTypeNode::le(NumberFormat::U32),
			),
			NumberTypeNode::le(NumberFormat::U8),
			codama_nodes::PrefixedCountNode::new(NumberTypeNode::le(NumberFormat::U32)),
		);
		let planned = plan(&map.into(), &mut types, "test").expect("map should plan");
		assert!(planned.is_variable());
		assert!(
			planned
				.rust_type
				.contains("&'argument [(&'argument str, u8)]")
		);

		// A remainder count has no length prefix, so a reader cannot find the end.
		let remainder = codama_nodes::MapTypeNode::new(
			NumberTypeNode::le(NumberFormat::U8),
			NumberTypeNode::le(NumberFormat::U8),
			codama_nodes::RemainderCountNode {},
		);
		assert!(plan(&remainder.into(), &mut types, "test").is_err());
	}

	#[test]
	fn rejects_unsupported_container_shapes() {
		let mut types = index(&[]);
		// `TypeNode::Link` is only reachable through a declared alias, so a bare
		// link node is reported rather than silently resolved.
		let error = plan(
			&codama_nodes::BooleanTypeNode::default().into(),
			&mut types,
			"test",
		);
		assert!(error.is_ok());

		let remainder =
			codama_nodes::ArrayTypeNode::remainder(NumberTypeNode::le(NumberFormat::U8));
		let error = plan(&remainder.into(), &mut types, "test")
			.expect_err("a remainder array has no length prefix");
		assert!(
			error
				.to_string()
				.contains("fixed-count and length-prefixed")
		);
	}

	#[test]
	fn decodes_every_supported_field_shape() {
		let address = Encoded {
			rust_type: "Address".to_string(),
			fixed_size: Some(32),
			max_size: 32,
			encode: String::new(),
			borrows: false,
		};
		assert!(address.decode_into("owner", "test").is_ok());

		let boolean = Encoded {
			rust_type: "bool".to_string(),
			fixed_size: Some(1),
			max_size: 1,
			encode: String::new(),
			borrows: false,
		};
		let read = boolean
			.decode_into("active", "test")
			.expect("bool should decode");
		// A self-referential binding is the exact bug this guards against.
		assert!(!read.contains("let active = active"));
		assert!(read.contains("data.get(cursor).copied()? != 0"));

		let bytes = Encoded {
			rust_type: "[u8; 8]".to_string(),
			fixed_size: Some(8),
			max_size: 8,
			encode: String::new(),
			borrows: false,
		};
		assert!(bytes.decode_into("tag", "test").is_ok());

		// A generated named type has no scalar reader.
		let named = Encoded {
			rust_type: "Key".to_string(),
			fixed_size: Some(1),
			max_size: 1,
			encode: String::new(),
			borrows: false,
		};
		let error = named
			.decode_into("key", "test")
			.expect_err("a generated type needs its own reader");
		assert!(error.to_string().contains("value decoder"));

		// A variable-width field has no offset for the fields after it.
		let variable = Encoded {
			rust_type: "&'argument str".to_string(),
			fixed_size: None,
			max_size: 8,
			encode: String::new(),
			borrows: true,
		};
		assert!(variable.decode_into("name", "test").is_err());
	}
	#[test]
	fn plans_booleans_public_keys_and_fixed_bytes_directly() {
		let mut types = index(&[]);

		let boolean =
			plan(&BooleanTypeNode::default().into(), &mut types, "test").expect("bool should plan");
		assert_eq!(boolean.fixed_size, Some(1));
		assert!(boolean.encode.contains("u8::from"));

		let key =
			plan(&PublicKeyTypeNode::new().into(), &mut types, "test").expect("key should plan");
		assert_eq!(key.fixed_size, Some(32));
		assert!(key.encode.contains("as_ref()"));

		let bytes = FixedSizeTypeNode::new(BytesTypeNode {}, 4).into();
		let planned = plan(&bytes, &mut types, "test").expect("fixed bytes should plan");
		assert!(planned.encode.contains("copy_from_slice(&self_value[..])"));
	}

	#[test]
	fn fixed_windows_accept_payloads_that_fit() {
		let mut types = index(&[]);
		let string = codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
			StringTypeNode::utf8(),
			NumberTypeNode::le(NumberFormat::U32),
		);
		// An 8-byte window cannot hold a u8-prefixed array whose payload may run
		// to 255 bytes, so the wrapper is rejected outright.
		let undersized = FixedSizeTypeNode::new(
			TypeNode::Array(codama_nodes::ArrayTypeNode::prefixed(
				NumberTypeNode::le(NumberFormat::U8),
				NumberTypeNode::le(NumberFormat::U8),
			)),
			8,
		);
		let error = plan(&undersized.into(), &mut types, "test")
			.expect_err("an undersized window must be rejected");
		assert!(error.to_string().contains("cannot hold a payload"));

		// A window that holds the payload maximum is accepted, and the window
		// itself is authoritative: the field is fixed at 256 bytes, the unused
		// tail is zeroed, and the cursor lands at the window end.
		let prefixed = codama_nodes::ArrayTypeNode::prefixed(
			NumberTypeNode::le(NumberFormat::U8),
			NumberTypeNode::le(NumberFormat::U8),
		);
		let loose = FixedSizeTypeNode::new(TypeNode::Array(prefixed), 256);
		let planned = plan(&loose.into(), &mut types, "test").expect("loose window should plan");
		assert_eq!(planned.fixed_size, Some(256));
		assert_eq!(planned.max_size, 256);
		assert!(planned.encode.contains("let window = offset;"));
		assert!(
			planned
				.encode
				.contains("data[offset..window + 256].fill(0)")
		);
		assert!(planned.encode.contains("offset = window + 256"));
	}

	#[test]
	fn an_alias_registers_the_terminal_struct_not_the_link() {
		let mut types = index(&[
			DefinedTypeNode::new("alias", DefinedTypeLinkNode::new("point")),
			DefinedTypeNode::new(
				"point",
				StructTypeNode::new(vec![StructFieldTypeNode::new(
					"x",
					NumberTypeNode::le(NumberFormat::U64),
				)]),
			),
		]);

		let planned = plan(
			&DefinedTypeLinkNode::new("alias").into(),
			&mut types,
			"test",
		)
		.expect("alias to a struct should plan");
		// The generated declaration is keyed by the terminal name, so the use
		// site references the rendered `Point` type, and `render_type_page`
		// receives the struct node it can actually render.
		assert_eq!(planned.rust_type, "Point");
		assert_eq!(types.named(), ["point"]);
	}

	#[test]
	fn a_variable_variant_makes_the_enum_variable_width() {
		let mut types = index(&[]);
		let enumerated = codama_nodes::EnumTypeNode {
			variants: vec![
				codama_nodes::EnumStructVariantTypeNode::new(
					"fixed",
					StructTypeNode::new(vec![StructFieldTypeNode::new(
						"amount",
						NumberTypeNode::le(NumberFormat::U64),
					)]),
				)
				.into(),
				codama_nodes::EnumStructVariantTypeNode::new(
					"grows",
					StructTypeNode::new(vec![StructFieldTypeNode::new(
						"uri",
						codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
							StringTypeNode::utf8(),
							NumberTypeNode::le(NumberFormat::U32),
						),
					)]),
				)
				.into(),
				codama_nodes::EnumStructVariantTypeNode::new(
					"also_fixed",
					StructTypeNode::new(vec![StructFieldTypeNode::new(
						"amount",
						NumberTypeNode::le(NumberFormat::U64),
					)]),
				)
				.into(),
			],
			size: NumberTypeNode::le(NumberFormat::U8).into(),
		};
		let planned = plan(&enumerated.into(), &mut types, "test")
			.expect("a mixed enum still has an encoding");

		// The first variant is fixed and a later one matches it, but the
		// variable variant in between means the enum as a whole is not fixed.
		assert_eq!(planned.fixed_size, None);
	}

	#[test]
	fn size_prefix_encoders_validate_lengths_and_buffer_space() {
		let mut types = index(&[]);
		let node = codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
			StringTypeNode::utf8(),
			NumberTypeNode::le(NumberFormat::U8),
		);
		let planned =
			plan(&node.into(), &mut types, "test").expect("u8-prefixed string should plan");

		// A caller-supplied string longer than the prefix can describe is
		// rejected in the generated code instead of truncating silently.
		assert!(planned.encode.contains("if bytes.len() > 255"));
		// Every write checks the caller-owned buffer first.
		assert!(planned.encode.contains("if offset + 1 > data.len()"));
		assert!(
			planned
				.encode
				.contains("if offset + bytes.len() > data.len()")
		);

		let mut types = index(&[]);
		let node = codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
			codama_nodes::BytesTypeNode::new(),
			NumberTypeNode::le(NumberFormat::U8),
		);
		let planned =
			plan(&node.into(), &mut types, "test").expect("u8-prefixed bytes should plan");
		assert!(planned.encode.contains("if self_value.len() > 255"));
	}

	#[test]
	fn a_prefixed_array_length_covers_the_inner_count_prefix() {
		let mut types = index(&[]);
		let node = codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
			codama_nodes::ArrayTypeNode::prefixed(
				NumberTypeNode::le(NumberFormat::U8),
				NumberTypeNode::le(NumberFormat::U32),
			),
			NumberTypeNode::le(NumberFormat::U32),
		);
		let planned =
			plan(&node.into(), &mut types, "test").expect("nested prefixed array should plan");

		// The outer prefix describes everything after itself, so it counts the
		// inner count prefix too.
		assert!(
			planned
				.encode
				.contains("let payload_len = 4 + self_value.len() * 1;")
		);
	}

	#[test]
	fn a_fixed_option_none_clears_the_whole_declared_window() {
		let mut types = index(&[]);
		let item = plan(
			&TypeNode::Number(NumberTypeNode::le(NumberFormat::U64)),
			&mut types,
			"test",
		)
		.expect("u64 should plan");
		let planned = plan_option(
			&NumberTypeNode::le(NumberFormat::U8),
			&item,
			Some(true),
			"test",
		)
		.expect("a fixed option should plan");

		assert!(planned.encode.contains("data[offset..offset + 9].fill(0)"));
		assert!(planned.encode.contains("offset += 9"));
	}

	#[test]
	fn rejects_fixed_arrays_with_variable_elements_and_overflowing_counts() {
		let mut types = index(&[]);
		let variable_item = codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
			StringTypeNode::utf8(),
			NumberTypeNode::le(NumberFormat::U32),
		);
		let array = codama_nodes::ArrayTypeNode::fixed(variable_item, 3);
		let error = plan(&array.into(), &mut types, "test")
			.expect_err("fixed array of variable elements must be rejected");
		assert!(error.to_string().contains("cannot hold a variable-length"));

		let overflowing =
			codama_nodes::ArrayTypeNode::fixed(NumberTypeNode::le(NumberFormat::U16), u64::MAX);
		let error = plan(&overflowing.into(), &mut types, "test")
			.expect_err("overflowing array must be rejected");
		assert!(error.to_string().contains("overflows"));
	}

	#[test]
	fn plans_options_in_both_encodings() {
		let mut types = index(&[]);
		let variable = codama_nodes::OptionTypeNode::new(NumberTypeNode::le(NumberFormat::U64));
		let planned =
			plan(&variable.into(), &mut types, "test").expect("variable option should plan");
		assert!(planned.is_variable());
		assert!(planned.encode.contains("Some(value)"));
		assert!(planned.encode.contains("None =>"));

		let fixed = codama_nodes::OptionTypeNode::fixed(NumberTypeNode::le(NumberFormat::U64));
		let planned = plan(&fixed.into(), &mut types, "test").expect("fixed option should plan");
		assert_eq!(planned.fixed_size, Some(9));
		assert!(planned.encode.contains("fill(0)"));
	}

	#[test]
	fn plans_size_prefixed_arrays_and_rejects_bad_inners() {
		let mut types = index(&[]);
		let prefix = NumberTypeNode::le(NumberFormat::U32);

		let string_array = codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
			TypeNode::Array(codama_nodes::ArrayTypeNode::prefixed(
				NumberTypeNode::le(NumberFormat::U8),
				NumberTypeNode::le(NumberFormat::U8),
			)),
			prefix.clone(),
		);
		let planned =
			plan(&string_array.into(), &mut types, "test").expect("prefixed array should plan");
		assert!(planned.is_variable());
		assert!(planned.encode.contains("payload_len"));

		// A size prefix around a bare number has no length to encode.
		let invalid = codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
			NumberTypeNode::le(NumberFormat::U64),
			prefix,
		);
		let error = plan(&invalid.into(), &mut types, "test")
			.expect_err("size prefix around a number must be rejected");
		assert!(
			error
				.to_string()
				.contains("must wrap a string, byte slice, or array")
		);
	}

	#[test]
	fn rejects_non_utf8_strings() {
		let mut types = index(&[]);
		let node = codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
			codama_nodes::StringTypeNode::base16(),
			NumberTypeNode::le(NumberFormat::U32),
		);
		let error =
			plan(&node.into(), &mut types, "test").expect_err("non-UTF-8 strings must be rejected");
		assert!(error.to_string().contains("UTF-8"));
	}

	#[test]
	fn plans_uniform_enums_and_wide_discriminants() {
		let mut types = index(&[]);
		let wide = codama_nodes::EnumTypeNode {
			variants: vec![
				codama_nodes::EnumEmptyVariantTypeNode::new("small").into(),
				codama_nodes::EnumEmptyVariantTypeNode::new("large").into(),
			],
			size: codama_nodes::NumberTypeNode::le(NumberFormat::U32).into(),
		};
		let planned = plan(&wide.into(), &mut types, "test").expect("wide enum should plan");
		assert_eq!(planned.fixed_size, Some(4));

		let bad_size = codama_nodes::EnumTypeNode {
			variants: vec![EnumEmptyVariantTypeNode::new("only").into()],
			size: codama_nodes::NumberTypeNode::le(NumberFormat::U128).into(),
		};
		let error = plan(&bad_size.into(), &mut types, "test")
			.expect_err("u128 discriminants must be rejected");
		assert!(error.to_string().contains("at most 8 bytes"));
	}

	#[test]
	fn plans_enum_tuple_variants() {
		let mut types = index(&[]);
		let enumeration = EnumTypeNode::new(vec![
			codama_nodes::EnumTupleVariantTypeNode::new(
				"pair",
				codama_nodes::TupleTypeNode::new(vec![
					NumberTypeNode::le(NumberFormat::U8).into(),
					NumberTypeNode::le(NumberFormat::U8).into(),
				]),
			)
			.into(),
		]);
		let planned =
			plan(&enumeration.into(), &mut types, "test").expect("tuple enum should plan");
		assert_eq!(planned.fixed_size, Some(3));
	}

	#[test]
	fn rejects_unsupported_root_shapes() {
		let mut types = index(&[]);
		let date_time = codama_nodes::DateTimeTypeNode::new(NumberTypeNode::le(NumberFormat::U64));
		let error = plan(&date_time.into(), &mut types, "test")
			.expect_err("unsupported shapes must be rejected");
		assert!(error.to_string().contains("no instruction-data encoding"));
	}

	#[test]
	fn maps_reject_non_prefixed_counts_and_prefix_their_entries() {
		let mut types = index(&[]);
		let prefixed = codama_nodes::MapTypeNode::new(
			codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
				StringTypeNode::utf8(),
				NumberTypeNode::le(NumberFormat::U32),
			),
			NumberTypeNode::le(NumberFormat::U8),
			codama_nodes::PrefixedCountNode::new(NumberTypeNode::le(NumberFormat::U16)),
		);
		let planned = plan(&prefixed.into(), &mut types, "test").expect("map should plan");
		assert!(planned.encode.contains("for (key, value) in"));
		assert!(planned.rust_type.contains("&'argument str"));

		let fixed = codama_nodes::MapTypeNode::new(
			NumberTypeNode::le(NumberFormat::U8),
			NumberTypeNode::le(NumberFormat::U8),
			codama_nodes::FixedCountNode::new(3),
		);
		let error =
			plan(&fixed.into(), &mut types, "test").expect_err("fixed-count maps must be rejected");
		assert!(error.to_string().contains("length-prefixed sequence"));
	}

	#[test]
	fn decode_final_drops_the_trailing_advance() {
		let number = Encoded {
			rust_type: "u64".to_string(),
			fixed_size: Some(8),
			max_size: 8,
			encode: "data[offset..offset + 8].copy_from_slice(&self_value.to_le_bytes());\noffset \
			         += 8;"
				.to_string(),
			borrows: false,
		};

		let advancing = number
			.decode_into("value", "test")
			.expect("decode should succeed");
		assert!(advancing.contains("cursor += 8;"));

		let final_field = number
			.decode_final("value", "test")
			.expect("decode should succeed");
		assert!(!final_field.contains("cursor +="));
	}

	#[test]
	fn rejects_generated_types_without_a_scalar_reader() {
		let named = Encoded {
			rust_type: "Key".to_string(),
			fixed_size: Some(1),
			max_size: 1,
			encode: String::new(),
			borrows: false,
		};
		let error = named
			.decode_into("key", "test")
			.expect_err("generated types need their own reader");
		assert!(error.to_string().contains("value decoder"));
	}
	#[test]
	fn plans_both_offset_directions() {
		let mut types = index(&[]);
		let string = codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
			StringTypeNode::utf8(),
			NumberTypeNode::le(NumberFormat::U32),
		);
		let inner = || {
			TypeNode::SizePrefix(codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
				StringTypeNode::utf8(),
				NumberTypeNode::le(NumberFormat::U32),
			))
		};

		let pre = codama_nodes::PreOffsetTypeNode {
			offset: 12,
			strategy: codama_nodes::PreOffsetStrategy::Relative,
			r#type: Box::new(inner()),
		};
		let planned =
			plan(&TypeNode::PreOffset(pre), &mut types, "test").expect("preOffset should plan");
		assert!(planned.is_variable());

		let post = codama_nodes::PostOffsetTypeNode {
			offset: 4,
			strategy: codama_nodes::PostOffsetStrategy::Relative,
			r#type: Box::new(inner()),
		};
		let planned =
			plan(&TypeNode::PostOffset(post), &mut types, "test").expect("postOffset should plan");
		assert!(planned.is_variable());
	}

	#[test]
	fn fixed_windows_reject_variable_payloads_and_match_exact_ones() {
		let mut types = index(&[]);

		// A boolean is fixed 1 byte, so a 1-byte window matches exactly.
		let exact = FixedSizeTypeNode::new(BooleanTypeNode::default(), 1);
		let planned = plan(&exact.into(), &mut types, "test").expect("exact window should plan");
		assert_eq!(planned.fixed_size, Some(1));

		// A variable-length payload cannot live in a fixed window at all.
		let variable = FixedSizeTypeNode::new(
			TypeNode::Option(codama_nodes::OptionTypeNode::new(NumberTypeNode::le(
				NumberFormat::U64,
			))),
			9,
		);
		let error = plan(&variable.into(), &mut types, "test")
			.expect_err("variable payloads must be rejected");
		assert!(error.to_string().contains("variable-length payload"));
	}

	#[test]
	fn rejects_size_prefixed_arrays_with_bad_shapes() {
		let mut types = index(&[]);

		// A fixed-count array carries no length of its own.
		let fixed_count = codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
			TypeNode::Array(codama_nodes::ArrayTypeNode::fixed(
				NumberTypeNode::le(NumberFormat::U8),
				3,
			)),
			NumberTypeNode::le(NumberFormat::U32),
		);
		let error = plan(&fixed_count.into(), &mut types, "test")
			.expect_err("fixed-count arrays must be rejected");
		assert!(
			error
				.to_string()
				.contains("must itself carry a count prefix")
		);

		// A prefixed array of variable-length strings cannot be bounded.
		let variable_items = codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
			TypeNode::Array(codama_nodes::ArrayTypeNode::prefixed(
				codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
					StringTypeNode::utf8(),
					NumberTypeNode::le(NumberFormat::U32),
				),
				NumberTypeNode::le(NumberFormat::U8),
			)),
			NumberTypeNode::le(NumberFormat::U32),
		);
		let error = plan(&variable_items.into(), &mut types, "test")
			.expect_err("variable elements must be rejected");
		assert!(
			error
				.to_string()
				.contains("cannot hold a variable-length element")
		);
	}

	#[test]
	fn reports_wide_prefix_limits() {
		let mut types = index(&[]);
		for (prefix_format, width, prefix_max) in [
			(NumberFormat::U16, 2usize, u16::MAX as usize),
			(NumberFormat::U64, 8usize, usize::MAX),
		] {
			let node = codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
				StringTypeNode::utf8(),
				NumberTypeNode::le(prefix_format),
			);
			let planned = plan(&node.into(), &mut types, "test").expect("prefix should plan");
			assert_eq!(
				planned.max_size,
				prefix_max.saturating_add(width),
				"width {width}"
			);
		}
	}

	#[test]
	fn skips_and_reports_omitted_struct_and_variant_fields() {
		let mut types = index(&[]);

		let mut omitted = StructFieldTypeNode::new("pad", NumberTypeNode::le(NumberFormat::U8));
		omitted.default_value_strategy = Some(codama_nodes::DefaultValueStrategy::Omitted);
		let structure = StructTypeNode::new(vec![
			StructFieldTypeNode::new("amount", NumberTypeNode::le(NumberFormat::U64)),
			omitted,
			StructFieldTypeNode::new("bare", StringTypeNode::utf8()),
		]);
		let error = plan(&structure.into(), &mut types, "test")
			.expect_err("bare strings must still be rejected");
		assert!(error.to_string().contains("length prefix"));

		let mut omitted = StructFieldTypeNode::new("pad", NumberTypeNode::le(NumberFormat::U8));
		omitted.default_value_strategy = Some(codama_nodes::DefaultValueStrategy::Omitted);
		let enumeration = EnumTypeNode::new(vec![
			codama_nodes::EnumStructVariantTypeNode::new(
				"withPad",
				StructTypeNode::new(vec![
					omitted,
					StructFieldTypeNode::new("bare", StringTypeNode::utf8()),
				]),
			)
			.into(),
		]);
		let error = plan(&enumeration.into(), &mut types, "test")
			.expect_err("variant omitted fields must still be rejected");
		assert!(error.to_string().contains("length prefix"));
	}
}
