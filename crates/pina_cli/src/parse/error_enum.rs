use syn::File;
use syn::Item;

use super::doc_comments::extract_docs;
use crate::ir::ErrorIr;

/// Extract all `#[error]` enums from a file.
pub fn extract_error_enums(file: &File) -> Vec<ErrorIr> {
	let mut result = Vec::new();

	for item in &file.items {
		let Item::Enum(item_enum) = item else {
			continue;
		};
		if !has_attr(&item_enum.attrs, "error") {
			continue;
		}

		for variant in &item_enum.variants {
			let code = variant
				.discriminant
				.as_ref()
				.and_then(|(_, expr)| expr_to_u32(expr))
				.unwrap_or(0);

			let docs = extract_docs(&variant.attrs);

			result.push(ErrorIr {
				name: variant.ident.to_string(),
				code,
				docs,
			});
		}
	}

	result
}

fn has_attr(attrs: &[syn::Attribute], name: &str) -> bool {
	attrs.iter().any(|a| a.path().is_ident(name))
}

fn expr_to_u32(expr: &syn::Expr) -> Option<u32> {
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
	fn extracts_error_enum() {
		let source = r#"
			#[error]
			#[derive(Debug, Clone, Copy, PartialEq, Eq)]
			pub enum TransferError {
				/// The sender does not have enough lamports.
				InsufficientFunds = 0,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let errors = extract_error_enums(&file);
		assert_eq!(errors.len(), 1);
		assert_eq!(errors[0].name, "InsufficientFunds");
		assert_eq!(errors[0].code, 0);
		assert_eq!(
			errors[0].docs,
			vec!["The sender does not have enough lamports."]
		);
	}
}
