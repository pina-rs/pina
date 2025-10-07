use proc_macro::TokenStream;
use quote::quote;
use syn::Attribute;
use syn::ItemEnum;
use syn::LitBool;
use syn::Token;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse_macro_input;
use syn::punctuated::Punctuated;

struct ErrorArgs {
	crate_path: Option<syn::Path>,
	is_final: Option<LitBool>,
}

impl Parse for ErrorArgs {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut crate_path = None;
		let mut is_final = None;

		let metas = Punctuated::<syn::Meta, Token![,]>::parse_terminated(input)?;

		for meta in metas {
			if meta.path().is_ident("crate_path") {
				if let syn::Meta::NameValue(nv) = meta
					&& let syn::Expr::Path(expr_path) = nv.value
				{
					crate_path = Some(expr_path.path);
				}

				continue;
			}

			if meta.path().is_ident("final") {
				if let syn::Meta::NameValue(nv) = meta
					&& let syn::Expr::Lit(lit) = nv.value
					&& let syn::Lit::Bool(lit_bool) = lit.lit
				{
					is_final = Some(lit_bool);
				}
			}
		}

		Ok(Self {
			crate_path,
			is_final,
		})
	}
}

/// `#[error]` is a lightweight modification to the provided enum acting as
/// syntactic sugar to make it easier to manage your custom program errors.
///
/// ```rust
/// use pina::*;
///
/// #[error(crate_path = ::pina, final = false)]
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// pub enum MyError {
/// 	/// Doc comments are significant as they will be read by a future parser to
/// 	/// generate the IDL.
/// 	Invalid = 0,
/// 	/// A duplicate issue has occurred.
/// 	Duplicate = 1,
/// }
/// ```
///
/// The above is transformed into:
///
/// ```rust
/// #[repr(u32)]
/// #[non_exhaustive] // This is present if you haven't set the attribute `final` or it is set to false.
/// #[derive(
/// 	::core::fmt::macros::Debug,
/// 	::core::clone::Clone,
/// 	::core::marker::Copy,
/// 	::core::cmp::PartialEq,
/// 	::core::cmp::Eq,
/// 	::pina::IntoPrimitive, /* `IntoPrimitive` is added to the derive macros */
/// )]
/// pub enum MyError {
/// 	/// Doc comments are significant as they will be read by a future parse to
/// 	/// generte the IDL.
/// 	Invalid = 0,
/// 	/// A duplicate issue has occurred.
/// 	Duplicate = 1,
/// }
///
/// impl ::core::convert::From<MyError> for ::pina::ProgramError {
/// 	fn from(e: MyError) -> Self {
/// 		::pina::ProgramError::Custom(e as u32)
/// 	}
/// }
/// ```
///
/// #### Properties
///
/// - `crate_path` - this defaults to `::pina` as the developer is expected to
///   have access to the `pina` crate in the dependencies.
///
/// - `final` - By default all error enums are marked as `non_exhaustive`. The
///   `final` attribute will remove this. This attribute is optional.
#[proc_macro_attribute]
pub fn error(args: TokenStream, input: TokenStream) -> TokenStream {
	let args = parse_macro_input!(args as ErrorArgs);
	let mut item_enum = parse_macro_input!(input as ItemEnum);

	let crate_path = args
		.crate_path
		.unwrap_or_else(|| syn::parse_str("::pina").unwrap());
	let is_final = args.is_final.is_some_and(|lit| lit.value);

	// Add #[repr(u32)]
	let repr_attr: Attribute = syn::parse_quote!(#[repr(u32)]);
	item_enum.attrs.push(repr_attr);

	// Add #[non_exhaustive] if not final
	if !is_final {
		let non_exhaustive_attr: Attribute = syn::parse_quote!(#[non_exhaustive]);
		item_enum.attrs.push(non_exhaustive_attr);
	}

	// Add IntoPrimitive to derive macros
	let into_primitive_path: syn::Path = syn::parse_quote!(#crate_path::IntoPrimitive);
	let pod_path: syn::Path = syn::parse_quote!(#crate_path::Pod);
	let zeroable_path: syn::Path = syn::parse_quote!(#crate_path::Zeroable);

	if let Some(derive_attr) = item_enum
		.attrs
		.iter_mut()
		.find(|attr| attr.path().is_ident("derive"))
	{
		// Get the existing derives
		let existing_derives =
			derive_attr.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated);

		if let Ok(mut existing_derives) = existing_derives {
			// Add our new derive
			existing_derives.push(into_primitive_path);
			existing_derives.push(pod_path);
			existing_derives.push(zeroable_path);

			// Create the new derive attribute
			let new_derive_attr: Attribute = syn::parse_quote! {
				#[derive(#existing_derives)]
			};

			// Replace the old attribute
			*derive_attr = new_derive_attr;
		}
	} else {
		// No derive attribute exists, so create one
		let new_derive_attr: Attribute =
			syn::parse_quote!(#[derive(#into_primitive_path, #pod_path, #zeroable_path)]);
		item_enum.attrs.push(new_derive_attr);
	}

	// let bytemuck_attr: Attribute = syn::parse_quote!(#[bytemuck(crate = )])
	// item_enum.attrs.push(value);

	let enum_name = &item_enum.ident;

	let from_impl = quote! {
		impl ::core::convert::From<#enum_name> for #crate_path::ProgramError {
			fn from(e: #enum_name) -> Self {
				#crate_path::ProgramError::Custom(e as u32)
			}
		}
	};

	let output = quote! {
		#item_enum
		#from_impl
	};

	output.into()
}
