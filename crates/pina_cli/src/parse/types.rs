use codama_nodes::BooleanTypeNode;
use codama_nodes::BytesTypeNode;
use codama_nodes::CountNode;
use codama_nodes::FixedSizeTypeNode;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::PublicKeyTypeNode;
use codama_nodes::TypeNode;
use quote::ToTokens;

/// Map a Rust type name (as it appears in pina structs) to a Codama
/// `TypeNode`.
pub fn rust_type_to_codama(ty: &str) -> TypeNode {
	match ty {
		"u8" => NumberTypeNode::le(NumberFormat::U8).into(),
		"u16" | "PodU16" => NumberTypeNode::le(NumberFormat::U16).into(),
		"u32" | "PodU32" => NumberTypeNode::le(NumberFormat::U32).into(),
		"u64" | "PodU64" => NumberTypeNode::le(NumberFormat::U64).into(),
		"u128" | "PodU128" => NumberTypeNode::le(NumberFormat::U128).into(),
		"i8" => NumberTypeNode::le(NumberFormat::I8).into(),
		"i16" | "PodI16" => NumberTypeNode::le(NumberFormat::I16).into(),
		"i32" => NumberTypeNode::le(NumberFormat::I32).into(),
		"i64" | "PodI64" => NumberTypeNode::le(NumberFormat::I64).into(),
		"i128" => NumberTypeNode::le(NumberFormat::I128).into(),
		"PodBool" | "bool" => BooleanTypeNode::default().into(),
		"Address" | "Pubkey" => PublicKeyTypeNode::new().into(),
		_ => {
			// Handle fixed-size byte arrays like [u8; 32]
			if let Some(size) = parse_byte_array(ty) {
				FixedSizeTypeNode::<TypeNode>::new(BytesTypeNode::new(), size).into()
			} else if let Some(node) = parse_pod_collection(ty) {
				node
			} else {
				// Fallback: treat unknown types as public keys (common for
				// address-like types)
				PublicKeyTypeNode::new().into()
			}
		}
	}
}

/// Parse a `PodString<N, PFX>` or `PodVec<T, N, PFX>` type into a fixed-size
/// Codama node.
///
/// - `PodString<N, PFX = 1>` maps to `FixedSizeTypeNode(BytesTypeNode, N + PFX)`:
///   the length prefix plus the UTF-8 payload, laid out inline.
/// - `PodVec<T, N, PFX = 2>` maps to
///   `FixedSizeTypeNode(BytesTypeNode, N × size_of::<T>() + PFX)`: the length
///   prefix plus the fixed element array, laid out inline.
///
/// Returns `None` for non-collection types or unparseable arguments.
fn parse_pod_collection(ty: &str) -> Option<TypeNode> {
	let (name, args) = parse_generic_args(ty)?;
	match name.as_str() {
		"PodString" => {
			let n: usize = args.first()?.parse().ok()?;
			let pfx: usize = match args.get(1) {
				Some(s) => s.parse().ok()?,
				None => 1,
			};
			Some(FixedSizeTypeNode::<TypeNode>::new(BytesTypeNode::new(), n + pfx).into())
		}
		"PodVec" => {
			let item = rust_type_to_codama(args.first()?);
			let n: usize = args.get(1)?.parse().ok()?;
			let pfx: usize = match args.get(2) {
				Some(s) => s.parse().ok()?,
				None => 2,
			};
			// Wire layout: [count: PFX bytes][items: N × T]. Emit the full
			// fixed size (prefix + elements) so generated clients decode the
			// correct account size and field offsets.
			let item_size = type_node_size(&item)?;
			Some(
				FixedSizeTypeNode::<TypeNode>::new(BytesTypeNode::new(), n * item_size + pfx)
					.into(),
			)
		}
		_ => None,
	}
}

/// Compute the on-chain byte size of a fixed-size Codama type node.
///
/// Returns `None` for variable-size or unsupported nodes.
fn type_node_size(node: &TypeNode) -> Option<usize> {
	match node {
		TypeNode::Number(number) => match number.format {
			NumberFormat::U8 | NumberFormat::I8 => Some(1),
			NumberFormat::U16 | NumberFormat::I16 => Some(2),
			NumberFormat::U32 | NumberFormat::I32 => Some(4),
			NumberFormat::U64 | NumberFormat::I64 => Some(8),
			NumberFormat::U128 | NumberFormat::I128 => Some(16),
			NumberFormat::F32 | NumberFormat::F64 | NumberFormat::ShortU16 => None,
		},
		TypeNode::Boolean(_) => Some(1),
		TypeNode::PublicKey(_) => Some(32),
		TypeNode::FixedSize(fixed) => Some(fixed.size),
		TypeNode::Array(array) => match array.count.as_ref() {
			CountNode::Fixed(count) => {
				type_node_size(&array.item).map(|size| size * count.value as usize)
			}
			_ => None,
		},
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
		let expected: TypeNode =
			FixedSizeTypeNode::<TypeNode>::new(BytesTypeNode::new(), 33).into();
		assert_eq!(ty, expected);
	}

	#[test]
	fn maps_pod_string_with_explicit_prefix() {
		// PodString<64, 2> = 2 length bytes + 64 payload bytes.
		let ty = rust_type_to_codama("PodString<64, 2>");
		let expected: TypeNode =
			FixedSizeTypeNode::<TypeNode>::new(BytesTypeNode::new(), 66).into();
		assert_eq!(ty, expected);
	}

	#[test]
	fn maps_pod_vec() {
		// PodVec<PodU64, 8> = 2 count bytes + 8 × 8-byte elements.
		let ty = rust_type_to_codama("PodVec<PodU64, 8>");
		let expected: TypeNode =
			FixedSizeTypeNode::<TypeNode>::new(BytesTypeNode::new(), 66).into();
		assert_eq!(ty, expected);
	}

	#[test]
	fn maps_pod_vec_with_explicit_prefix() {
		// PodVec<PodU16, 4, 1> = 1 count byte + 4 × 2-byte elements.
		let ty = rust_type_to_codama("PodVec<PodU16, 4, 1>");
		let expected: TypeNode =
			FixedSizeTypeNode::<TypeNode>::new(BytesTypeNode::new(), 9).into();
		assert_eq!(ty, expected);
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
