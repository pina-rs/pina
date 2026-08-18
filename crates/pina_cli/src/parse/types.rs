use codama_nodes::ArrayTypeNode;
use codama_nodes::BooleanTypeNode;
use codama_nodes::BytesTypeNode;
use codama_nodes::CountNode;
use codama_nodes::DefinedTypeLinkNode;
use codama_nodes::FixedSizeTypeNode;
use codama_nodes::NestedTypeNodeTrait;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::OptionTypeNode;
use codama_nodes::PublicKeyTypeNode;
use codama_nodes::SizePrefixTypeNode;
use codama_nodes::StringTypeNode;
use codama_nodes::TypeNode;
use quote::ToTokens;

use crate::ir::ZeroPodEnumIr;

/// Fallible type mapping used by IDL generation.
///
/// Unsupported Pod collection layouts are rejected rather than silently
/// emitted as public keys with an incorrect wire size.
pub fn try_rust_type_to_codama(ty: &str) -> Result<TypeNode, String> {
	try_rust_type_to_codama_with_zeropod_enums(ty, &[])
}

/// Fallible type mapping with the local zeropod enum registry.
pub fn try_rust_type_to_codama_with_zeropod_enums(
	ty: &str,
	zeropod_enums: &[ZeroPodEnumIr],
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
			if zeropod_enums.iter().any(|item| item.name == ty) {
				return Ok(DefinedTypeLinkNode::new(ty).into());
			}
			// Handle fixed-size byte arrays like [u8; 32]
			if let Some(size) = parse_byte_array(ty) {
				Ok(FixedSizeTypeNode::<TypeNode>::new(BytesTypeNode::new(), size).into())
			} else if let Some(node) = parse_pod_collection(ty, zeropod_enums)? {
				Ok(node)
			} else {
				Err(format!(
					"unknown fixed-layout type `{ty}`; only Address and Pubkey map to public keys"
				))
			}
		}
	}
}

/// Parse a zeropod collection schema or explicit storage type into a semantic,
/// fixed-size Codama node.
///
/// - `PodString<N, PFX = 1>` maps to a fixed-size, size-prefixed UTF-8 string.
/// - `PodVec<T, N, PFX = 2>` maps to a fixed-size, prefix-counted array.
/// - `Option<T>` maps to a fixed option with zeropod's one-byte tag.
/// - `PodOption<T, PFX = 1>` maps to a fixed option with an explicit tag width.
///
/// Returns `Ok(None)` for non-collection types and an error for collection
/// layouts whose byte size cannot be resolved statically.
fn parse_pod_collection(
	ty: &str,
	zeropod_enums: &[ZeroPodEnumIr],
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
			if !is_known_fixed_size_type(item_ty, zeropod_enums) {
				return Err(format!(
					"cannot determine the byte size of PodVec element `{item_ty}` in `{ty}`"
				));
			}
			let mut item = try_rust_type_to_codama_with_zeropod_enums(item_ty, zeropod_enums)?;
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
			let item_size = zeropod_enums
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
			if name == "PodOption" && !is_known_zeropod_storage_type(item_ty) {
				return Err(format!(
					"`{ty}` requires an alignment-one zeropod storage element; use \
					 `Option<{item_ty}>` for a native schema type"
				));
			}
			if !is_known_fixed_size_type(item_ty, zeropod_enums) {
				return Err(format!(
					"cannot determine the byte size of PodOption element `{item_ty}` in `{ty}`"
				));
			}
			let mut item = try_rust_type_to_codama_with_zeropod_enums(item_ty, zeropod_enums)?;
			let item_size = zeropod_enums
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

fn is_known_zeropod_storage_type(ty: &str) -> bool {
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
			"`{ty}` has unsupported option prefix size {pfx}; zeropod supports 1, 2, or 4 bytes"
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

fn is_known_fixed_size_type(ty: &str, zeropod_enums: &[ZeroPodEnumIr]) -> bool {
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
	) || zeropod_enums.iter().any(|item| item.name == ty)
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
fn type_node_size(node: &TypeNode) -> Option<usize> {
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
		let enums = [ZeroPodEnumIr {
			name: "Color".to_owned(),
			repr_size: 1,
			variants: Vec::new(),
			docs: Vec::new(),
		}];
		let option = try_rust_type_to_codama_with_zeropod_enums("Option<Color>", &enums)
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
	fn rejects_pod_option_prefixes_zeropod_does_not_support() {
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
