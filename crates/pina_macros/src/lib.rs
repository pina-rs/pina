use args::DiscriminatorArgs;
use args::ErrorArgs;
use darling::ast::NestedMeta;
use darling::FromMeta;
use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;
use syn::punctuated::Punctuated;
use syn::Attribute;
use syn::ItemEnum;
use syn::Token;

mod args;

/// `#[error]` is a lightweight modification to the provided enum acting as
/// syntactic sugar to make it easier to manage your custom program errors.
///
/// ```rust
/// use pina::*;
///
/// #[error(crate = ::pina)]
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
/// #[non_exhaustive] // This is present if you haven't set the `final` flag.
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
///
/// unsafe impl Zeroable for MyError {}
/// unsafe impl Pod for MyError {}
/// ```
///
/// #### Properties
///
/// - `crate` - this defaults to `::pina` as the developer is expected to have
///   access to the `pina` crate in the dependencies. This is optional and
///   defaults to `::pina` assuming that `pina` is installed in the consuming
///   crate.
///
/// - `final` - By default all error enums are marked as `non_exhaustive`. The
///   `final` flag will remove this.
#[proc_macro_attribute]
pub fn error(args: TokenStream, input: TokenStream) -> TokenStream {
	let nested_metas = match NestedMeta::parse_meta_list(args.into()) {
		Ok(value) => value,
		Err(e) => {
			return e.into_compile_error().into();
		}
	};

	let args = match ErrorArgs::from_list(&nested_metas) {
		Ok(v) => v,
		Err(e) => {
			return e.write_errors().into();
		}
	};

	let mut item_enum = parse_macro_input!(input as ItemEnum);

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

	// Add IntoPrimitive to derive macros
	let into_primitive_path: syn::Path = syn::parse_quote!(#crate_path::IntoPrimitive);

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

			// Create the new derive attribute
			let new_derive_attr: Attribute = syn::parse_quote! {
				#[derive(#existing_derives)]
			};

			// Replace the old attribute
			*derive_attr = new_derive_attr;
		}
	} else {
		// No derive attribute exists, so create one
		let new_derive_attr: Attribute = syn::parse_quote!(#[derive(#into_primitive_path)]);
		item_enum.attrs.push(new_derive_attr);
	}

	let enum_name = &item_enum.ident;

	let implementations = quote! {
		impl ::core::convert::From<#enum_name> for #crate_path::ProgramError {
			fn from(e: #enum_name) -> Self {
				#crate_path::ProgramError::Custom(e as u32)
			}
		}

		unsafe impl #crate_path::Zeroable for #enum_name {}
		unsafe impl #crate_path::Pod for #enum_name {}
	};

	let output = quote! {
		#item_enum
		#implementations
	};

	output.into()
}
