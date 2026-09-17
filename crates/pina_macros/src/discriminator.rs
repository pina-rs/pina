//! Expansion for `#[discriminator]`.

use darling::FromMeta;
use darling::ast::NestedMeta;
use heck::ToShoutySnakeCase;
use quote::format_ident;
use quote::quote;
use syn::Attribute;
use syn::ItemEnum;

use crate::args::DiscriminatorArgs;
use crate::support::add_derives;

pub(crate) fn expand(
	args: proc_macro2::TokenStream,
	input: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	let nested_metas = match NestedMeta::parse_meta_list(args) {
		Ok(value) => value,
		Err(e) => return e.into_compile_error(),
	};

	let args = match DiscriminatorArgs::from_list(&nested_metas) {
		Ok(v) => v,
		Err(error) => {
			let reason = crate::validation::darling_reason(&error);

			return syn::Error::new(
				error.span(),
				format!(
					"could not parse the `#[discriminator(...)]` input: {reason} Supported \
					 arguments are `primitive = u8 | u16 | u32 | u64`, `crate = path`, `final`, \
					 `entrypoint`, `migrations(Account, ...)`, `migrations_max_lamports = EXPR`, \
					 `capacity_test`, `maximum_accounts = EXPR`, `program_id = EXPR`, and `inline \
					 = \"always\" | \"hint\"`"
				),
			)
			.to_compile_error();
		}
	};

	let mut item_enum: ItemEnum = match syn::parse2(input) {
		Ok(v) => v,
		Err(e) => return e.to_compile_error(),
	};

	let enum_name = item_enum.ident.clone();

	let DiscriminatorArgs {
		primitive,
		crate_path,
		is_final,
		entrypoint,
		..
	} = &args;

	// Add #[repr(primitive)]
	let repr_attr: Attribute = syn::parse_quote!(#[repr(#primitive)]);
	item_enum.attrs.push(repr_attr);

	// Add #[non_exhaustive] if not final
	if !is_final.is_present() {
		let non_exhaustive_attr: Attribute = syn::parse_quote!(#[non_exhaustive]);
		item_enum.attrs.push(non_exhaustive_attr);
	}

	// The entrypoint is generated before the enum is finalized so its
	// diagnostics point at the enum and its variants.
	let entrypoint_expansion = if entrypoint.is_present() {
		match crate::entrypoint::expand(&args, &mut item_enum) {
			Ok(value) => Some(value),
			Err(error) => return error.to_compile_error(),
		}
	} else {
		None
	};

	let derives = [
		syn::parse_quote!(::core::clone::Clone),
		syn::parse_quote!(::core::marker::Copy),
		syn::parse_quote!(::core::cmp::PartialEq),
		syn::parse_quote!(::core::cmp::Eq),
	];

	if let Err(error) = add_derives(&mut item_enum.attrs, &derives) {
		return error.to_compile_error();
	}

	let primitive_size = primitive.byte_size().to_string();
	let primitive_width_assertion = quote! {
		const _: () = {
			::core::assert!(
				::core::mem::size_of::<#primitive>()
					<= #crate_path::MAX_DISCRIMINATOR_SPACE,
				concat!(
					"A discriminator with primitive `",
					stringify!(#primitive),
					"` (",
					#primitive_size,
					" bytes) exceeds `MAX_DISCRIMINATOR_SPACE` and cannot be safely used for zero-copy layouts. Supported primitives: `u8`, `u16`, `u32`, `u64`."
				)
			);
		};
	};

	let mut consts = Vec::new();
	let mut match_arms = Vec::new();
	let mut reserved_assertions = Vec::new();
	for variant in &item_enum.variants {
		if let Some((_, discriminant)) = &variant.discriminant {
			let variant_name = &variant.ident;
			let const_ident =
				format_ident!("__{}", variant_name.to_string().to_shouty_snake_case());

			consts.push(quote! {
				const #const_ident: #primitive = #discriminant;
			});

			// The all-ones discriminator belongs to the framework's reserved
			// `Migrate` instruction. Const evaluation catches every spelling
			// of the value, not just the literal.
			reserved_assertions.push(quote! {
				const _: () = {
					::core::assert!(
						#const_ident != !0,
						concat!(
							"discriminator value for `",
							stringify!(#variant_name),
							"` is the all-ones value reserved by Pina for the framework `Migrate` \
							 instruction; choose another value"
						)
					);
				};
			});

			match_arms.push(quote! {
				#const_ident => ::core::result::Result::Ok(Self::#variant_name),
			});
		} else {
			return syn::Error::new_spanned(
				variant,
				"Enum variant for discriminator must have an explicit value.",
			)
			.to_compile_error();
		}
	}

	let implementations = quote! {
		#primitive_width_assertion

		impl ::core::convert::From<#enum_name> for #primitive {
			#[inline]
			fn from(enum_value: #enum_name) -> Self {
				enum_value as Self
			}
		}

		impl ::core::convert::TryFrom<#primitive> for #enum_name {
			type Error = #crate_path::ProgramError;

			#[inline]
			fn try_from(number: #primitive) -> ::core::result::Result<Self, #crate_path::ProgramError> {
				#![allow(non_upper_case_globals)]
				#(#consts)*
				#(#reserved_assertions)*
				#[deny(unreachable_patterns)]
				match number {
					#(#match_arms)*
					#[allow(unreachable_patterns)]
					_ => ::core::result::Result::Err(#crate_path::PinaProgramError::InvalidDiscriminator.into()),
				}
			}
		}

		#crate_path::into_discriminator!(#enum_name, #primitive);
	};

	let (uniqueness_marker, entrypoint_impl) = match entrypoint_expansion {
		Some(expansion) => (expansion.uniqueness_marker, expansion.implementation),
		None => (quote! {}, quote! {}),
	};

	quote! {
		#uniqueness_marker
		#item_enum
		#implementations
		#entrypoint_impl
	}
}
