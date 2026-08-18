use syn::File;
use syn::Item;

use super::doc_comments::extract_docs;
use crate::error::IdlError;
use crate::ir::PodEnumIr;
use crate::ir::PodEnumVariantIr;

/// Extract local `#[derive(PodEnum)]` unit enums whose layout can be resolved
/// without type checking.
pub fn extract_pod_enums(file: &File) -> Result<Vec<PodEnumIr>, IdlError> {
	let mut result = Vec::new();

	for item in &file.items {
		let Item::Enum(item_enum) = item else {
			continue;
		};
		if !derives_pod_enum(&item_enum.attrs) {
			continue;
		}

		let name = item_enum.ident.to_string();
		let repr_size = repr_size(&item_enum.attrs).ok_or_else(|| {
			IdlError::Other(format!(
				"PodEnum `{name}` requires #[repr(u8)], #[repr(u16)], #[repr(u32)], or \
				 #[repr(u64)]"
			))
		})?;
		let max_discriminant = match repr_size {
			1 => u8::MAX as usize,
			2 => u16::MAX as usize,
			4 | 8 => u32::MAX as usize,
			_ => unreachable!(),
		};
		let mut variants = Vec::new();

		for (index, variant) in item_enum.variants.iter().enumerate() {
			if index > max_discriminant {
				return Err(IdlError::Other(format!(
					"PodEnum `{name}` has more variants than its repr can encode"
				)));
			}
			if !variant.fields.is_empty() {
				return Err(IdlError::Other(format!(
					"PodEnum `{name}` variant `{}` must be a unit variant",
					variant.ident
				)));
			}
			let value = variant
				.discriminant
				.as_ref()
				.and_then(|(_, expression)| literal_u32(expression))
				.ok_or_else(|| {
					IdlError::Other(format!(
						"PodEnum `{name}` variant `{}` requires an explicit literal discriminant \
						 representable by Codama",
						variant.ident
					))
				})?;
			if value != index as u32 {
				return Err(IdlError::Other(format!(
					"PodEnum `{name}` variant `{}` uses discriminant {value}; generated \
					 JavaScript clients currently require contiguous discriminants starting at \
					 zero",
					variant.ident
				)));
			}

			variants.push(PodEnumVariantIr {
				name: variant.ident.to_string(),
				value,
			});
		}

		result.push(PodEnumIr {
			zc_name: format!("{name}Zc"),
			name,
			repr_size,
			variants,
			docs: extract_docs(&item_enum.attrs),
		});
	}

	Ok(result)
}

fn derives_pod_enum(attrs: &[syn::Attribute]) -> bool {
	attrs.iter().any(|attribute| {
		if !attribute.path().is_ident("derive") {
			return false;
		}

		let mut found = false;
		let _ = attribute.parse_nested_meta(|meta| {
			if meta
				.path
				.segments
				.last()
				.is_some_and(|segment| segment.ident == "PodEnum")
			{
				found = true;
			}
			Ok(())
		});
		found
	})
}

fn repr_size(attrs: &[syn::Attribute]) -> Option<usize> {
	for attribute in attrs {
		if !attribute.path().is_ident("repr") {
			continue;
		}

		let mut size = None;
		let _ = attribute.parse_nested_meta(|meta| {
			size = if meta.path.is_ident("u8") {
				Some(1)
			} else if meta.path.is_ident("u16") {
				Some(2)
			} else if meta.path.is_ident("u32") {
				Some(4)
			} else if meta.path.is_ident("u64") {
				Some(8)
			} else {
				size
			};
			Ok(())
		});
		if size.is_some() {
			return size;
		}
	}

	None
}

fn literal_u32(expression: &syn::Expr) -> Option<u32> {
	let syn::Expr::Lit(syn::ExprLit {
		lit: syn::Lit::Int(literal),
		..
	}) = expression
	else {
		return None;
	};

	literal.base10_parse().ok()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn extracts_local_pod_enum() {
		let file = syn::parse_file(
			r#"
				/// A color stored on chain.
				#[derive(Clone, pina::PodEnum)]
				#[repr(u16)]
				enum Color {
					/// Red.
					Red = 0,
					Blue = 1,
				}
			"#,
		)
		.unwrap_or_else(|error| panic!("parse failed: {error}"));

		let enums = extract_pod_enums(&file).unwrap_or_else(|error| panic!("{error}"));
		assert_eq!(enums.len(), 1);
		assert_eq!(enums[0].name, "Color");
		assert_eq!(enums[0].zc_name, "ColorZc");
		assert_eq!(enums[0].repr_size, 2);
		assert_eq!(enums[0].variants[1].value, 1);
		assert_eq!(enums[0].docs, vec!["A color stored on chain."]);
	}

	#[test]
	fn rejects_non_literal_discriminants() {
		let file = syn::parse_file(
			r#"
				#[derive(PodEnum)]
				#[repr(u8)]
				enum Color { Red = VALUE }
			"#,
		)
		.unwrap_or_else(|error| panic!("parse failed: {error}"));

		let error = extract_pod_enums(&file).expect_err("expression must fail closed");
		assert!(error.to_string().contains("explicit literal"));
	}

	#[test]
	fn rejects_sparse_discriminants_until_js_preserves_them() {
		let file = syn::parse_file(
			r#"
				#[derive(PodEnum)]
				#[repr(u8)]
				enum Color { Red = 0, Blue = 7 }
			"#,
		)
		.unwrap_or_else(|error| panic!("parse failed: {error}"));

		let error = extract_pod_enums(&file).expect_err("sparse enum must fail closed");
		assert!(error.to_string().contains("contiguous discriminants"));
	}
}
