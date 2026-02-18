use syn::Attribute;

/// Extract doc comments from a list of attributes.
///
/// Returns each `/// comment` line as a trimmed string.
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
						Some(s.value().trim().to_owned())
					} else {
						None
					}
				}
				_ => None,
			}
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
