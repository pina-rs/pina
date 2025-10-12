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
/// ```
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
/// ```
/// #[repr(u32)]
/// #[non_exhaustive] // This is present if you haven't set the `final` flag.
/// #[derive(
/// 	::core::fmt::Debug,
/// 	::core::clone::Clone,
/// 	::core::marker::Copy,
/// 	::core::cmp::PartialEq,
/// 	::core::cmp::Eq,
/// 	::pina::IntoPrimitive, /* `IntoPrimitive` is added to the derive macros */
/// )]
/// pub enum MyError {
/// 	/// Doc comments are significant as they will be read by a future parser to
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
/// unsafe impl ::pina::Zeroable for MyError {}
/// unsafe impl ::pina::Pod for MyError {}
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

/// This attribute macro should be used for annotating the globally shared
/// instruction and account discriminators.
///
/// #### Attributes
///
/// - `primitive` - Defaults to `u8` which takes up 1 byte of space for the
///   discriminator. This would allow up to 256 variations of the type being
///   discriminated. The type can be the following:
///   - `u8` - 256 variations
///   - `u16` - 65,536 variations
///   - `u32` - 4,294,967,296 variations
///   - `u64` - 18,446,744,073,709,551,616 variations (overkill!)
/// - `crate` - this defaults to `::pina` as the developer is expected to have
///   access to the `pina` crate in the dependencies.
/// - `final` - By default all error enums are marked as `non_exhaustive`. The
///   `final` flag will remove this annotation.
///
/// The following:
///
/// ```
/// use pina::*;
///
/// #[discriminator(crate = pina, primitive = u16, final)]
/// pub enum MyAccount {
/// 	ConfigState = 0,
/// 	GameState = 1,
/// 	SectionState = 2,
/// }
/// ```
///
/// Is transformed to:
///
/// ```
/// use pina::*;
///
/// #[repr(u8)]
/// #[derive(
/// 	::core::fmt::Debug,
/// 	::core::clone::Clone,
/// 	::core::marker::Copy,
/// 	::core::cmp::PartialEq,
/// 	::core::cmp::Eq,
/// 	::pina::IntoPrimitive,
/// 	::pina::TryFromPrimitive,
/// )]
/// #[num_enum(error_type(name = ::pina::ProgramError, constructor = MyAccount::try_from_primitive_error))]
/// pub enum MyAccount {
/// 	ConfigState = 0,
/// 	GameState = 1,
/// 	SectionState = 2,
/// }
///
/// impl MyAccount {
///   fn try_from_primitive_error(_primitive: u8) ->
///     ::pina::ProgramError { ::pina::PinaError::TryFromPrimitiveError.into()
///   }
/// }
///
/// ::pina::into_discriminator!(MyAccount, u8);
/// ```
#[proc_macro_attribute]
pub fn discriminator(args: TokenStream, input: TokenStream) -> TokenStream {
	let nested_metas = match NestedMeta::parse_meta_list(args.into()) {
		Ok(value) => value,
		Err(e) => {
			return e.into_compile_error().into();
		}
	};

	let args = match DiscriminatorArgs::from_list(&nested_metas) {
		Ok(v) => v,
		Err(e) => {
			return e.write_errors().into();
		}
	};

	let mut item_enum = parse_macro_input!(input as ItemEnum);
	let enum_name = &item_enum.ident;

	let DiscriminatorArgs {
		primitive,
		crate_path,
		is_final,
	} = args;

	// Add #[repr(primitive)]
	let repr_attr: Attribute = syn::parse_quote!(#[repr(#primitive)]);
	item_enum.attrs.push(repr_attr);

	// Add #[non_exhaustive] if not final
	if !is_final.is_present() {
		let non_exhaustive_attr: Attribute = syn::parse_quote!(#[non_exhaustive]);
		item_enum.attrs.push(non_exhaustive_attr);
	}

	// Add derive macros
	let derives_to_add: [syn::Path; 7] = [
		syn::parse_quote!(::core::fmt::Debug),
		syn::parse_quote!(::core::clone::Clone),
		syn::parse_quote!(::core::marker::Copy),
		syn::parse_quote!(::core::cmp::PartialEq),
		syn::parse_quote!(::core::cmp::Eq),
		syn::parse_quote!(#crate_path::IntoPrimitive),
		syn::parse_quote!(#crate_path::TryFromPrimitive),
	];

	if let Some(derive_attr) = item_enum
		.attrs
		.iter_mut()
		.find(|attr| attr.path().is_ident("derive"))
	{
		let existing_derives_result =
			derive_attr.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated);

		if let Ok(mut existing_derives) = existing_derives_result {
			let existing_derive_names: std::collections::HashSet<String> = existing_derives
				.iter()
				.map(|p| p.segments.last().unwrap().ident.to_string())
				.collect();

			for derive_to_add in &derives_to_add {
				let to_add_name = derive_to_add.segments.last().unwrap().ident.to_string();
				if !existing_derive_names.contains(&to_add_name) {
					existing_derives.push(derive_to_add.clone());
				}
			}

			let new_derive_attr: Attribute = syn::parse_quote! {
				#[derive(#existing_derives)]
			};

			*derive_attr = new_derive_attr;
		}
	} else {
		// No derive attribute exists, so create one
		let new_derive_attr: Attribute = syn::parse_quote!(#[derive(#(#derives_to_add),*)]);
		item_enum.attrs.push(new_derive_attr);
	}

	// Add num_enum attribute
	let num_enum_attr: Attribute = syn::parse_quote! {
		#[num_enum(error_type(name = #crate_path::ProgramError, constructor = #enum_name::try_from_primitive_error))]
	};
	item_enum.attrs.push(num_enum_attr);

	let implementation = quote! {
		#crate_path::into_discriminator!(#enum_name, #primitive);

		impl #enum_name {
			fn try_from_primitive_error(_primitive: #primitive) -> #crate_path::ProgramError {
				#crate_path::PinaError::TryFromPrimitiveError.into()
			}
		}
	};

	let output = quote! {
		#item_enum
		#implementation
	};

	output.into()
}
