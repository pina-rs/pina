use codama_nodes::ArrayTypeNode;
use codama_nodes::BooleanTypeNode;
use codama_nodes::BytesTypeNode;
use codama_nodes::CountNode;
use codama_nodes::DefinedTypeLinkNode;
use codama_nodes::FixedSizeTypeNode;
use codama_nodes::NestedTypeNode;
use codama_nodes::NestedTypeNodeTrait;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::OptionTypeNode;
use codama_nodes::PostOffsetTypeNode;
use codama_nodes::PreOffsetTypeNode;
use codama_nodes::PublicKeyTypeNode;
use codama_nodes::SizePrefixTypeNode;
use codama_nodes::StringTypeNode;
use codama_nodes::TypeNode;
use quote::ToTokens;

use crate::ir::PinaPodEnumIr;

/// Fallible type mapping used by IDL generation.
///
/// Unsupported Pod collection layouts are rejected rather than silently
/// emitted as public keys with an incorrect wire size.
pub fn try_rust_type_to_codama(ty: &str) -> Result<TypeNode, String> {
	try_rust_type_to_codama_with_pinapod_enums(ty, &[])
}

/// Fallible type mapping with the local `PinaPod` enum registry.
pub fn try_rust_type_to_codama_with_pinapod_enums(
	ty: &str,
	pinapod_enums: &[PinaPodEnumIr],
) -> Result<TypeNode, String> {
	match ty {
		"u8" => Ok(NumberTypeNode::le(NumberFormat::U8).into()),
		"u16" | "PodU16" => Ok(NumberTypeNode::le(NumberFormat::U16).into()),
		"u32" | "PodU32" => Ok(NumberTypeNode::le(NumberFormat::U32).into()),
		"u64" | "PodU64" => Ok(NumberTypeNode::le(NumberFormat::U64).into()),
		"u128" | "PodU128" => Ok(NumberTypeNode::le(NumberFormat::U128).into()),
		"i8" => Ok(NumberTypeNode::le(NumberFormat::I8).into()),
		"i16" | "PodI16" => Ok(NumberTypeNode::le(NumberFormat::I16).into()),
		"i32" | "PodI32" => Ok(NumberTypeNode::le(NumberFormat::I32).into()),
		"i64" | "PodI64" => Ok(NumberTypeNode::le(NumberFormat::I64).into()),
		"i128" | "PodI128" => Ok(NumberTypeNode::le(NumberFormat::I128).into()),
		"PodBool" | "bool" => Ok(BooleanTypeNode::default().into()),
		"Address" | "Pubkey" => Ok(PublicKeyTypeNode::new().into()),
		_ => {
			if pinapod_enums.iter().any(|item| item.name == ty) {
				return Ok(DefinedTypeLinkNode::new(ty).into());
			}
			// Handle fixed-size byte arrays like [u8; 32]
			if let Some(size) = parse_byte_array(ty) {
				Ok(FixedSizeTypeNode::<TypeNode>::new(BytesTypeNode::new(), size).into())
			} else if let Some(node) = parse_pod_collection(ty, pinapod_enums)? {
				Ok(node)
			} else {
				Err(format!(
					"unknown fixed-layout type `{ty}`; only Address and Pubkey map to public keys"
				))
			}
		}
	}
}

/// Map a trailing collection in a compact account without fixed-size padding.
/// The declared capacity remains an on-chain validation bound; the
/// Codama node describes the active prefixed elements present on the wire.
pub fn try_rust_type_to_codama_compact_tail(
	ty: &str,
	context: &str,
	pinapod_enums: &[PinaPodEnumIr],
) -> Result<TypeNode, crate::error::IdlError> {
	try_rust_type_to_codama_compact_tail_at(ty, context, pinapod_enums, None, 0)
}

/// The supported dynamic suffix grammar for a compact `PinaPod` account.
///
/// An optional dynamic tail keeps only its one-byte presence tag in the
/// shared header. Its inner string/vector prefix is part of the payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CompactTailSchema {
	String {
		capacity: usize,
		prefix_size: usize,
	},
	Vec {
		capacity: usize,
		item_ty: String,
		prefix_size: usize,
	},
	OptionString {
		capacity: usize,
		prefix_size: usize,
	},
	OptionVec {
		capacity: usize,
		item_ty: String,
		prefix_size: usize,
	},
}

impl CompactTailSchema {
	pub(crate) fn capacity(&self) -> usize {
		match self {
			Self::String { capacity, .. }
			| Self::Vec { capacity, .. }
			| Self::OptionString { capacity, .. }
			| Self::OptionVec { capacity, .. } => *capacity,
		}
	}

	pub(crate) fn header_size(&self) -> usize {
		match self {
			Self::String { prefix_size, .. } | Self::Vec { prefix_size, .. } => *prefix_size,
			Self::OptionString { .. } | Self::OptionVec { .. } => 1,
		}
	}
}

/// Parse a compact field without guessing at malformed dynamic spellings.
///
/// `Ok(None)` means the field has a fixed representation and belongs in the
/// inline header. Recognized but unsupported dynamic nesting is an error.
pub(crate) fn compact_tail_schema(ty: &str) -> Result<Option<CompactTailSchema>, String> {
	let Some((name, args)) = parse_generic_args(ty) else {
		if matches!(ty, "String" | "PodString" | "Vec" | "PodVec" | "Option") {
			return Err(compact_tail_grammar_error(ty));
		}
		return Ok(None);
	};

	match name.as_str() {
		"String" | "PodString" => parse_compact_string_schema(ty, &args).map(Some),
		"Vec" | "PodVec" => parse_compact_vec_schema(ty, &args).map(Some),
		"Option" => {
			if args.len() != 1 {
				return Err(format!(
					"`{ty}` requires exactly one type argument in a compact account"
				));
			}
			let inner_ty = &args[0];
			let Some(inner) = compact_tail_schema(inner_ty)? else {
				return Ok(None);
			};
			match inner {
				CompactTailSchema::String {
					capacity,
					prefix_size,
				} => {
					Ok(Some(CompactTailSchema::OptionString {
						capacity,
						prefix_size,
					}))
				}
				CompactTailSchema::Vec {
					capacity,
					item_ty,
					prefix_size,
				} if !is_dynamic_type_name(&item_ty) => {
					Ok(Some(CompactTailSchema::OptionVec {
						capacity,
						item_ty,
						prefix_size,
					}))
				}
				CompactTailSchema::Vec { item_ty, .. } => {
					Err(format!(
						"unsupported dynamic nesting `{ty}`: `Option<Vec<String<M>, N>>` is not \
						 supported; vector element `{item_ty}` is dynamic. {}",
						compact_tail_supported_variants(),
					))
				}
				CompactTailSchema::OptionString { .. } | CompactTailSchema::OptionVec { .. } => {
					Err(format!(
						"unsupported dynamic nesting `{ty}`: nested dynamic options are not \
						 supported. {}",
						compact_tail_supported_variants(),
					))
				}
			}
		}
		_ => Ok(None),
	}
}

fn parse_compact_string_schema(ty: &str, args: &[String]) -> Result<CompactTailSchema, String> {
	if !(1..=2).contains(&args.len()) {
		return Err(format!(
			"`{ty}` requires a capacity and an optional 1, 2, 4, or 8-byte prefix"
		));
	}
	let capacity = parse_collection_size(args.first(), ty, "capacity")?;
	let prefix_size = match args.get(1) {
		Some(value) => parse_collection_size(Some(value), ty, "prefix size")?,
		None => 1,
	};
	validate_prefix_size(prefix_size, ty)?;
	validate_collection_capacity(capacity, prefix_size, ty)?;
	Ok(CompactTailSchema::String {
		capacity,
		prefix_size,
	})
}

fn parse_compact_vec_schema(ty: &str, args: &[String]) -> Result<CompactTailSchema, String> {
	if !(2..=3).contains(&args.len()) {
		return Err(format!(
			"`{ty}` requires an element type, capacity, and optional 1, 2, 4, or 8-byte prefix"
		));
	}
	let item_ty = args[0].clone();
	if let Some(item_schema) = compact_tail_schema(&item_ty)?
		&& !matches!(item_schema, CompactTailSchema::String { .. })
	{
		return Err(format!(
			"unsupported dynamic compact vector element `{item_ty}` in `{ty}`. {}",
			compact_tail_supported_variants(),
		));
	}
	let capacity = parse_collection_size(args.get(1), ty, "capacity")?;
	let prefix_size = match args.get(2) {
		Some(value) => parse_collection_size(Some(value), ty, "prefix size")?,
		None => 2,
	};
	validate_prefix_size(prefix_size, ty)?;
	validate_collection_capacity(capacity, prefix_size, ty)?;
	Ok(CompactTailSchema::Vec {
		capacity,
		item_ty,
		prefix_size,
	})
}

fn is_dynamic_type_name(ty: &str) -> bool {
	parse_generic_args(ty).is_some_and(|(name, _)| {
		matches!(
			name.as_str(),
			"String" | "PodString" | "Vec" | "PodVec" | "Option"
		)
	})
}

fn compact_tail_supported_variants() -> &'static str {
	"Supported compact fields are `String<N>`, `Vec<T, N>` for fixed `T`, `Option<T>` for fixed \
	 `T`, `Option<String<N>>`, `Option<Vec<T, N>>` for fixed `T`, and `Vec<String<M>, N>`."
}

fn compact_tail_grammar_error(ty: &str) -> String {
	format!(
		"invalid compact field `{ty}`. {}",
		compact_tail_supported_variants()
	)
}

/// Map a compact tail whose prefix is stored in the shared compact header.
///
/// `prefix_offset` is absolute from the start of the account. `payload_skip`
/// advances the first tail over the remaining header prefixes before its
/// payload is decoded. Nested pre/post offsets make the count read restore the
/// payload cursor, matching pinapod's header-then-payload layout.
pub(crate) fn try_rust_type_to_codama_compact_tail_at(
	ty: &str,
	context: &str,
	pinapod_enums: &[PinaPodEnumIr],
	prefix_offset: Option<usize>,
	payload_skip: usize,
) -> Result<TypeNode, crate::error::IdlError> {
	let error = |reason| {
		crate::error::IdlError::UnsupportedType {
			ty: ty.to_owned(),
			context: context.to_owned(),
			reason,
		}
	};
	let schema = compact_tail_schema(ty)
		.map_err(&error)?
		.ok_or_else(|| error(compact_tail_grammar_error(ty)))?;
	let prefix_offset = |prefix: NumberTypeNode| -> Result<_, crate::error::IdlError> {
		if let Some(prefix_offset) = prefix_offset {
			let offset = i32::try_from(prefix_offset).map_err(|_| {
				error("compact header offset exceeds Codama's i32 range".to_string())
			})?;
			let prefix =
				PreOffsetTypeNode::<NestedTypeNode<NumberTypeNode>>::absolute(prefix, offset);
			Ok(PostOffsetTypeNode::<NestedTypeNode<NumberTypeNode>>::pre_offset(prefix, 0).into())
		} else {
			Ok(prefix.into())
		}
	};
	let dynamic_payload = |item_ty: &str,
	                       prefix_size: usize,
	                       is_string: bool|
	 -> Result<TypeNode, crate::error::IdlError> {
		let prefix = prefix_number_type(prefix_size, ty).map_err(&error)?;
		if is_string {
			Ok(SizePrefixTypeNode::<TypeNode>::new(StringTypeNode::utf8(), prefix).into())
		} else {
			if !is_known_fixed_size_type(item_ty, pinapod_enums) {
				return Err(error(format!(
					"cannot determine compact `Vec` element size for `{item_ty}`"
				)));
			}
			let item = try_rust_type_to_codama_with_pinapod_enums(item_ty, pinapod_enums)
				.map_err(&error)?;
			Ok(ArrayTypeNode::prefixed(item, prefix).into())
		}
	};

	let dynamic: TypeNode = match &schema {
		CompactTailSchema::String { prefix_size, .. } => {
			let prefix = prefix_offset(prefix_number_type(*prefix_size, ty).map_err(&error)?)?;
			SizePrefixTypeNode::<TypeNode>::new(StringTypeNode::utf8(), prefix).into()
		}
		CompactTailSchema::Vec {
			item_ty,
			prefix_size,
			..
		} => {
			if !is_known_fixed_size_type(item_ty, pinapod_enums) {
				return Err(error(format!(
					"cannot determine compact `Vec` element size for `{item_ty}`"
				)));
			}
			let item = try_rust_type_to_codama_with_pinapod_enums(item_ty, pinapod_enums)
				.map_err(&error)?;
			let prefix = prefix_offset(prefix_number_type(*prefix_size, ty).map_err(&error)?)?;
			ArrayTypeNode::prefixed(item, prefix).into()
		}
		CompactTailSchema::OptionString { prefix_size, .. } => {
			let item = dynamic_payload("", *prefix_size, true)?;
			let prefix = prefix_offset(NumberTypeNode::le(NumberFormat::U8))?;
			OptionTypeNode {
				fixed: None,
				item: Box::new(item),
				prefix,
			}
			.into()
		}
		CompactTailSchema::OptionVec {
			item_ty,
			prefix_size,
			..
		} => {
			let item = dynamic_payload(item_ty, *prefix_size, false)?;
			let prefix = prefix_offset(NumberTypeNode::le(NumberFormat::U8))?;
			OptionTypeNode {
				fixed: None,
				item: Box::new(item),
				prefix,
			}
			.into()
		}
	};

	if payload_skip == 0 {
		Ok(dynamic)
	} else {
		let offset = i32::try_from(payload_skip)
			.map_err(|_| error("compact header size exceeds Codama's i32 range".to_string()))?;
		Ok(PreOffsetTypeNode::<TypeNode>::relative(dynamic, offset).into())
	}
}

/// Parse a pinapod collection schema or explicit storage type into a semantic,
/// fixed-size Codama node.
///
/// - `PodString<N, PFX = 1>` maps to a fixed-size, size-prefixed UTF-8 string.
/// - `PodVec<T, N, PFX = 2>` maps to a fixed-size, prefix-counted array.
/// - `Option<T>` maps to a fixed option with pinapod's one-byte tag.
/// - `PodOption<T, PFX = 1>` maps to a fixed option with an explicit tag width.
///
/// Returns `Ok(None)` for non-collection types and an error for collection
/// layouts whose byte size cannot be resolved statically.
fn parse_pod_collection(
	ty: &str,
	pinapod_enums: &[PinaPodEnumIr],
) -> Result<Option<TypeNode>, String> {
	let Some((name, args)) = parse_generic_args(ty) else {
		if ty == "String"
			|| ty.starts_with("String<")
			|| ty == "Vec"
			|| ty.starts_with("Vec<")
			|| ty == "PodString"
			|| ty.starts_with("PodString<")
			|| ty == "PodVec"
			|| ty.starts_with("PodVec<")
			|| ty == "Option"
			|| ty.starts_with("Option<")
			|| ty == "PodOption"
			|| ty.starts_with("PodOption<")
		{
			return Err(format!("unable to parse Pod collection type `{ty}`"));
		}

		return Ok(None);
	};

	match name.as_str() {
		"String" | "PodString" => {
			if !(1..=2).contains(&args.len()) {
				return Err(format!("`{ty}` expects one or two generic arguments"));
			}

			let n = parse_collection_size(args.first(), ty, "capacity")?;
			let pfx: usize = match args.get(1) {
				Some(s) => parse_collection_size(Some(s), ty, "prefix size")?,
				None => 1,
			};
			validate_prefix_size(pfx, ty)?;
			validate_collection_capacity(n, pfx, ty)?;
			let size = n
				.checked_add(pfx)
				.ok_or_else(|| format!("`{ty}` byte size overflows usize"))?;
			let prefix = prefix_number_type(pfx, ty)?;
			let string = SizePrefixTypeNode::<TypeNode>::new(StringTypeNode::utf8(), prefix);
			Ok(Some(
				FixedSizeTypeNode::<TypeNode>::new(string, size).into(),
			))
		}
		"Vec" | "PodVec" => {
			if !(2..=3).contains(&args.len()) {
				return Err(format!("`{ty}` expects two or three generic arguments"));
			}

			let item_ty = args
				.first()
				.ok_or_else(|| format!("`{ty}` is missing its element type"))?;
			if !is_known_fixed_size_type(item_ty, pinapod_enums) {
				return Err(format!(
					"cannot determine the byte size of PodVec element `{item_ty}` in `{ty}`"
				));
			}
			let mut item = try_rust_type_to_codama_with_pinapod_enums(item_ty, pinapod_enums)?;
			let n = parse_collection_size(args.get(1), ty, "capacity")?;
			let pfx: usize = match args.get(2) {
				Some(s) => parse_collection_size(Some(s), ty, "prefix size")?,
				None => 2,
			};
			validate_prefix_size(pfx, ty)?;
			validate_collection_capacity(n, pfx, ty)?;
			// Wire layout: [count: PFX bytes][items: N × T]. Emit the full
			// fixed size (prefix + elements) so generated clients decode the
			// correct account size and field offsets.
			let item_size = pinapod_enums
				.iter()
				.find(|item| item.name == *item_ty)
				.map(|item| item.repr_size)
				.or_else(|| type_node_size(&item))
				.ok_or_else(|| format!("cannot determine the byte size of `{item_ty}`"))?;
			if matches!(item, TypeNode::Link(_)) {
				item = FixedSizeTypeNode::<TypeNode>::new(item, item_size).into();
			}
			let size = n
				.checked_mul(item_size)
				.and_then(|size| size.checked_add(pfx))
				.ok_or_else(|| format!("`{ty}` byte size overflows usize"))?;
			let prefix = prefix_number_type(pfx, ty)?;
			let array = ArrayTypeNode::prefixed(item, prefix);
			Ok(Some(FixedSizeTypeNode::<TypeNode>::new(array, size).into()))
		}
		"Option" | "PodOption" => {
			let valid_argument_count = if name == "Option" {
				args.len() == 1
			} else {
				(1..=2).contains(&args.len())
			};
			if !valid_argument_count {
				let expected = if name == "Option" {
					"one generic argument"
				} else {
					"one or two generic arguments"
				};
				return Err(format!("`{ty}` expects {expected}"));
			}

			let item_ty = args
				.first()
				.ok_or_else(|| format!("`{ty}` is missing its element type"))?;
			if name == "PodOption" && !is_known_pinapod_storage_type(item_ty) {
				return Err(format!(
					"`{ty}` requires an alignment-one PinaPod storage element; use \
					 `Option<{item_ty}>` for a native schema type"
				));
			}
			if !is_known_fixed_size_type(item_ty, pinapod_enums) {
				return Err(format!(
					"cannot determine the byte size of PodOption element `{item_ty}` in `{ty}`"
				));
			}
			let mut item = try_rust_type_to_codama_with_pinapod_enums(item_ty, pinapod_enums)?;
			let item_size = pinapod_enums
				.iter()
				.find(|item| item.name == *item_ty)
				.map(|item| item.repr_size)
				.or_else(|| type_node_size(&item))
				.ok_or_else(|| format!("cannot determine the byte size of `{item_ty}`"))?;
			if matches!(item, TypeNode::Link(_)) {
				item = FixedSizeTypeNode::<TypeNode>::new(item, item_size).into();
			}

			let pfx = match args.get(1) {
				Some(value) => parse_collection_size(Some(value), ty, "prefix size")?,
				None => 1,
			};
			validate_option_prefix_size(pfx, ty)?;
			pfx.checked_add(item_size)
				.ok_or_else(|| format!("`{ty}` byte size overflows usize"))?;
			let prefix = prefix_number_type(pfx, ty)?;
			Ok(Some(
				OptionTypeNode {
					fixed: Some(true),
					item: Box::new(item),
					prefix: prefix.into(),
				}
				.into(),
			))
		}
		_ => Ok(None),
	}
}

fn is_known_pinapod_storage_type(ty: &str) -> bool {
	matches!(
		ty,
		"u8" | "i8"
			| "PodU16"
			| "PodU32"
			| "PodU64"
			| "PodU128"
			| "PodI16"
			| "PodI32"
			| "PodI64"
			| "PodI128"
			| "PodBool"
			| "Address"
	) || parse_byte_array(ty).is_some()
		|| ty.starts_with("String<")
		|| ty.starts_with("Vec<")
		|| ty.starts_with("PodString<")
		|| ty.starts_with("PodVec<")
		|| ty.starts_with("PodOption<")
}

fn parse_collection_size(value: Option<&String>, ty: &str, name: &str) -> Result<usize, String> {
	value
		.ok_or_else(|| format!("`{ty}` is missing its {name}"))?
		.parse()
		.map_err(|_| format!("`{ty}` requires a literal usize {name}"))
}

fn validate_prefix_size(pfx: usize, ty: &str) -> Result<(), String> {
	if matches!(pfx, 1 | 2 | 4 | 8) {
		Ok(())
	} else {
		Err(format!("`{ty}` has unsupported prefix size {pfx}"))
	}
}

fn validate_option_prefix_size(pfx: usize, ty: &str) -> Result<(), String> {
	if matches!(pfx, 1 | 2 | 4) {
		Ok(())
	} else {
		Err(format!(
			"`{ty}` has unsupported option prefix size {pfx}; pinapod supports 1, 2, or 4 bytes"
		))
	}
}

fn validate_collection_capacity(capacity: usize, pfx: usize, ty: &str) -> Result<(), String> {
	let fits_prefix = match pfx {
		1 => u8::try_from(capacity).is_ok(),
		2 => u16::try_from(capacity).is_ok(),
		4 => u32::try_from(capacity).is_ok(),
		8 => true,
		_ => false,
	};

	if fits_prefix {
		Ok(())
	} else {
		Err(format!(
			"`{ty}` capacity {capacity} cannot be represented by its {pfx}-byte prefix"
		))
	}
}

fn prefix_number_type(pfx: usize, ty: &str) -> Result<NumberTypeNode, String> {
	let format = match pfx {
		1 => NumberFormat::U8,
		2 => NumberFormat::U16,
		4 => NumberFormat::U32,
		8 => NumberFormat::U64,
		_ => return Err(format!("`{ty}` has unsupported prefix size {pfx}")),
	};

	Ok(NumberTypeNode::le(format))
}

fn is_known_fixed_size_type(ty: &str, pinapod_enums: &[PinaPodEnumIr]) -> bool {
	matches!(
		ty,
		"u8" | "u16"
			| "PodU16"
			| "u32" | "PodU32"
			| "u64" | "PodU64"
			| "u128" | "PodU128"
			| "i8" | "i16"
			| "PodI16"
			| "i32" | "PodI32"
			| "i64" | "PodI64"
			| "i128" | "PodI128"
			| "PodBool"
			| "bool" | "Address"
			| "Pubkey"
	) || pinapod_enums.iter().any(|item| item.name == ty)
		|| parse_byte_array(ty).is_some()
		|| ty.starts_with("String<")
		|| ty.starts_with("Vec<")
		|| ty.starts_with("Option<")
		|| ty.starts_with("PodString<")
		|| ty.starts_with("PodVec<")
		|| ty.starts_with("PodOption<")
}

/// Compute the on-chain byte size of a fixed-size Codama type node.
///
/// Returns `None` for variable-size or unsupported nodes.
pub(crate) fn type_node_size(node: &TypeNode) -> Option<usize> {
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
		TypeNode::Option(option) if option.fixed == Some(true) => {
			let prefix = option.prefix.get_nested_type_node();
			number_type_size(prefix).and_then(|prefix_size| {
				type_node_size(&option.item)
					.and_then(|item_size| prefix_size.checked_add(item_size))
			})
		}
		TypeNode::Array(array) => {
			match array.count.as_ref() {
				CountNode::Fixed(count) => {
					type_node_size(&array.item).map(|size| size * count.value as usize)
				}
				_ => None,
			}
		}
		_ => None,
	}
}

fn number_type_size(number: &NumberTypeNode) -> Option<usize> {
	match number.format {
		NumberFormat::U8 | NumberFormat::I8 => Some(1),
		NumberFormat::U16 | NumberFormat::I16 => Some(2),
		NumberFormat::U32 | NumberFormat::I32 => Some(4),
		NumberFormat::U64 | NumberFormat::I64 => Some(8),
		NumberFormat::U128 | NumberFormat::I128 => Some(16),
		NumberFormat::F32 | NumberFormat::F64 | NumberFormat::ShortU16 => None,
	}
}

/// Split a generic type string like `PodVec<PodU64, 8, 2>` into its base name
/// and top-level arguments.
fn parse_generic_args(ty: &str) -> Option<(String, Vec<String>)> {
	let open = ty.find('<')?;
	let close = ty.rfind('>')?;
	if close < open {
		return None;
	}
	let name = ty[..open].trim().to_owned();
	let inner = &ty[open + 1..close];

	let mut args = Vec::new();
	let mut depth = 0usize;
	let mut start = 0usize;
	for (i, c) in inner.char_indices() {
		match c {
			'<' | '(' | '[' => depth += 1,
			'>' | ')' | ']' => depth = depth.saturating_sub(1),
			',' if depth == 0 => {
				args.push(inner[start..i].trim().to_owned());
				start = i + 1;
			}
			_ => {}
		}
	}
	args.push(inner[start..].trim().to_owned());

	if args.iter().any(String::is_empty) {
		return None;
	}
	Some((name, args))
}

/// Render a single generic argument (type, const, or lifetime) as a string.
fn generic_arg_to_string(arg: &syn::GenericArgument) -> String {
	match arg {
		syn::GenericArgument::Type(t) => type_to_string(t),
		syn::GenericArgument::Const(e) => e.to_token_stream().to_string(),
		syn::GenericArgument::Lifetime(lt) => lt.ident.to_string(),
		_ => "unknown".to_owned(),
	}
}

/// Try to parse `[u8; N]` and return `N`.
fn parse_byte_array(ty: &str) -> Option<usize> {
	let ty = ty.trim();
	let inner = ty.strip_prefix('[')?.strip_suffix(']')?;
	let (elem, size) = inner.split_once(';')?;
	if elem.trim() != "u8" {
		return None;
	}
	size.trim().parse().ok()
}

/// Extract the simple type name from a `syn::Type`. Handles paths like
/// `PodU64`, `Address`, `u8`, and arrays like `[u8; 32]`.
pub fn type_to_string(ty: &syn::Type) -> String {
	match ty {
		syn::Type::Path(p) => {
			// Use the last segment (e.g. `PodU64` from `pina::PodU64`),
			// preserving generic arguments (e.g. `PodString<32>`, `PodVec<
			// PodU64, 8>`) so collection types keep their capacity parameters
			// for IDL extraction.
			if let Some(seg) = p.path.segments.last() {
				let mut s = seg.ident.to_string();
				if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
					let inner = args
						.args
						.iter()
						.map(generic_arg_to_string)
						.collect::<Vec<_>>()
						.join(", ");
					s = format!("{s}<{inner}>");
				}
				s
			} else {
				"unknown".to_owned()
			}
		}
		syn::Type::Array(arr) => {
			let elem = type_to_string(&arr.elem);
			let len = match &arr.len {
				syn::Expr::Lit(syn::ExprLit {
					lit: syn::Lit::Int(i),
					..
				}) => i.base10_digits().to_owned(),
				_ => {
					// Non-literal array length; fallback.
					"0".to_owned()
				}
			};
			format!("[{elem}; {len}]")
		}
		_ => "unknown".to_owned(),
	}
}

#[cfg(test)]
mod tests {
	use codama_nodes::NumberFormat;

	use super::*;

	fn mapped(ty: &str) -> TypeNode {
		try_rust_type_to_codama(ty).unwrap_or_else(|error| panic!("failed to map `{ty}`: {error}"))
	}

	#[test]
	fn maps_pod_types() {
		assert_eq!(
			mapped("PodU64"),
			NumberTypeNode::le(NumberFormat::U64).into()
		);
		assert_eq!(mapped("PodBool"), BooleanTypeNode::default().into());
	}

	#[test]
	fn maps_primitives() {
		assert_eq!(mapped("u8"), NumberTypeNode::le(NumberFormat::U8).into());
	}

	#[test]
	fn maps_address() {
		assert_eq!(mapped("Address"), PublicKeyTypeNode::new().into());
	}

	#[test]
	fn maps_byte_array() {
		let ty = mapped("[u8; 32]");
		let expected: TypeNode =
			FixedSizeTypeNode::<TypeNode>::new(BytesTypeNode::new(), 32).into();
		assert_eq!(ty, expected);
	}

	#[test]
	fn maps_pod_string() {
		// PodString<32> = 1 length byte + 32 payload bytes.
		let ty = mapped("PodString<32>");
		let string = SizePrefixTypeNode::<TypeNode>::new(
			StringTypeNode::utf8(),
			NumberTypeNode::le(NumberFormat::U8),
		);
		let expected: TypeNode = FixedSizeTypeNode::<TypeNode>::new(string, 33).into();
		assert_eq!(ty, expected);
	}

	#[test]
	fn maps_pod_string_with_explicit_prefix() {
		// PodString<64, 2> = 2 length bytes + 64 payload bytes.
		let ty = mapped("PodString<64, 2>");
		let string = SizePrefixTypeNode::<TypeNode>::new(
			StringTypeNode::utf8(),
			NumberTypeNode::le(NumberFormat::U16),
		);
		let expected: TypeNode = FixedSizeTypeNode::<TypeNode>::new(string, 66).into();
		assert_eq!(ty, expected);
	}

	#[test]
	fn maps_wide_collection_prefixes_and_nested_elements() {
		for (ty, size) in [("PodString<64, 4>", 68), ("PodString<64, 8>", 72)] {
			let TypeNode::FixedSize(node) = mapped(ty) else {
				panic!("{ty} did not lower to a fixed-size node");
			};
			assert_eq!(node.size, size);
		}

		let TypeNode::FixedSize(node) = mapped("PodVec<PodString<8, 1>, 4, 2>") else {
			panic!("nested PodVec did not lower to a fixed-size node");
		};
		assert_eq!(node.size, 38);
	}

	#[test]
	fn maps_pod_vec() {
		// PodVec<PodU64, 8> = 2 count bytes + 8 × 8-byte elements.
		let ty = mapped("PodVec<PodU64, 8>");
		let array = ArrayTypeNode::prefixed(
			NumberTypeNode::le(NumberFormat::U64),
			NumberTypeNode::le(NumberFormat::U16),
		);
		let expected: TypeNode = FixedSizeTypeNode::<TypeNode>::new(array, 66).into();
		assert_eq!(ty, expected);
	}

	#[test]
	fn maps_pod_vec_with_explicit_prefix() {
		// PodVec<PodU16, 4, 1> = 1 count byte + 4 × 2-byte elements.
		let ty = mapped("PodVec<PodU16, 4, 1>");
		let array = ArrayTypeNode::prefixed(
			NumberTypeNode::le(NumberFormat::U16),
			NumberTypeNode::le(NumberFormat::U8),
		);
		let expected: TypeNode = FixedSizeTypeNode::<TypeNode>::new(array, 9).into();
		assert_eq!(ty, expected);
	}

	#[test]
	fn maps_compact_vec_tails_without_fixed_capacity_padding() {
		for (ty, item, prefix) in [
			("Vec<u64, 64>", NumberFormat::U64, NumberFormat::U16),
			("PodVec<PodU16, 8, 1>", NumberFormat::U16, NumberFormat::U8),
			("Vec<u32, 8, 4>", NumberFormat::U32, NumberFormat::U32),
			("Vec<u8, 8, 8>", NumberFormat::U8, NumberFormat::U64),
		] {
			let mapped = try_rust_type_to_codama_compact_tail(ty, "test account", &[])
				.unwrap_or_else(|error| panic!("failed to map `{ty}`: {error}"));
			assert_eq!(
				mapped,
				ArrayTypeNode::prefixed(NumberTypeNode::le(item), NumberTypeNode::le(prefix),)
					.into(),
				"wrong compact node for `{ty}`"
			);
		}
	}

	#[test]
	fn maps_every_supported_compact_dynamic_shape() {
		for (ty, expected_kind) in [
			("String<8>", "sizePrefixTypeNode"),
			("Vec<u64, 8>", "arrayTypeNode"),
			("Option<String<8>>", "optionTypeNode"),
			("Option<Vec<u64, 8>>", "optionTypeNode"),
			("Vec<String<8>, 4>", "arrayTypeNode"),
		] {
			let mapped = try_rust_type_to_codama_compact_tail(ty, "test account", &[])
				.unwrap_or_else(|error| panic!("failed to map `{ty}`: {error}"));
			let value = serde_json::to_value(mapped)
				.unwrap_or_else(|error| panic!("failed to serialize `{ty}`: {error}"));
			assert_eq!(value["kind"], expected_kind, "wrong node for `{ty}`");
		}

		let option =
			try_rust_type_to_codama_compact_tail("Option<Vec<u64, 8>>", "test account", &[])
				.expect("option vector should map");
		let value = serde_json::to_value(option).expect("option vector should serialize");
		assert_eq!(value["prefix"]["format"], "u8");
		assert_eq!(value["item"]["count"]["prefix"]["format"], "u16");

		let string_vec =
			try_rust_type_to_codama_compact_tail("Vec<String<8>, 4>", "test account", &[])
				.expect("string vector should map");
		let value = serde_json::to_value(string_vec).expect("string vector should serialize");
		assert_eq!(value["item"]["kind"], "fixedSizeTypeNode");
		assert_eq!(value["item"]["size"], 9);
	}

	#[test]
	fn maps_compact_string_tails_without_fixed_capacity_padding() {
		for (ty, prefix) in [
			("String<32>", NumberFormat::U8),
			("PodString<512, 2>", NumberFormat::U16),
			("PodString<32, 4>", NumberFormat::U32),
			("PodString<32, 8>", NumberFormat::U64),
		] {
			let mapped = try_rust_type_to_codama_compact_tail(ty, "test account", &[])
				.unwrap_or_else(|error| panic!("failed to map `{ty}`: {error}"));
			assert_eq!(
				mapped,
				SizePrefixTypeNode::<TypeNode>::new(
					StringTypeNode::utf8(),
					NumberTypeNode::le(prefix),
				)
				.into(),
				"wrong compact node for `{ty}`"
			);
		}
	}

	#[test]
	fn maps_compact_tail_prefixes_into_a_shared_header() {
		let vector = try_rust_type_to_codama_compact_tail_at(
			"Vec<u64, 8>",
			"test account",
			&[],
			Some(38),
			4,
		)
		.unwrap_or_else(|error| panic!("failed to map relocated tail: {error}"));
		let vector = serde_json::to_value(vector)
			.unwrap_or_else(|error| panic!("serialize relocated tail: {error}"));
		assert_eq!(
			vector
				.pointer("/offset")
				.and_then(serde_json::Value::as_i64),
			Some(4)
		);
		assert_eq!(
			vector
				.pointer("/type/count/prefix/type/offset")
				.and_then(serde_json::Value::as_i64),
			Some(38)
		);
		assert_eq!(
			vector
				.pointer("/type/count/prefix/strategy")
				.and_then(serde_json::Value::as_str),
			Some("preOffset")
		);

		let string = try_rust_type_to_codama_compact_tail_at(
			"PodString<512, 2>",
			"test account",
			&[],
			Some(40),
			7,
		)
		.unwrap_or_else(|error| panic!("failed to map relocated string: {error}"));
		let string = serde_json::to_value(string)
			.unwrap_or_else(|error| panic!("serialize relocated string: {error}"));
		assert_eq!(
			string
				.pointer("/offset")
				.and_then(serde_json::Value::as_i64),
			Some(7)
		);
		assert_eq!(
			string
				.pointer("/type/prefix/type/offset")
				.and_then(serde_json::Value::as_i64),
			Some(40)
		);
		assert_eq!(
			string
				.pointer("/type/prefix/strategy")
				.and_then(serde_json::Value::as_str),
			Some("preOffset")
		);
	}

	#[test]
	fn rejects_compact_offsets_outside_codama_range() {
		let too_large = i32::MAX as usize + 1;
		for (ty, prefix, skip, expected) in [
			("Vec<u8, 8>", Some(too_large), 0, "header offset"),
			("Vec<u8, 8>", None, too_large, "header size"),
			("String<8>", Some(too_large), 0, "header offset"),
			("String<8>", None, too_large, "header size"),
		] {
			let error =
				try_rust_type_to_codama_compact_tail_at(ty, "test account", &[], prefix, skip)
					.expect_err("oversized offset must fail")
					.to_string();
			assert!(error.contains(expected), "unexpected error: {error}");
		}
	}

	#[test]
	fn classifies_compact_tail_capacity_and_header_size() {
		for (ty, capacity, header_size) in [
			("String<8>", 8, 1),
			("PodString<16, 2>", 16, 2),
			("Vec<u64, 8>", 8, 2),
			("PodVec<u8, 16, 1>", 16, 1),
			("Option<String<8>>", 8, 1),
			("Option<Vec<u64, 8, 4>>", 8, 1),
			("Vec<String<8>, 4>", 4, 2),
		] {
			let schema = compact_tail_schema(ty)
				.unwrap_or_else(|error| panic!("failed to classify `{ty}`: {error}"))
				.unwrap_or_else(|| panic!("`{ty}` was not recognized as a compact tail"));
			assert_eq!(schema.capacity(), capacity, "wrong capacity for `{ty}`");
			assert_eq!(schema.header_size(), header_size, "wrong header for `{ty}`");
		}

		assert!(
			compact_tail_schema("Option<u64>")
				.expect("fixed option should parse")
				.is_none()
		);
		assert!(
			compact_tail_schema("u64")
				.expect("scalar should parse")
				.is_none()
		);
	}

	#[test]
	fn rejects_invalid_compact_tails() {
		for (ty, expected) in [
			("u64", "invalid compact field"),
			("String<u64, 8>", "literal usize capacity"),
			("Vec<u64>", "requires an element type"),
			("PodString<8, PREFIX>", "literal usize prefix size"),
			("PodString<8, 3>", "unsupported prefix size"),
			("PodString<256, 1>", "cannot be represented"),
			("Vec<MyPod, 8>", "cannot determine compact"),
			("Vec<u64, CAPACITY>", "literal usize capacity"),
			("Vec<u64, 8, PREFIX>", "literal usize prefix size"),
			("Vec<u64, 8, 3>", "unsupported prefix size"),
			("Vec<u64, 256, 1>", "cannot be represented"),
			("Option<Vec<String<8>, 4>>", "Option<Vec<String<M>, N>>"),
			("Option<Option<String<8>>>", "nested dynamic options"),
			(
				"Vec<Vec<u8, 4>, 2>",
				"unsupported dynamic compact vector element",
			),
		] {
			let error = try_rust_type_to_codama_compact_tail(ty, "account `State.values`", &[])
				.expect_err("invalid compact tail must be rejected")
				.to_string();
			assert!(
				error.contains(expected),
				"unexpected error for `{ty}`: {error}"
			);
			assert!(error.contains("State.values"), "missing context: {error}");
		}
	}

	#[test]
	fn maps_pod_vec_with_signed_pod_elements() {
		let ty = mapped("PodVec<PodI32, 8>");
		let array = ArrayTypeNode::prefixed(
			NumberTypeNode::le(NumberFormat::I32),
			NumberTypeNode::le(NumberFormat::U16),
		);
		let expected: TypeNode = FixedSizeTypeNode::<TypeNode>::new(array, 34).into();
		assert_eq!(ty, expected);

		let ty = mapped("PodVec<PodI128, 8>");
		let array = ArrayTypeNode::prefixed(
			NumberTypeNode::le(NumberFormat::I128),
			NumberTypeNode::le(NumberFormat::U16),
		);
		let expected: TypeNode = FixedSizeTypeNode::<TypeNode>::new(array, 130).into();
		assert_eq!(ty, expected);
	}

	#[test]
	fn maps_native_and_explicit_pod_options() {
		let expected_u8: TypeNode = OptionTypeNode {
			fixed: Some(true),
			item: Box::new(NumberTypeNode::le(NumberFormat::U64).into()),
			prefix: NumberTypeNode::le(NumberFormat::U8).into(),
		}
		.into();
		assert_eq!(mapped("Option<u64>"), expected_u8);
		assert_eq!(mapped("PodOption<PodU64>"), expected_u8);

		let expected_u16: TypeNode = OptionTypeNode {
			fixed: Some(true),
			item: Box::new(NumberTypeNode::le(NumberFormat::U64).into()),
			prefix: NumberTypeNode::le(NumberFormat::U16).into(),
		}
		.into();
		assert_eq!(mapped("PodOption<PodU64, 2>"), expected_u16);
	}

	#[test]
	fn maps_nested_pod_options_and_vectors() {
		assert_eq!(type_node_size(&mapped("Option<PodString<8>>")), Some(10));
		assert_eq!(type_node_size(&mapped("PodVec<Option<u16>, 3>")), Some(11));
	}

	#[test]
	fn maps_options_over_local_enums_and_storage_types() {
		let enums = [PinaPodEnumIr {
			name: "Color".to_owned(),
			repr_size: 1,
			variants: Vec::new(),
			docs: Vec::new(),
		}];
		let option = try_rust_type_to_codama_with_pinapod_enums("Option<Color>", &enums)
			.unwrap_or_else(|error| panic!("failed to map enum option: {error}"));
		assert_eq!(type_node_size(&option), Some(2));
		assert!(matches!(option, TypeNode::Option(_)));

		for ty in [
			"PodOption<u8>",
			"PodOption<PodI16>",
			"PodOption<Address>",
			"PodOption<[u8; 4]>",
			"PodOption<String<4>>",
			"PodOption<Vec<u8, 2>>",
			"PodOption<PodString<4>>",
			"PodOption<PodVec<u8, 2>>",
			"PodOption<PodOption<u8>>",
		] {
			mapped(ty);
		}
	}

	#[test]
	fn rejects_pod_collections_with_unresolved_sizes() {
		for ty in [
			"PodString<NAME_LEN>",
			"PodVec<PodU64, { 4 + 4 }>",
			"PodVec<MyPod, 8>",
			"Option",
			"Option<>",
			"Option<u64, u64>",
			"Option<MyPod>",
			"PodOption",
			"PodOption<>",
			"PodOption<u8, 1, 2>",
			"PodOption<u64>",
			"PodOption<PodU64, PodU64>",
			"PodOption<MyPod>",
		] {
			let error = try_rust_type_to_codama(ty)
				.expect_err("unresolved Pod collection sizes must be rejected");
			assert!(error.contains(ty), "unexpected error for {ty}: {error}");
		}
	}

	#[test]
	fn rejects_pod_option_prefixes_pinapod_does_not_support() {
		for ty in [
			"PodOption<PodU64, 0>",
			"PodOption<PodU64, 3>",
			"PodOption<PodU64, 8>",
		] {
			let error = try_rust_type_to_codama(ty)
				.expect_err("unsupported PodOption prefix must be rejected");
			assert!(error.contains("supports 1, 2, or 4 bytes"), "{error}");
		}
	}

	#[test]
	fn rejects_pod_option_layout_size_overflow() {
		let ty = format!("Option<[u8; {}]>", usize::MAX);
		let error = try_rust_type_to_codama(&ty)
			.expect_err("option layout whose total size overflows must be rejected");
		assert!(error.contains("byte size overflows"), "{error}");
	}

	#[test]
	fn computes_all_option_prefix_sizes_without_overflow() {
		for (format, expected) in [
			(NumberFormat::U16, Some(3)),
			(NumberFormat::U32, Some(5)),
			(NumberFormat::U64, Some(9)),
			(NumberFormat::U128, Some(17)),
			(NumberFormat::F32, None),
		] {
			let option: TypeNode = OptionTypeNode {
				fixed: Some(true),
				item: Box::new(NumberTypeNode::le(NumberFormat::U8).into()),
				prefix: NumberTypeNode::le(format).into(),
			}
			.into();
			assert_eq!(type_node_size(&option), expected);
		}
	}

	#[test]
	fn rejects_collection_capacities_that_do_not_fit_the_prefix() {
		for ty in ["PodString<256, 1>", "PodVec<u8, 256, 1>"] {
			let error = try_rust_type_to_codama(ty)
				.expect_err("collection capacity must fit its length prefix");
			assert!(error.contains("cannot be represented"), "{error}");
		}
	}

	#[test]
	fn type_to_string_preserves_generics() {
		let ty: syn::Type = syn::parse_str("PodString<32>").unwrap_or_else(|e| panic!("{e}"));
		assert_eq!(type_to_string(&ty), "PodString<32>");

		let ty: syn::Type =
			syn::parse_str("pina::PodVec<PodU64, 8>").unwrap_or_else(|e| panic!("{e}"));
		assert_eq!(type_to_string(&ty), "PodVec<PodU64, 8>");

		let ty: syn::Type = syn::parse_str("PodU64").unwrap_or_else(|e| panic!("{e}"));
		assert_eq!(type_to_string(&ty), "PodU64");
	}
}
