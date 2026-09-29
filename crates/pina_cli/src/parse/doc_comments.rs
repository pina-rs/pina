use syn::Attribute;

/// Extract doc comments from a list of attributes.
///
/// Returns each `/// comment` line as a trimmed string. A `#[doc = "..."]`
/// value that spans several lines contributes one entry per line, so no doc
/// entry carries a line terminator into the comments that clients render.
pub fn extract_docs(attrs: &[Attribute]) -> Vec<String> {
	attrs
		.iter()
		.filter_map(|attr| {
			if !attr.path().is_ident("doc") {
				return None;
			}
			match &attr.meta {
				syn::Meta::NameValue(nv) => {
					if let syn::Expr::Lit(syn::ExprLit {
						lit: syn::Lit::Str(s),
						..
					}) = &nv.value
					{
						Some(s.value())
					} else {
						None
					}
				}
				_ => None,
			}
		})
		.flat_map(|value| {
			value
				.replace("\r\n", "\n")
				.split(['\r', '\n', '\u{2028}', '\u{2029}'])
				.map(|line| line.trim().to_owned())
				.collect::<Vec<_>>()
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use syn::parse_quote;

	use super::*;

	#[test]
	fn extracts_doc_comments() {
		let item: syn::ItemStruct = parse_quote! {
			/// First line
			/// Second line
			pub struct Foo;
		};
		let docs = extract_docs(&item.attrs);
		assert_eq!(docs, vec!["First line", "Second line"]);
	}

	#[test]
	fn splits_multiline_doc_values_into_separate_lines() {
		let item: syn::ItemStruct = parse_quote! {
			#[doc = "first\nsecond\r\nthird\u{2028}fourth"]
			pub struct Foo;
		};
		let docs = extract_docs(&item.attrs);
		assert_eq!(docs, vec!["first", "second", "third", "fourth"]);
	}

	#[test]
	fn ignores_non_doc_attrs() {
		let item: syn::ItemStruct = parse_quote! {
			#[derive(Debug)]
			/// A doc
			pub struct Foo;
		};
		let docs = extract_docs(&item.attrs);
		assert_eq!(docs, vec!["A doc"]);
	}
}
