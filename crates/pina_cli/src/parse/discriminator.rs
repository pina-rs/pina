use syn::File;
use syn::Item;

/// A parsed `#[discriminator]` enum.
#[derive(Debug, Clone)]
pub struct DiscriminatorEnum {
	pub name: String,
	pub variants: Vec<DiscriminatorVariant>,
	/// The repr size in bytes (1 for u8, 2 for u16, etc.). Defaults to 1.
	pub repr_size: usize,
}

#[derive(Debug, Clone)]
pub struct DiscriminatorVariant {
	pub name: String,
	pub value: u64,
}

/// Extract all `#[discriminator]` enums from a file.
pub fn extract_discriminator_enums(file: &File) -> Vec<DiscriminatorEnum> {
	let mut result = Vec::new();

	for item in &file.items {
		let Item::Enum(item_enum) = item else {
			continue;
		};
		if !has_attr(&item_enum.attrs, "discriminator") {
			continue;
		}

		let repr_size = detect_repr_size(&item_enum.attrs);
		let mut variants = Vec::new();
		for variant in &item_enum.variants {
			if let Some((_, expr)) = &variant.discriminant {
				if let Some(val) = expr_to_u64(expr) {
					variants.push(DiscriminatorVariant {
						name: variant.ident.to_string(),
						value: val,
					});
				}
			}
		}

		result.push(DiscriminatorEnum {
			name: item_enum.ident.to_string(),
			variants,
			repr_size,
		});
	}

	result
}

/// Detect `#[repr(u8)]`, `#[repr(u16)]`, etc. Default to 1 byte.
fn detect_repr_size(attrs: &[syn::Attribute]) -> usize {
	for attr in attrs {
		if !attr.path().is_ident("repr") {
			continue;
		}
		let Ok(inner) = attr.parse_args::<syn::Ident>() else {
			continue;
		};
		return match inner.to_string().as_str() {
			"u16" => 2,
			"u32" => 4,
			"u64" => 8,
			_ => 1,
		};
	}
	// The #[discriminator] macro defaults to u8 repr.
	1
}

fn has_attr(attrs: &[syn::Attribute], name: &str) -> bool {
	attrs.iter().any(|a| a.path().is_ident(name))
}

fn expr_to_u64(expr: &syn::Expr) -> Option<u64> {
	match expr {
		syn::Expr::Lit(syn::ExprLit {
			lit: syn::Lit::Int(lit),
			..
		}) => lit.base10_parse().ok(),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn extracts_discriminator_enum() {
		let source = r#"
			#[discriminator]
			pub enum MyInstruction {
				Foo = 0,
				Bar = 1,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let enums = extract_discriminator_enums(&file);
		assert_eq!(enums.len(), 1);
		assert_eq!(enums[0].name, "MyInstruction");
		assert_eq!(enums[0].variants.len(), 2);
		assert_eq!(enums[0].variants[0].name, "Foo");
		assert_eq!(enums[0].variants[0].value, 0);
		assert_eq!(enums[0].variants[1].name, "Bar");
		assert_eq!(enums[0].variants[1].value, 1);
		assert_eq!(enums[0].repr_size, 1);
	}
}
