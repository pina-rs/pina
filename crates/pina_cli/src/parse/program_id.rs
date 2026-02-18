use syn::File;
use syn::Item;

/// Extract the program ID from a `declare_id!("...")` macro invocation.
pub fn extract_program_id(file: &File) -> Option<String> {
	for item in &file.items {
		if let Item::Macro(item_macro) = item {
			let path = &item_macro.mac.path;
			if path_ends_with(path, "declare_id") {
				let tokens = item_macro.mac.tokens.to_string();
				// The tokens look like: "base58string" — strip quotes.
				let trimmed = tokens.trim().trim_matches('"');
				if !trimmed.is_empty() {
					return Some(trimmed.to_owned());
				}
			}
		}
	}
	None
}

fn path_ends_with(path: &syn::Path, ident: &str) -> bool {
	path.segments.last().is_some_and(|seg| seg.ident == ident)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn extracts_program_id() {
		let source = r#"declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		assert_eq!(
			extract_program_id(&file),
			Some("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS".into())
		);
	}

	#[test]
	fn returns_none_when_missing() {
		let source = "pub fn foo() {}";
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		assert_eq!(extract_program_id(&file), None);
	}
}
