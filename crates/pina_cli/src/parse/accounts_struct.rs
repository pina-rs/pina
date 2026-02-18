use syn::File;
use syn::Item;

use super::doc_comments::extract_docs;

/// A parsed `#[derive(Accounts)]` struct.
#[derive(Debug, Clone)]
pub struct AccountsStruct {
	pub name: String,
	pub fields: Vec<AccountsField>,
	pub docs: Vec<String>,
}

/// A single field inside an `#[derive(Accounts)]` struct.
#[derive(Debug, Clone)]
pub struct AccountsField {
	pub name: String,
	pub docs: Vec<String>,
}

/// Extract all `#[derive(Accounts)]` structs from a file.
pub fn extract_accounts_structs(file: &File) -> Vec<AccountsStruct> {
	let mut result = Vec::new();

	for item in &file.items {
		let Item::Struct(item_struct) = item else {
			continue;
		};
		if !has_accounts_derive(&item_struct.attrs) {
			continue;
		}

		let fields = extract_account_fields(&item_struct.fields);
		let docs = extract_docs(&item_struct.attrs);

		result.push(AccountsStruct {
			name: item_struct.ident.to_string(),
			fields,
			docs,
		});
	}

	result
}

fn has_accounts_derive(attrs: &[syn::Attribute]) -> bool {
	for attr in attrs {
		if !attr.path().is_ident("derive") {
			continue;
		}
		let Ok(meta_list) = attr.parse_args_with(
			syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
		) else {
			continue;
		};
		for path in &meta_list {
			if path.is_ident("Accounts") {
				return true;
			}
		}
	}
	false
}

fn extract_account_fields(fields: &syn::Fields) -> Vec<AccountsField> {
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
			let docs = extract_docs(&f.attrs);
			AccountsField { name, docs }
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn extracts_accounts_struct() {
		let source = r#"
			#[derive(Accounts, Debug)]
			pub struct InitializeAccounts<'a> {
				/// The authority.
				pub authority: &'a AccountView,
				/// The counter PDA.
				pub counter: &'a AccountView,
				/// System program.
				pub system_program: &'a AccountView,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let structs = extract_accounts_structs(&file);
		assert_eq!(structs.len(), 1);
		assert_eq!(structs[0].name, "InitializeAccounts");
		assert_eq!(structs[0].fields.len(), 3);
		assert_eq!(structs[0].fields[0].name, "authority");
		assert_eq!(structs[0].fields[1].name, "counter");
		assert_eq!(structs[0].fields[2].name, "system_program");
	}
}
