use syn::File;
use syn::Item;

use super::doc_comments::extract_docs;
use super::types::type_to_string;
use crate::ir::FieldIr;

/// A parsed `#[account(discriminator = ...)]` struct.
#[derive(Debug, Clone)]
pub struct AccountStruct {
	pub name: String,
	pub discriminator_enum: String,
	pub fields: Vec<FieldIr>,
	pub docs: Vec<String>,
}

/// Extract all `#[account(...)]` structs from a file.
pub fn extract_account_structs(file: &File) -> Vec<AccountStruct> {
	let mut result = Vec::new();

	for item in &file.items {
		let Item::Struct(item_struct) = item else {
			continue;
		};

		let Some(disc_enum) = get_account_discriminator_enum(&item_struct.attrs) else {
			continue;
		};

		let fields = extract_named_fields(&item_struct.fields);
		let docs = extract_docs(&item_struct.attrs);

		result.push(AccountStruct {
			name: item_struct.ident.to_string(),
			discriminator_enum: disc_enum,
			fields,
			docs,
		});
	}

	result
}

/// Parse the `discriminator = EnumType` from `#[account(discriminator =
/// EnumType)]`.
fn get_account_discriminator_enum(attrs: &[syn::Attribute]) -> Option<String> {
	for attr in attrs {
		if !attr.path().is_ident("account") {
			continue;
		}
		let Ok(meta_list) = attr.parse_args_with(
			syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
		) else {
			continue;
		};
		for meta in &meta_list {
			if let syn::Meta::NameValue(nv) = meta {
				if nv.path.is_ident("discriminator") {
					return Some(expr_to_ident_string(&nv.value));
				}
			}
		}
	}
	None
}

fn expr_to_ident_string(expr: &syn::Expr) -> String {
	match expr {
		syn::Expr::Path(p) => {
			p.path
				.segments
				.iter()
				.map(|s| s.ident.to_string())
				.collect::<Vec<_>>()
				.join("::")
		}
		_ => "unknown".to_owned(),
	}
}

fn extract_named_fields(fields: &syn::Fields) -> Vec<FieldIr> {
	let syn::Fields::Named(named) = fields else {
		return Vec::new();
	};

	named
		.named
		.iter()
		.map(|f| {
			let name = f
				.ident
				.as_ref()
				.map_or_else(|| "unknown".to_owned(), ToString::to_string);
			let rust_type = type_to_string(&f.ty);
			let docs = extract_docs(&f.attrs);
			FieldIr {
				name,
				rust_type,
				docs,
			}
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn extracts_account_struct() {
		let source = r#"
			#[account(discriminator = CounterAccountType)]
			pub struct CounterState {
				pub bump: u8,
				pub count: PodU64,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let accounts = extract_account_structs(&file);
		assert_eq!(accounts.len(), 1);
		assert_eq!(accounts[0].name, "CounterState");
		assert_eq!(accounts[0].discriminator_enum, "CounterAccountType");
		assert_eq!(accounts[0].fields.len(), 2);
		assert_eq!(accounts[0].fields[0].name, "bump");
		assert_eq!(accounts[0].fields[0].rust_type, "u8");
		assert_eq!(accounts[0].fields[1].name, "count");
		assert_eq!(accounts[0].fields[1].rust_type, "PodU64");
	}
}
