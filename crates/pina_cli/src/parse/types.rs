use codama_nodes::BooleanTypeNode;
use codama_nodes::BytesTypeNode;
use codama_nodes::FixedSizeTypeNode;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::PublicKeyTypeNode;
use codama_nodes::TypeNode;

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
			} else {
				// Fallback: treat unknown types as public keys (common for
				// address-like types)
				PublicKeyTypeNode::new().into()
			}
		}
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
			// Use the last segment (e.g. `PodU64` from `pina::PodU64`)
			if let Some(seg) = p.path.segments.last() {
				seg.ident.to_string()
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
}
