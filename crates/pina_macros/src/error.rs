//! Expansion for `#[error]`.

use darling::FromMeta;
use darling::ast::NestedMeta;
use quote::quote;
use quote::quote_spanned;
use syn::Attribute;
use syn::ItemEnum;

use crate::args::ErrorArgs;

pub(crate) fn expand(
	args: proc_macro2::TokenStream,
	input: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	let nested_metas = match NestedMeta::parse_meta_list(args) {
		Ok(value) => value,
		Err(e) => return e.into_compile_error(),
	};

	let args = match ErrorArgs::from_list(&nested_metas) {
		Ok(v) => v,
		Err(e) => return e.write_errors(),
	};

	let mut item_enum: ItemEnum = match syn::parse2(input) {
		Ok(v) => v,
		Err(e) => return e.to_compile_error(),
	};

	let ErrorArgs {
		crate_path,
		is_final,
	} = args;

	// Add #[repr(u32)]
	let repr_attr: Attribute = syn::parse_quote!(#[repr(u32)]);
	item_enum.attrs.push(repr_attr);

	// Add #[non_exhaustive] if not final
	if !is_final.is_present() {
		let non_exhaustive_attr: Attribute = syn::parse_quote!(#[non_exhaustive]);
		item_enum.attrs.push(non_exhaustive_attr);
	}

	let enum_name = &item_enum.ident;
	let reserved_range_assertions = item_enum
		.variants
		.iter()
		.map(|variant| reserved_range_assertion(&crate_path, enum_name, variant));
	let impls = quote! {
		impl ::core::convert::From<#enum_name> for #crate_path::ProgramError {
			fn from(e: #enum_name) -> Self {
				#crate_path::ProgramError::Custom(e as u32)
			}
		}
	};

	quote! {
		#item_enum
		#(#reserved_range_assertions)*
		#impls
	}
}

/// Rejects a variant whose discriminant falls in the range `PinaProgramError`
/// reserves, where a client could not tell the two errors apart.
///
/// Casting the variant reads the discriminant rustc resolved, so explicit
/// values, constant expressions, and implicit auto-increments are all checked.
/// The `const _` item is evaluated at compile time and emits no code. Spanning
/// it at the variant makes the diagnostic point at the offending line.
fn reserved_range_assertion(
	crate_path: &syn::Path,
	enum_name: &syn::Ident,
	variant: &syn::Variant,
) -> proc_macro2::TokenStream {
	let variant_name = &variant.ident;
	// A variant compiled out by `cfg` has no discriminant to check, and naming
	// it would not resolve.
	let cfg_attrs = variant
		.attrs
		.iter()
		.filter(|attr| attr.path().is_ident("cfg"));

	// The quoted range mirrors `pina::RESERVED_ERROR_CODE_START`, which the
	// comparison itself reads so the boundary has one source of truth.
	// `allow(deprecated)` keeps a deprecated variant from warning at a use
	// site the author never wrote.
	quote_spanned! {variant_name.span()=>
		#(#cfg_attrs)*
		#[allow(deprecated)]
		const _: () = ::core::assert!(
			(#enum_name::#variant_name as u32) < #crate_path::RESERVED_ERROR_CODE_START,
			::core::concat!(
				"error discriminant for `",
				::core::stringify!(#enum_name),
				"::",
				::core::stringify!(#variant_name),
				"` is in the range 0xFFFF_0000..=0xFFFF_FFFF reserved for Pina's framework \
				 errors; use a value below 0xFFFF_0000"
			)
		);
	}
}

#[cfg(test)]
mod tests {
	use proc_macro2::TokenStream;

	fn expand_with(args: TokenStream, item: TokenStream) -> String {
		crate::error::expand(args, item).to_string()
	}

	/// Collapse whitespace so assertions match a token stream's normalized
	/// spacing rather than its pretty-printed form.
	fn squeezed(tokens: &str) -> String {
		tokens.split_whitespace().collect()
	}

	#[test]
	fn every_variant_is_checked_against_the_reserved_range() {
		let expanded = squeezed(&expand_with(
			quote::quote!(),
			quote::quote!(
				pub enum MyError {
					Explicit = 6000,
					Implicit,
				}
			),
		));

		for variant in ["Explicit", "Implicit"] {
			assert!(
				expanded.contains(&format!(
					"(MyError::{variant}asu32)<::pina::RESERVED_ERROR_CODE_START"
				)),
				"`{variant}` must be compared with the shared boundary: {expanded}"
			);
		}
		assert_eq!(expanded.matches("const_:()=").count(), 2);
	}

	#[test]
	fn the_assertion_reads_the_boundary_through_the_crate_path() {
		let expanded = squeezed(&expand_with(
			quote::quote!(crate = pina),
			quote::quote!(
				pub enum MyError {
					Invalid = 0,
				}
			),
		));

		assert!(
			expanded.contains("<pina::RESERVED_ERROR_CODE_START"),
			"the boundary must resolve through `crate = pina`: {expanded}"
		);
	}

	#[test]
	fn the_assertion_inherits_the_variant_cfg_and_tolerates_deprecation() {
		let expanded = squeezed(&expand_with(
			quote::quote!(),
			quote::quote!(
				pub enum MyError {
					#[cfg(feature = "extra")]
					#[deprecated]
					#[doc = "Only with `extra`."]
					Extra = 1,
				}
			),
		));

		assert!(
			expanded.contains("#[cfg(feature=\"extra\")]#[allow(deprecated)]const_:()="),
			"the assertion must share the variant's `cfg`: {expanded}"
		);
		assert_eq!(
			expanded.matches("#[deprecated]").count(),
			1,
			"only the variant's `cfg` carries over: {expanded}"
		);
		assert_eq!(expanded.matches("Onlywith").count(), 1);
	}
}
