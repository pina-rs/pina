use syn::File;
use syn::Item;

use super::doc_comments::extract_docs;
use crate::ir::ErrorIr;

/// Extract all `#[error]` enums from a file.
///
/// Each variant records the code the program returns for it: an explicit
/// discriminant, or for a variant without one, one more than the previous
/// variant's code (starting at zero), which is how Rust numbers a `repr(u32)`
/// enum.
///
/// # Errors
///
/// Returns an error when a discriminant is not an integer literal that fits
/// `u32`, or when an implicit discriminant would follow `u32::MAX`. The
/// extractor cannot evaluate other expressions, and guessing a code would
/// publish an IDL whose errors decode to the wrong variant.
pub fn extract_error_enums(file: &File) -> Result<Vec<ErrorIr>, syn::Error> {
	Ok(extract_declared_errors(file)?
		.into_iter()
		.map(|declared| declared.error)
		.collect())
}

/// An `#[error]` variant together with the enum that declares it.
#[derive(Debug, Clone)]
pub(crate) struct DeclaredError {
	/// Name of the `#[error]` enum, such as `ValidationError`.
	pub(crate) enum_name: String,
	/// The variant and the code the program returns for it.
	pub(crate) error: ErrorIr,
}

/// [`extract_error_enums`], keeping each variant's enum name so source sites
/// that construct `Enum::Variant` can be found.
pub(crate) fn extract_declared_errors(file: &File) -> Result<Vec<DeclaredError>, syn::Error> {
	let mut result = Vec::new();

	for item in &file.items {
		let Item::Enum(item_enum) = item else {
			continue;
		};
		if !has_attr(&item_enum.attrs, "error") {
			continue;
		}

		let mut next_code = Some(0_u32);
		for variant in &item_enum.variants {
			let code = match &variant.discriminant {
				Some((_, expression)) => discriminant_to_u32(expression, item_enum, variant)?,
				None => {
					next_code.ok_or_else(|| {
						syn::Error::new_spanned(
							&variant.ident,
							format!(
								"the implicit discriminant of `{}::{}` overflows u32; give it an \
								 explicit value below 0xFFFF_0000",
								item_enum.ident, variant.ident
							),
						)
					})?
				}
			};
			next_code = code.checked_add(1);

			result.push(DeclaredError {
				enum_name: item_enum.ident.to_string(),
				error: ErrorIr {
					name: variant.ident.to_string(),
					code,
					docs: extract_docs(&variant.attrs),
				},
			});
		}
	}

	Ok(result)
}

fn has_attr(attrs: &[syn::Attribute], name: &str) -> bool {
	attrs.iter().any(|a| a.path().is_ident(name))
}

fn discriminant_to_u32(
	expression: &syn::Expr,
	item_enum: &syn::ItemEnum,
	variant: &syn::Variant,
) -> Result<u32, syn::Error> {
	let syn::Expr::Lit(syn::ExprLit {
		lit: syn::Lit::Int(literal),
		..
	}) = expression
	else {
		return Err(syn::Error::new_spanned(
			expression,
			format!(
				"pina cannot evaluate the discriminant of `{}::{}`; use an integer literal so the \
				 IDL records the code the program returns",
				item_enum.ident, variant.ident
			),
		));
	};

	literal.base10_parse().map_err(|_| {
		syn::Error::new_spanned(
			literal,
			format!(
				"the discriminant of `{}::{}` does not fit u32",
				item_enum.ident, variant.ident
			),
		)
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	fn parse(source: &str) -> File {
		syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"))
	}

	fn codes(source: &str) -> Vec<(String, u32)> {
		extract_error_enums(&parse(source))
			.expect("valid error enum")
			.into_iter()
			.map(|error| (error.name, error.code))
			.collect()
	}

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
		let errors = extract_error_enums(&parse(source)).expect("valid error enum");
		assert_eq!(errors.len(), 1);
		assert_eq!(errors[0].name, "InsufficientFunds");
		assert_eq!(errors[0].code, 0);
		assert_eq!(
			errors[0].docs,
			vec!["The sender does not have enough lamports."]
		);
	}

	#[test]
	fn numbers_implicit_discriminants_like_rust() {
		let source = r"
			struct NotAnError;
			enum Plain { Ignored = 9 }
			#[error]
			pub enum ProgramError {
				First,
				Second,
				Explicit = 6000,
				AfterExplicit,
				Hex = 0x20,
				AfterHex,
				Suffixed = 7_000u32,
				AfterSuffixed,
			}
		";

		assert_eq!(
			codes(source),
			[
				("First".to_owned(), 0),
				("Second".to_owned(), 1),
				("Explicit".to_owned(), 6000),
				("AfterExplicit".to_owned(), 6001),
				("Hex".to_owned(), 0x20),
				("AfterHex".to_owned(), 0x21),
				("Suffixed".to_owned(), 7000),
				("AfterSuffixed".to_owned(), 7001),
			]
		);
	}

	#[test]
	fn restarts_numbering_for_each_enum() {
		let source = r"
			#[error]
			enum First { A = 10, B }
			#[error]
			enum Second { C, D }
		";

		assert_eq!(
			codes(source),
			[
				("A".to_owned(), 10),
				("B".to_owned(), 11),
				("C".to_owned(), 0),
				("D".to_owned(), 1),
			]
		);
	}

	#[test]
	fn rejects_discriminants_it_cannot_evaluate() {
		for (discriminant, message) in [
			(
				"BASE + 1",
				"cannot evaluate the discriminant of `Failing::Variant`",
			),
			(
				"-1",
				"cannot evaluate the discriminant of `Failing::Variant`",
			),
			(
				"4294967296",
				"the discriminant of `Failing::Variant` does not fit u32",
			),
		] {
			let source = format!("#[error] enum Failing {{ Variant = {discriminant} }}");
			let error = extract_error_enums(&parse(&source))
				.expect_err("an unevaluable discriminant must not be guessed");

			assert!(
				error.to_string().contains(message),
				"{discriminant}: {error}"
			);
		}
	}

	#[test]
	fn rejects_an_implicit_discriminant_past_u32_max() {
		let source = "#[error] enum Overflow { Last = 4294967295, Next }";
		let error = extract_error_enums(&parse(source))
			.expect_err("an implicit discriminant past u32::MAX must fail");

		assert!(
			error
				.to_string()
				.contains("the implicit discriminant of `Overflow::Next` overflows u32")
		);
	}
}
