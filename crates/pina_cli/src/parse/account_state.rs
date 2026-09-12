use syn::File;
use syn::Item;

use super::discriminator::extract_discriminator_and_variant;
use super::doc_comments::extract_docs;
use super::types::type_to_string;
use crate::error::IdlError;
use crate::ir::COMPACT_ACCOUNT_DOC_MARKER;
use crate::ir::FieldIr;

/// A parsed `#[account(discriminator = ...)]` struct.
#[derive(Debug, Clone)]
pub struct AccountStruct {
	pub name: String,
	pub discriminator_enum: String,
	pub variant: String,
	pub fields: Vec<FieldIr>,
	pub docs: Vec<String>,
	pub migratable: bool,
	/// The name of the PDA declared for this account via `#[pda(...)]`.
	pub pda_name: Option<String>,
}

impl AccountStruct {
	pub(crate) fn is_compact(&self) -> bool {
		self.docs
			.iter()
			.any(|doc| doc == COMPACT_ACCOUNT_DOC_MARKER)
	}
}

/// Extract all `#[account(...)]` structs from a file.
///
/// # Errors
///
/// Returns an error when an account discriminator does not match the grammar
/// accepted by the attribute macro.
pub fn extract_account_structs(file: &File) -> Result<Vec<AccountStruct>, IdlError> {
	let mut result = Vec::new();

	for item in &file.items {
		let Item::Struct(item_struct) = item else {
			continue;
		};

		let Some((discriminator_enum, variant)) =
			extract_discriminator_and_variant(&item_struct.attrs, "account", &item_struct.ident)?
		else {
			continue;
		};

		let fields = extract_named_fields(&item_struct.fields);
		let mut docs = extract_docs(&item_struct.attrs);
		if has_compact_flag(&item_struct.attrs) {
			docs.push(COMPACT_ACCOUNT_DOC_MARKER.to_owned());
		}
		let pda_name = extract_pda_name(&item_struct.attrs, &item_struct.ident.to_string());
		let migratable = super::event_data::has_migrations_flag(&item_struct.attrs, "account");

		result.push(AccountStruct {
			name: item_struct.ident.to_string(),
			discriminator_enum,
			variant,
			fields,
			docs,
			migratable,
			pda_name,
		});
	}

	Ok(result)
}

fn has_compact_flag(attrs: &[syn::Attribute]) -> bool {
	attrs.iter().any(|attr| {
		if !attr.path().is_ident("account") {
			return false;
		}
		attr.parse_args_with(
			syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
		)
		.is_ok_and(|items| items.iter().any(|item| item.path().is_ident("compact")))
	})
}

/// Derive the IDL PDA name for a struct with a `#[pda(...)]` attribute.
///
/// The `State` suffix is stripped so `CounterState` produces the `counter`
/// PDA, matching the naming convention used by the example programs.
fn extract_pda_name(attrs: &[syn::Attribute], struct_name: &str) -> Option<String> {
	let has_pda_attr = attrs.iter().any(|attr| attr.path().is_ident("pda"));
	if !has_pda_attr {
		return None;
	}

	Some(super::pda_attr::pda_name_for_struct(struct_name))
}

pub(crate) fn extract_named_fields(fields: &syn::Fields) -> Vec<FieldIr> {
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
		let accounts =
			extract_account_structs(&file).unwrap_or_else(|e| panic!("extract failed: {e}"));
		assert_eq!(accounts.len(), 1);
		assert_eq!(accounts[0].name, "CounterState");
		assert!(!accounts[0].is_compact());
		assert_eq!(accounts[0].discriminator_enum, "CounterAccountType");
		assert_eq!(accounts[0].variant, "CounterState");
		assert_eq!(accounts[0].fields.len(), 2);
		assert_eq!(accounts[0].fields[0].name, "bump");
		assert_eq!(accounts[0].fields[0].rust_type, "u8");
		assert_eq!(accounts[0].fields[1].name, "count");
		assert_eq!(accounts[0].fields[1].rust_type, "PodU64");
	}

	#[test]
	fn extracts_compact_account_flag() {
		let source = r#"
			#[account(discriminator = AccountType, compact)]
			pub struct DynamicState { pub values: Vec<u64, 8> }
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let accounts =
			extract_account_structs(&file).unwrap_or_else(|e| panic!("extract failed: {e}"));

		assert!(accounts[0].is_compact());
	}

	#[test]
	fn extracts_account_path_variant() {
		let source = r#"
			#[account(discriminator = crate::types::CounterAccountType::Counter)]
			pub struct CounterState {}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let accounts =
			extract_account_structs(&file).unwrap_or_else(|e| panic!("extract failed: {e}"));

		assert_eq!(accounts[0].discriminator_enum, "CounterAccountType");
		assert_eq!(accounts[0].variant, "Counter");
	}

	#[test]
	fn extracts_account_with_explicit_variant() {
		let source = r#"
			#[account(discriminator = crate::types::CounterAccountType, variant = Counter)]
			pub struct CounterState {}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let accounts =
			extract_account_structs(&file).unwrap_or_else(|e| panic!("extract failed: {e}"));

		assert_eq!(accounts[0].discriminator_enum, "CounterAccountType");
		assert_eq!(accounts[0].variant, "Counter");
	}
}
