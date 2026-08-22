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
	pub is_mutable: bool,
	/// Whether the field is wrapped in `Option<...>`, marking the account
	/// slot as optional in generated clients.
	pub is_optional: bool,
}

/// Extract all `#[derive(Accounts)]` structs from a file.
pub fn extract_accounts_structs(file: &File) -> Result<Vec<AccountsStruct>, syn::Error> {
	let mut result = Vec::new();

	for item in &file.items {
		let Item::Struct(item_struct) = item else {
			continue;
		};
		if !has_accounts_derive(&item_struct.attrs) {
			continue;
		}

		let fields = extract_account_fields(&item_struct.fields)?;
		let docs = extract_docs(&item_struct.attrs);

		result.push(AccountsStruct {
			name: item_struct.ident.to_string(),
			fields,
			docs,
		});
	}

	Ok(result)
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

fn extract_account_fields(fields: &syn::Fields) -> Result<Vec<AccountsField>, syn::Error> {
	let syn::Fields::Named(named) = fields else {
		return Ok(Vec::new());
	};

	named
		.named
		.iter()
		.map(|field| -> Result<AccountsField, syn::Error> {
			let name = field
				.ident
				.as_ref()
				.map_or_else(|| "unknown".to_owned(), ToString::to_string);
			let docs = extract_docs(&field.attrs);
			let is_mutable = type_is_mutable_account(&field.ty);
			let (is_optional, inner_is_mutable) = type_is_optional_account(&field.ty)?
				.map_or((false, None), |inner| (true, Some(inner)));
			// An optional mutable account (`Option<&mut AccountView>`) still
			// declares a writable slot for provided values.
			let is_mutable = is_mutable || inner_is_mutable == Some(true);

			Ok(AccountsField {
				name,
				docs,
				is_mutable,
				is_optional,
			})
		})
		.collect()
}

fn type_is_mutable_account(ty: &syn::Type) -> bool {
	let syn::Type::Reference(reference) = ty else {
		return false;
	};

	reference.mutability.is_some()
}

/// Detect `Option<&AccountView>` / `Option<&mut AccountView>` fields and
/// report whether the wrapped reference is mutable.
fn type_is_optional_account(ty: &syn::Type) -> Result<Option<bool>, syn::Error> {
	let syn::Type::Path(type_path) = ty else {
		return Ok(None);
	};

	let Some(segment) = type_path.path.segments.last() else {
		return Ok(None);
	};
	if segment.ident != "Option" {
		return Ok(None);
	}

	let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
		return Err(syn::Error::new_spanned(
			ty,
			"`Option` account fields require explicit type arguments, e.g. `Option<&AccountView>`",
		));
	};

	let [syn::GenericArgument::Type(syn::Type::Reference(reference))] =
		arguments.args.iter().collect::<Vec<_>>().as_slice()
	else {
		return Err(syn::Error::new_spanned(
			ty,
			"only `Option<&AccountView>` and `Option<&mut AccountView>` fields are supported",
		));
	};
	let syn::Type::Path(referenced_type) = &*reference.elem else {
		return Err(syn::Error::new_spanned(
			ty,
			"only `Option<&AccountView>` and `Option<&mut AccountView>` fields are supported",
		));
	};
	if referenced_type
		.path
		.segments
		.last()
		.is_none_or(|segment| segment.ident != "AccountView")
	{
		return Err(syn::Error::new_spanned(
			ty,
			"only `Option<&AccountView>` and `Option<&mut AccountView>` fields are supported",
		));
	}

	Ok(Some(reference.mutability.is_some()))
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
		let structs = extract_accounts_structs(&file).expect("valid accounts struct");
		assert_eq!(structs.len(), 1);
		assert_eq!(structs[0].name, "InitializeAccounts");
		assert_eq!(structs[0].fields.len(), 3);
		assert_eq!(structs[0].fields[0].name, "authority");
		assert_eq!(structs[0].fields[1].name, "counter");
		assert_eq!(structs[0].fields[2].name, "system_program");
		assert!(structs[0].fields.iter().all(|field| !field.is_mutable));
	}

	#[test]
	fn extracts_mutable_account_fields() {
		let source = r#"
			#[derive(Accounts, Debug)]
			pub struct UpdateAccounts<'a> {
				pub authority: &'a AccountView,
				pub state: &'a mut AccountView,
				#[pina(remaining)]
				pub remaining: &'a mut [AccountView],
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let structs = extract_accounts_structs(&file).expect("valid accounts struct");

		assert_eq!(structs.len(), 1);
		assert_eq!(structs[0].fields.len(), 3);
		assert!(!structs[0].fields[0].is_mutable);
		assert!(structs[0].fields[1].is_mutable);
		assert!(structs[0].fields[2].is_mutable);
	}

	#[test]
	fn extracts_optional_account_fields() {
		let source = r#"
			#[derive(Accounts, Debug)]
			pub struct MakeAccounts<'a> {
				pub maker: &'a mut AccountView,
				pub escrow: Option<&'a mut AccountView>,
				pub witness: Option<&'a AccountView>,
				pub system_program: &'a AccountView,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let structs = extract_accounts_structs(&file).expect("valid accounts struct");

		assert_eq!(structs.len(), 1);
		let fields = &structs[0].fields;
		assert_eq!(fields.len(), 4);

		assert!(!fields[0].is_optional && fields[0].is_mutable);
		assert!(fields[1].is_optional && fields[1].is_mutable);
		assert!(fields[2].is_optional && !fields[2].is_mutable);
		assert!(!fields[3].is_optional && !fields[3].is_mutable);
	}

	#[test]
	fn rejects_invalid_optional_account_fields() {
		for ty in [
			"Option",
			"Option<u64>",
			"Option<&u64>",
			"Option<&mut u64>",
			"Option<&[AccountView]>",
		] {
			let source = format!(
				r#"
					#[derive(Accounts, Debug)]
					pub struct WeirdAccounts<'a> {{
						pub maker: &'a AccountView,
						pub invalid: {ty},
					}}
				"#
			);
			let file = syn::parse_file(&source).unwrap_or_else(|e| panic!("parse failed: {e}"));
			let error = extract_accounts_structs(&file)
				.expect_err("invalid optional account field must fail IDL extraction");
			assert!(error.to_string().contains("Option<&AccountView>"));
		}
	}

	#[test]
	fn handles_non_named_structs_and_non_option_paths() {
		let file = syn::parse_file(
			r#"
				#[derive(Accounts)]
				struct TupleAccounts(&'static AccountView);
				#[derive(Accounts)]
				struct UnitAccounts;
			"#,
		)
		.expect("valid Rust syntax");
		let structs = extract_accounts_structs(&file).expect("non-named structs are tolerated");
		assert_eq!(structs.len(), 2);
		assert!(structs.iter().all(|item| item.fields.is_empty()));

		let non_option = syn::parse_quote!(u64);
		assert_eq!(type_is_optional_account(&non_option).unwrap(), None);

		let empty_path = syn::Type::Path(syn::TypePath {
			attrs: Vec::new(),
			qself: None,
			path: syn::Path {
				leading_colon: None,
				segments: syn::punctuated::Punctuated::new(),
			},
		});
		assert_eq!(type_is_optional_account(&empty_path).unwrap(), None);
	}

	#[test]
	fn accepts_qualified_optional_account_view() {
		let source = r#"
			#[derive(Accounts, Debug)]
			pub struct QualifiedAccounts<'a> {
				pub account: Option<&'a pina::AccountView>,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let structs = extract_accounts_structs(&file).expect("qualified AccountView");
		assert!(structs[0].fields[0].is_optional);
		assert!(!structs[0].fields[0].is_mutable);
	}
}
