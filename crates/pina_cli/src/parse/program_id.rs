use solana_address::Address;
use syn::File;
use syn::Item;

use crate::error::IdlError;

/// Extract a canonical Solana address from a `declare_id!("...")` invocation.
///
/// Returns `None` when the declaration is missing or invalid.
pub fn extract_program_id(file: &File) -> Option<String> {
	try_extract_program_id(file).ok().flatten()
}

/// Extract a canonical address while preserving invalid-declaration diagnostics.
///
/// # Errors
///
/// Returns an error when a declaration is not a single string literal whose
/// value is a 32-byte Solana address encoded in Base58.
pub(crate) fn try_extract_program_id(file: &File) -> Result<Option<String>, IdlError> {
	for item in &file.items {
		if let Item::Macro(item_macro) = item
			&& path_ends_with(&item_macro.mac.path, "declare_id")
		{
			let literal =
				syn::parse2::<syn::LitStr>(item_macro.mac.tokens.clone()).map_err(|error| {
					IdlError::Other(format!(
						"declare_id! requires a single string literal: {error}"
					))
				})?;
			// Solana's --program-id also accepts keypair paths; only addresses may reach the IDL.
			let address = literal.value().parse::<Address>().map_err(|error| {
				IdlError::Other(format!(
					"declare_id! requires a base58-encoded 32-byte Solana address: {error}"
				))
			})?;

			return Ok(Some(address.to_string()));
		}
	}

	Ok(None)
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

	#[test]
	fn accepts_rust_string_literals_and_canonicalizes_the_address() {
		for source in [
			r#"pina::declare_id!(r"11111111111111111111111111111111");"#,
			r#"declare_id!("\x311111111111111111111111111111111");"#,
		] {
			let file = syn::parse_file(source).unwrap();
			assert_eq!(
				extract_program_id(&file).as_deref(),
				Some("11111111111111111111111111111111")
			);
		}
	}

	#[test]
	fn rejects_paths_invalid_base58_and_wrong_address_lengths() {
		for value in [
			"keys/other-program.json",
			"/tmp/program.json",
			"../program.json",
			"",
			"0OIl",
			"1111111111111111111111111111111",
			"111111111111111111111111111111111",
			"111111111111111111111111111111111111111111111111",
		] {
			let file = syn::parse_file(&format!("declare_id!({value:?});")).unwrap();
			assert_eq!(extract_program_id(&file), None);
			let error = try_extract_program_id(&file).unwrap_err().to_string();
			assert!(
				error.contains("base58-encoded 32-byte Solana address"),
				"{error}"
			);
		}
	}

	#[test]
	fn rejects_nonliteral_and_extra_macro_arguments() {
		for source in [
			"declare_id!(PROGRAM_ID);",
			"declare_id!(123);",
			"declare_id!();",
			r#"declare_id!("11111111111111111111111111111111", "extra");"#,
		] {
			let file = syn::parse_file(source).unwrap();
			assert_eq!(extract_program_id(&file), None);
			let error = try_extract_program_id(&file).unwrap_err().to_string();
			assert!(error.contains("single string literal"), "{error}");
		}
	}
}
