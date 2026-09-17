use syn::File;
use syn::Item;
use syn::ext::IdentExt;

use crate::error::IdlError;

/// A parsed `#[discriminator]` enum.
#[derive(Debug, Clone)]
pub struct DiscriminatorEnum {
	pub name: String,
	pub variants: Vec<DiscriminatorVariant>,
	/// The repr size in bytes (1 for u8, 2 for u16, etc.). Defaults to 1.
	pub repr_size: usize,
}

#[derive(Debug, Clone)]
pub struct DiscriminatorVariant {
	pub name: String,
	pub value: u64,
}

#[derive(Debug, Default)]
struct DiscriminatorArgs {
	primitive: Option<syn::Expr>,
}

impl syn::parse::Parse for DiscriminatorArgs {
	fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
		let mut args = Self::default();
		// Most arguments belong to the proc macro's public grammar, and IDL
		// extraction only validates their shape and uniqueness. They are
		// consumed here so the scanner accepts every documented spelling.
		let mut seen: Vec<String> = Vec::new();

		while !input.is_empty() {
			let name = input.call(syn::Ident::parse_any)?;
			let name_text = name.to_string();

			match name_text.as_str() {
				"primitive" => {
					if args.primitive.is_some() {
						return Err(syn::Error::new(
							name.span(),
							"duplicate `primitive` argument",
						));
					}
					input.parse::<syn::Token![=]>()?;
					args.primitive = Some(input.parse()?);
					seen.push(name_text);
				}
				"crate" => {
					input.parse::<syn::Token![=]>()?;
					input.parse::<syn::Path>()?;
					record_once(&mut seen, &name_text, name.span())?;
				}
				// Bare flags.
				"final" | "entrypoint" | "capacity_test" => {
					record_once(&mut seen, &name_text, name.span())?;
				}
				// `name = value` arguments the scanner validates but does not read.
				"migrations_max_lamports" | "maximum_accounts" | "program_id" => {
					input.parse::<syn::Token![=]>()?;
					input.parse::<syn::Expr>()?;
					record_once(&mut seen, &name_text, name.span())?;
				}
				"inline" => {
					input.parse::<syn::Token![=]>()?;
					input.parse::<syn::LitStr>()?;
					record_once(&mut seen, &name_text, name.span())?;
				}
				// A list of paths: `migrations(Account, ...)`.
				"migrations" => {
					let content;
					syn::parenthesized!(content in input);
					while !content.is_empty() {
						content.parse::<syn::Path>()?;
						if content.is_empty() {
							break;
						}
						content.parse::<syn::Token![,]>()?;
					}
					record_once(&mut seen, &name_text, name.span())?;
				}
				_ => {
					return Err(syn::Error::new(
						name.span(),
						format!("unknown discriminator argument `{name}`"),
					));
				}
			}

			if input.is_empty() {
				break;
			}
			input.parse::<syn::Token![,]>()?;
		}

		Ok(args)
	}
}

/// Reject a repeated argument name, mirroring the proc macro's grammar.
fn record_once(seen: &mut Vec<String>, name: &str, span: proc_macro2::Span) -> syn::Result<()> {
	if seen.iter().any(|existing| existing == name) {
		return Err(syn::Error::new(
			span,
			format!("duplicate `{name}` argument"),
		));
	}
	seen.push(name.to_owned());

	Ok(())
}

/// Parse the discriminator enum and variant used by an attribute macro.
pub(super) fn extract_discriminator_and_variant(
	attrs: &[syn::Attribute],
	attribute_name: &str,
	struct_name: &syn::Ident,
) -> Result<Option<(String, String)>, IdlError> {
	for attr in attrs {
		if !attr.path().is_ident(attribute_name) {
			continue;
		}

		let meta_list = attr
			.parse_args_with(
				syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
			)
			.map_err(|error| {
				invalid_discriminator(attribute_name, struct_name, error.to_string())
			})?;
		let mut discriminator = None;
		let mut explicit_variant = None;

		for meta in &meta_list {
			let syn::Meta::NameValue(name_value) = meta else {
				continue;
			};

			if name_value.path.is_ident("discriminator") {
				discriminator = Some(path_segments(
					&name_value.value,
					"discriminator",
					attribute_name,
					struct_name,
				)?);
			} else if name_value.path.is_ident("variant") {
				explicit_variant = Some(path_segments(
					&name_value.value,
					"variant",
					attribute_name,
					struct_name,
				)?);
			}
		}

		let discriminator = discriminator.ok_or_else(|| {
			invalid_discriminator(
				attribute_name,
				struct_name,
				"missing `discriminator` argument",
			)
		})?;
		let explicit_variant = match explicit_variant.as_deref() {
			None => None,
			Some([variant]) => Some(variant.clone()),
			Some(_) => {
				return Err(invalid_discriminator(
					attribute_name,
					struct_name,
					"`variant` must be a single identifier",
				));
			}
		};
		let (enum_segments, variant) = match explicit_variant {
			Some(variant) => (discriminator.as_slice(), variant),
			None => {
				match discriminator.as_slice() {
					[enum_name] => (core::slice::from_ref(enum_name), struct_name.to_string()),
					[.., variant] => {
						let enum_segments = &discriminator[..discriminator.len() - 1];
						(enum_segments, variant.clone())
					}
					[] => {
						return Err(invalid_discriminator(
							attribute_name,
							struct_name,
							"`discriminator` path cannot be empty",
						));
					}
				}
			}
		};
		let discriminator_enum = enum_segments.last().cloned().ok_or_else(|| {
			invalid_discriminator(
				attribute_name,
				struct_name,
				"`discriminator` path cannot be empty",
			)
		})?;

		return Ok(Some((discriminator_enum, variant)));
	}

	Ok(None)
}

fn path_segments(
	expr: &syn::Expr,
	argument_name: &str,
	attribute_name: &str,
	struct_name: &syn::Ident,
) -> Result<Vec<String>, IdlError> {
	let syn::Expr::Path(path) = expr else {
		return Err(invalid_discriminator(
			attribute_name,
			struct_name,
			format!("`{argument_name}` must be a path"),
		));
	};

	Ok(path
		.path
		.segments
		.iter()
		.map(|segment| segment.ident.to_string())
		.collect())
}

fn invalid_discriminator(
	attribute_name: &str,
	struct_name: &syn::Ident,
	message: impl core::fmt::Display,
) -> IdlError {
	IdlError::Other(format!(
		"Invalid `#[{attribute_name}]` discriminator on `{struct_name}`: {message}"
	))
}

/// Extract all `#[discriminator]` enums from a file.
///
/// # Errors
///
/// Returns an error when the attribute arguments or backing primitive do not
/// match the `#[discriminator]` macro grammar.
pub fn extract_discriminator_enums(file: &File) -> Result<Vec<DiscriminatorEnum>, IdlError> {
	let mut result = Vec::new();

	for item in &file.items {
		let Item::Enum(item_enum) = item else {
			continue;
		};
		if !has_attr(&item_enum.attrs, "discriminator") {
			continue;
		}

		let repr_size = discriminator_repr_size(item_enum)?;
		let mut variants = Vec::new();
		for variant in &item_enum.variants {
			if let Some((_, expr)) = &variant.discriminant
				&& let Some(val) = expr_to_u64(expr)
			{
				variants.push(DiscriminatorVariant {
					name: variant.ident.to_string(),
					value: val,
				});
			}
		}

		result.push(DiscriminatorEnum {
			name: item_enum.ident.to_string(),
			variants,
			repr_size,
		});
	}

	Ok(result)
}

/// Parse the backing primitive from the source-level attribute macro grammar.
fn discriminator_repr_size(item_enum: &syn::ItemEnum) -> Result<usize, IdlError> {
	let Some(attr) = item_enum
		.attrs
		.iter()
		.find(|attr| attr.path().is_ident("discriminator"))
	else {
		return Ok(1);
	};
	if matches!(attr.meta, syn::Meta::Path(_)) {
		return Ok(1);
	}
	let args = attr
		.parse_args::<DiscriminatorArgs>()
		.map_err(|error| invalid_discriminator_enum(&item_enum.ident, error))?;
	let Some(primitive) = args.primitive else {
		return Ok(1);
	};
	let syn::Expr::Path(primitive) = primitive else {
		return Err(invalid_discriminator_enum(
			&item_enum.ident,
			"`primitive` must be a primitive type path",
		));
	};
	let Some(primitive) = primitive.path.get_ident() else {
		return Err(invalid_primitive(&item_enum.ident));
	};

	Ok(match primitive.to_string().as_str() {
		"u8" => 1,
		"u16" => 2,
		"u32" => 4,
		"u64" => 8,
		_ => return Err(invalid_primitive(&item_enum.ident)),
	})
}

fn invalid_primitive(enum_name: &syn::Ident) -> IdlError {
	invalid_discriminator_enum(
		enum_name,
		"`primitive` must be one of `u8`, `u16`, `u32`, or `u64`",
	)
}

fn invalid_discriminator_enum(
	enum_name: &syn::Ident,
	message: impl core::fmt::Display,
) -> IdlError {
	IdlError::Other(format!(
		"Invalid `#[discriminator]` enum `{enum_name}`: {message}"
	))
}

fn has_attr(attrs: &[syn::Attribute], name: &str) -> bool {
	attrs.iter().any(|a| a.path().is_ident(name))
}

fn expr_to_u64(expr: &syn::Expr) -> Option<u64> {
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
	fn extracts_discriminator_enum() {
		let source = r#"
			#[discriminator]
			pub enum MyInstruction {
				Foo = 0,
				Bar = 1,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let enums =
			extract_discriminator_enums(&file).unwrap_or_else(|e| panic!("extract failed: {e}"));
		assert_eq!(enums.len(), 1);
		assert_eq!(enums[0].name, "MyInstruction");
		assert_eq!(enums[0].variants.len(), 2);
		assert_eq!(enums[0].variants[0].name, "Foo");
		assert_eq!(enums[0].variants[0].value, 0);
		assert_eq!(enums[0].variants[1].name, "Bar");
		assert_eq!(enums[0].variants[1].value, 1);
		assert_eq!(enums[0].repr_size, 1);
	}

	#[test]
	fn extracts_source_level_primitive_widths() {
		for (primitive, repr_size) in [("u16", 2), ("u32", 4), ("u64", 8)] {
			let source = format!(
				r#"
					#[discriminator(primitive = {primitive})]
					pub enum MyInstruction {{
						Foo = 0,
					}}
				"#
			);
			let file = syn::parse_file(&source).unwrap_or_else(|e| panic!("parse failed: {e}"));
			let enums = extract_discriminator_enums(&file)
				.unwrap_or_else(|e| panic!("extract failed: {e}"));

			assert_eq!(enums[0].repr_size, repr_size, "primitive {primitive}");
		}
	}

	#[test]
	fn rejects_invalid_source_level_primitive() {
		let source = r#"
			#[discriminator(primitive = u128)]
			pub enum MyInstruction {
				Foo = 0,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let error = extract_discriminator_enums(&file)
			.expect_err("unsupported primitive must fail extraction");

		assert!(error.to_string().contains("u8`, `u16`, `u32`, or `u64"));
	}
	/// Drive the argument scanner directly so every documented branch executes.
	///
	/// The argument list is interpolated into a real attribute and parsed, which
	/// is exactly how `extract_discriminator_enums` reaches this scanner.
	fn parse_discriminator_args(source: &str) -> syn::Result<DiscriminatorArgs> {
		let item_enum: syn::ItemEnum = syn::parse_str(&format!(
			"#[discriminator({source})] enum Probe {{ A = 0 }}"
		))
		.unwrap_or_else(|error| panic!("test attribute: {error}"));
		let attribute = item_enum
			.attrs
			.first()
			.unwrap_or_else(|| panic!("the probe carries one attribute"));

		attribute.parse_args::<DiscriminatorArgs>()
	}

	#[test]
	fn argument_scanner_accepts_every_documented_spelling() {
		// One enumeration covering each match arm: the primitive, the crate
		// path, the bare flags, the expression-valued arguments, the string
		// argument, and the parenthesized ladder.
		let args = parse_discriminator_args(
			r#"primitive = u8, crate = ::pina, final, capacity_test,
				migrations_max_lamports = 20_000, maximum_accounts = 255,
				program_id = ID, inline = "hint", migrations(State, ManualState)"#,
		)
		.unwrap_or_else(|error| panic!("documented grammar must parse: {error}"));

		assert!(args.primitive.is_some());
	}

	#[test]
	fn argument_scanner_accepts_the_entrypoint_flag() {
		parse_discriminator_args("entrypoint")
			.unwrap_or_else(|error| panic!("the flag must parse: {error}"));
	}

	#[test]
	fn argument_scanner_accepts_an_empty_ladder() {
		// The `migrations` arm's inner loop must terminate immediately when the
		// list is empty.
		parse_discriminator_args("migrations()")
			.unwrap_or_else(|error| panic!("empty ladder must parse: {error}"));
	}

	#[test]
	fn argument_scanner_rejects_a_duplicate_flag() {
		let error = parse_discriminator_args("final, final")
			.expect_err("a repeated argument must be rejected");

		assert!(error.to_string().contains("duplicate `final` argument"));
	}

	#[test]
	fn argument_scanner_rejects_an_unknown_argument() {
		let error = parse_discriminator_args("disptach = true")
			.expect_err("an unknown argument must be rejected");

		assert!(error.to_string().contains("unknown discriminator argument"));
	}

	#[test]
	fn entrypoint_flag_is_read_from_the_enum() {
		// The scanner's result feeds `extract_discriminator_enums`, so the
		// entrypoint spelling must survive a full extraction.
		let source = r#"
			#[discriminator(entrypoint)]
			pub enum CounterInstruction {
				Initialize = 0,
				Increment = 1,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let enums = extract_discriminator_enums(&file)
			.unwrap_or_else(|error| panic!("extraction must succeed: {error}"));

		assert_eq!(enums.len(), 1);
		assert_eq!(enums[0].variants.len(), 2);
	}
}
