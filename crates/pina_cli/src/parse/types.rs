use codama_nodes::ArrayTypeNode;
use codama_nodes::BooleanTypeNode;
use codama_nodes::BytesTypeNode;
use codama_nodes::CountNode;
use codama_nodes::DefinedTypeLinkNode;
use codama_nodes::FixedSizeTypeNode;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::PublicKeyTypeNode;
use codama_nodes::SizePrefixTypeNode;
use codama_nodes::StringTypeNode;
use codama_nodes::TypeNode;
use quote::ToTokens;

use crate::ir::PodEnumIr;

/// Map a Rust type name (as it appears in pina structs) to a Codama
/// `TypeNode`.
pub fn rust_type_to_codama(ty: &str) -> TypeNode {
	try_rust_type_to_codama(ty).unwrap_or_else(|_| PublicKeyTypeNode::new().into())
}

/// Fallible type mapping used by IDL generation.
///
/// Unsupported Pod collection layouts are rejected rather than silently
/// emitted as public keys with an incorrect wire size.
pub fn try_rust_type_to_codama(ty: &str) -> Result<TypeNode, String> {
	try_rust_type_to_codama_with_pod_enums(ty, &[])
}

/// Fallible type mapping with the local `PodEnum` companion registry.
pub fn try_rust_type_to_codama_with_pod_enums(
	ty: &str,
	pod_enums: &[PodEnumIr],
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
			if pod_enums.iter().any(|pod_enum| pod_enum.zc_name == ty) {
				return Ok(DefinedTypeLinkNode::new(ty).into());
			}
			// Handle fixed-size byte arrays like [u8; 32]
			if let Some(size) = parse_byte_array(ty) {
				Ok(FixedSizeTypeNode::<TypeNode>::new(BytesTypeNode::new(), size).into())
			} else if let Some(node) = parse_pod_collection(ty, pod_enums)? {
				Ok(node)
			} else {
				Err(format!(
					"unknown fixed-layout type `{ty}`; only Address and Pubkey map to public keys"
				))
			}
		}
	}
}

/// Parse a `PodString<N, PFX>` or `PodVec<T, N, PFX>` type into a semantic,
/// fixed-size Codama node. `PodOption<T>` remains unsupported until its
/// semantic mapping is represented in the generated client schema.
///
/// - `PodString<N, PFX = 1>` maps to a fixed-size, size-prefixed UTF-8 string.
/// - `PodVec<T, N, PFX = 2>` maps to a fixed-size, prefix-counted array.
///
/// Returns `Ok(None)` for non-collection types and an error for collection
/// layouts whose byte size cannot be resolved statically.
fn parse_pod_collection(ty: &str, pod_enums: &[PodEnumIr]) -> Result<Option<TypeNode>, String> {
	let Some((name, args)) = parse_generic_args(ty) else {
		if ty == "PodString"
			|| ty.starts_with("PodString<")
			|| ty == "PodVec"
			|| ty.starts_with("PodVec<")
			|| ty == "PodOption"
			|| ty.starts_with("PodOption<")
		{
			return Err(format!("unable to parse Pod collection type `{ty}`"));
		}

		return Ok(None);
	};

	match name.as_str() {
		"PodString" => {
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
		"PodVec" => {
			if !(2..=3).contains(&args.len()) {
				return Err(format!("`{ty}` expects two or three generic arguments"));
			}

			let item_ty = args
				.first()
				.ok_or_else(|| format!("`{ty}` is missing its element type"))?;
			if !is_known_fixed_size_type(item_ty, pod_enums) {
				return Err(format!(
					"cannot determine the byte size of PodVec element `{item_ty}` in `{ty}`"
				));
			}
			let mut item = try_rust_type_to_codama_with_pod_enums(item_ty, pod_enums)?;
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
			let item_size = pod_enums
				.iter()
				.find(|pod_enum| pod_enum.zc_name == *item_ty)
				.map(|pod_enum| pod_enum.repr_size)
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
		"PodOption" => Err(format!("`{ty}` is not yet supported by IDL generation")),
		_ => Ok(None),
	}
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

fn is_known_fixed_size_type(ty: &str, pod_enums: &[PodEnumIr]) -> bool {
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
	) || pod_enums.iter().any(|pod_enum| pod_enum.zc_name == ty)
		|| parse_byte_array(ty).is_some()
		|| ty.starts_with("PodString<")
		|| ty.starts_with("PodVec<")
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

	#[test]
	fn maps_pod_types() {
		assert_eq!(
			rust_type_to_codama("PodU64"),
			NumberTypeNode::le(NumberFormat::U64).into()
		);
		assert_eq!(
			rust_type_to_codama("PodBool"),
			BooleanTypeNode::default().into()
		);
	}

	#[test]
	fn maps_primitives() {
		assert_eq!(
			rust_type_to_codama("u8"),
			NumberTypeNode::le(NumberFormat::U8).into()
		);
	}

	#[test]
	fn maps_address() {
		assert_eq!(
			rust_type_to_codama("Address"),
			PublicKeyTypeNode::new().into()
		);
	}

	#[test]
	fn maps_byte_array() {
		let ty = rust_type_to_codama("[u8; 32]");
		let expected: TypeNode =
			FixedSizeTypeNode::<TypeNode>::new(BytesTypeNode::new(), 32).into();
		assert_eq!(ty, expected);
	}

	#[test]
	fn maps_pod_string() {
		// PodString<32> = 1 length byte + 32 payload bytes.
		let ty = rust_type_to_codama("PodString<32>");
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
		let ty = rust_type_to_codama("PodString<64, 2>");
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
			let TypeNode::FixedSize(node) = rust_type_to_codama(ty) else {
				panic!("{ty} did not lower to a fixed-size node");
			};
			assert_eq!(node.size, size);
		}

		let TypeNode::FixedSize(node) = rust_type_to_codama("PodVec<PodString<8, 1>, 4, 2>") else {
			panic!("nested PodVec did not lower to a fixed-size node");
		};
		assert_eq!(node.size, 38);
	}

	#[test]
	fn maps_pod_vec() {
		// PodVec<PodU64, 8> = 2 count bytes + 8 × 8-byte elements.
		let ty = rust_type_to_codama("PodVec<PodU64, 8>");
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
		let ty = rust_type_to_codama("PodVec<PodU16, 4, 1>");
		let array = ArrayTypeNode::prefixed(
			NumberTypeNode::le(NumberFormat::U16),
			NumberTypeNode::le(NumberFormat::U8),
		);
		let expected: TypeNode = FixedSizeTypeNode::<TypeNode>::new(array, 9).into();
		assert_eq!(ty, expected);
	}

	#[test]
	fn maps_pod_vec_with_signed_pod_elements() {
		let ty = rust_type_to_codama("PodVec<PodI32, 8>");
		let array = ArrayTypeNode::prefixed(
			NumberTypeNode::le(NumberFormat::I32),
			NumberTypeNode::le(NumberFormat::U16),
		);
		let expected: TypeNode = FixedSizeTypeNode::<TypeNode>::new(array, 34).into();
		assert_eq!(ty, expected);

		let ty = rust_type_to_codama("PodVec<PodI128, 8>");
		let array = ArrayTypeNode::prefixed(
			NumberTypeNode::le(NumberFormat::I128),
			NumberTypeNode::le(NumberFormat::U16),
		);
		let expected: TypeNode = FixedSizeTypeNode::<TypeNode>::new(array, 130).into();
		assert_eq!(ty, expected);
	}

	#[test]
	fn rejects_pod_collections_with_unresolved_sizes() {
		for ty in [
			"PodString<NAME_LEN>",
			"PodVec<PodU64, { 4 + 4 }>",
			"PodVec<MyPod, 8>",
			"PodOption",
			"PodOption<>",
			"PodOption<PodU64>",
			"PodOption<PodU64, PodU64>",
		] {
			let error = try_rust_type_to_codama(ty)
				.expect_err("unresolved Pod collection sizes must be rejected");
			assert!(error.contains(ty), "unexpected error for {ty}: {error}");
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
