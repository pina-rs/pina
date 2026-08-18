use args::AccountArgs;
use args::AccountsInput;
use args::DiscriminatorArgs;
use args::ErrorArgs;
use args::EventArgs;
use args::PdaArgs;
use args::PdaSeedArg;
use darling::FromDeriveInput;
use darling::FromMeta;
use darling::ast::NestedMeta;
use darling::ast::Style;
use heck::ToShoutySnakeCase;
use proc_macro::TokenStream;
use quote::format_ident;
use quote::quote;
use syn::Attribute;
use syn::DeriveInput;
use syn::Fields;
use syn::ItemEnum;
use syn::ItemStruct;
use syn::Token;
use syn::Type;
use syn::punctuated::Punctuated;

use crate::args::InstructionArgs;

mod args;

fn add_derive(item: &mut ItemStruct, derive: syn::Path) -> syn::Result<()> {
	if let Some(attribute) = item
		.attrs
		.iter_mut()
		.find(|attribute| attribute.path().is_ident("derive"))
	{
		let mut derives =
			attribute.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)?;
		let derive_name = derive
			.segments
			.last()
			.expect("derive path has a segment")
			.ident
			.to_string();
		let is_present = derives.iter().any(|existing| {
			existing
				.segments
				.last()
				.is_some_and(|segment| segment.ident == derive_name)
		});
		if !is_present {
			derives.push(derive);
		}
		*attribute = syn::parse_quote!(#[derive(#derives)]);
	} else {
		item.attrs.push(syn::parse_quote!(#[derive(#derive)]));
	}

	Ok(())
}

/// Generates bytes-first construction and validated viewing helpers.
///
/// These helpers never turn a native schema value into bytes. Callers provide
/// initialized storage, and zeropod returns the generated zero-copy companion
/// that is allowed to observe and mutate that storage.
fn generate_view_helpers(
	crate_path: &syn::Path,
	error: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	quote! {
		/// The exact number of bytes required by the zeropod representation.
		pub const SIZE: usize = <Self as #crate_path::ZeroPodFixed>::SIZE;

		/// Validate `data` and return zeropod's immutable zero-copy companion.
		pub fn try_from_bytes(
			data: &[u8],
		) -> Result<&<Self as #crate_path::ZeroPodFixed>::Zc, #crate_path::ProgramError> {
			if data.len() != Self::SIZE
				|| !<Self as #crate_path::HasDiscriminator>::matches_discriminator(data)
			{
				return Err(#error);
			}

			<Self as #crate_path::ZeroPodFixed>::from_bytes(data).map_err(|_| #error)
		}

		/// Initialize caller-owned storage and return its mutable zero-copy view.
		///
		/// The complete slice is initialized before zeropod validates it. The
		/// returned borrow prevents the caller from observing or changing the raw
		/// bytes while the typed view is live.
		pub fn initialize(
			data: &mut [u8],
		) -> Result<&mut <Self as #crate_path::ZeroPodFixed>::Zc, #crate_path::ProgramError> {
			if data.len() != Self::SIZE {
				return Err(#error);
			}

			data.fill(0);
			<Self as #crate_path::HasDiscriminator>::write_discriminator(data);
			<Self as #crate_path::ZeroPodFixed>::from_bytes_mut(data).map_err(|_| #error)
		}
	}
}

#[cfg(test)]
mod tests;

/// Derives the `TryFromAccountInfos` trait for a named-field struct.
///
/// Fields may be `&'a AccountView`, `&'a mut AccountView`, `&'a [AccountView]`,
/// or `&'a mut [AccountView]`. One field may be annotated with
/// `#[pina(remaining)]` to capture all trailing accounts as a slice.
#[proc_macro_derive(Accounts, attributes(pina))]
pub fn accounts_derive(input: TokenStream) -> TokenStream {
	accounts_derive_impl(input.into()).into()
}

fn accounts_derive_impl(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
	// Parse input
	let input: DeriveInput = match syn::parse2(input) {
		Ok(v) => v,
		Err(e) => return e.to_compile_error(),
	};

	let args = match AccountsInput::from_derive_input(&input) {
		Ok(v) => v,
		Err(e) => return e.write_errors(),
	};

	// Extract configuration
	let struct_name = &args.ident;
	let (impl_generics, ty_generics, where_clause) = args.generics.split_for_impl();
	let crate_path = &args.crate_path;
	let fields = match args.data.take_struct() {
		Some(fields) if fields.style == Style::Struct => fields,
		Some(_) => {
			return syn::Error::new_spanned(&args.ident, "Accounts structs must have named fields")
				.to_compile_error();
		}
		None => {
			return syn::Error::new_spanned(&args.ident, "Accounts derive only supports structs")
				.to_compile_error();
		}
	};

	// Get lifetime parameter
	let lifetime = match args.generics.lifetimes().next() {
		Some(lt) => &lt.lifetime,
		None => {
			return syn::Error::new_spanned(
				&args.ident,
				"Accounts struct must have **ONE** lifetime parameter",
			)
			.to_compile_error();
		}
	};

	// Process fields
	let mut field_idents = Vec::new();
	let mut parse_fields = Vec::new();
	let mut remaining_field = None;
	let field_count = fields.len();
	let mut seen_remaining = false;

	for field in fields.iter() {
		if !field.remaining.is_present() {
			continue;
		}

		if seen_remaining {
			return syn::Error::new_spanned(
				&field.ident,
				"Only one field can be marked as `remaining`",
			)
			.to_compile_error();
		}

		seen_remaining = true;
	}

	for (index, field) in fields.iter().enumerate() {
		let ident = field
			.ident
			.as_ref()
			.unwrap_or_else(|| panic!("internal error: `Accounts` field without an ident"));

		if field.remaining.is_present() {
			if index + 1 != field_count {
				return syn::Error::new_spanned(
					&field.ident,
					"`#[pina(remaining)]` field must be the last field",
				)
				.to_compile_error();
			}

			remaining_field = field
				.ident
				.as_ref()
				.map(|ident| (ident, is_mut_reference(&field.ty)));
			continue;
		}

		field_idents.push(ident);
		let parse_field = if is_mut_reference(&field.ty) {
			quote! { let #ident = cursor.next_mut()?; }
		} else if is_reference(&field.ty) {
			quote! { let #ident = cursor.next()?; }
		} else {
			let ty = &field.ty;
			quote! { let #ident = <#ty as #crate_path::ParseAccounts>::parse_accounts(cursor)?; }
		};
		parse_fields.push(parse_field);
	}

	let finish_exact = remaining_field.is_none().then(|| {
		quote! {
			cursor.finish_exact()?;
		}
	});
	let remaining_binding = remaining_field.map(|(field, is_mut)| {
		if is_mut {
			quote! { let #field = cursor.remaining_mut()?; }
		} else {
			quote! { let #field = cursor.take_remaining(); }
		}
	});
	let remaining_field_ident = remaining_field.map(|(field, _)| quote!(#field,));

	quote! {
		impl #impl_generics #crate_path::ParseAccounts #ty_generics for #struct_name #ty_generics #where_clause {
			fn parse_accounts(
				cursor: &mut #crate_path::AccountsCursor<#lifetime>,
			) -> ::core::result::Result<Self, #crate_path::ProgramError> {
				#(#parse_fields)*
				#remaining_binding

				Ok(Self {
					#(#field_idents,)*
					#remaining_field_ident
				})
			}
		}

		impl #impl_generics #crate_path::TryFromAccountInfos #ty_generics for #struct_name #ty_generics #where_clause {
			fn try_from_account_infos(
				accounts: & #lifetime mut [#crate_path::AccountView],
			) -> ::core::result::Result<Self, #crate_path::ProgramError> {
				let mut cursor = #crate_path::AccountsCursor::new(accounts);
				let parsed = <Self as #crate_path::ParseAccounts>::parse_accounts(&mut cursor)?;
				#finish_exact

				Ok(parsed)
			}
		}

		impl #impl_generics ::core::convert::TryFrom<& #lifetime mut [#crate_path::AccountView]> for #struct_name #ty_generics #where_clause {
			type Error = #crate_path::ProgramError;

			fn try_from(accounts: & #lifetime mut [#crate_path::AccountView]) -> ::core::result::Result<Self, Self::Error> {
				<Self as #crate_path::TryFromAccountInfos>::try_from_account_infos(accounts)
			}
		}
	}
}

fn is_reference(ty: &Type) -> bool {
	matches!(ty, Type::Reference(_))
}

fn is_mut_reference(ty: &Type) -> bool {
	matches!(ty, Type::Reference(reference) if reference.mutability.is_some())
}

/// Split the final segment from a qualified `Enum::Variant` path.
fn split_discriminator_path(path: &syn::Path) -> Result<(syn::Path, syn::Ident), syn::Error> {
	let Some(variant) = path.segments.last() else {
		return Err(syn::Error::new_spanned(
			path,
			"`discriminator` path cannot be empty",
		));
	};
	let mut enum_segments = Punctuated::new();

	for segment in path.segments.iter().take(path.segments.len() - 1) {
		enum_segments.push(segment.clone());
	}

	if enum_segments.is_empty() {
		return Err(syn::Error::new_spanned(
			path,
			"`discriminator` must include an enum before its variant",
		));
	}

	Ok((
		syn::Path {
			leading_colon: path.leading_colon,
			segments: enum_segments,
		},
		variant.ident.clone(),
	))
}

/// Resolve the discriminator variant from a `discriminator` path and an
/// explicit `variant` argument.
///
/// - `discriminator = Enum::Variant` → `Variant`
/// - `discriminator = Enum, variant = Variant` → `Variant`
/// - `discriminator = Enum` → the struct name (shorthand)
fn resolve_discriminator_variant(
	discriminator: &syn::Path,
	explicit_variant: Option<syn::Ident>,
	struct_name: &syn::Ident,
) -> Result<(syn::Path, syn::Ident), syn::Error> {
	if let Some(variant) = explicit_variant {
		return Ok((discriminator.clone(), variant));
	}

	if discriminator.segments.len() == 1 {
		return Ok((discriminator.clone(), struct_name.clone()));
	}

	split_discriminator_path(discriminator)
}

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
	error_impl(args.into(), input.into()).into()
}

fn error_impl(
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
	let impls = quote! {
		impl ::core::convert::From<#enum_name> for #crate_path::ProgramError {
			fn from(e: #enum_name) -> Self {
				#crate_path::ProgramError::Custom(e as u32)
			}
		}
	};

	quote! {
		#item_enum
		#impls
	}
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
/// - `final` - By default all discriminator enums are marked as
///   `non_exhaustive`. The `final` flag will remove this annotation.
///
/// #### Codegen
///
/// The following:
///
/// ```rust
/// use pina::*;
///
/// #[discriminator(crate = ::pina, primitive = u8, final)]
/// #[derive(Debug)]
/// pub enum MyAccount {
/// 	ConfigState = 0,
/// 	GameState = 1,
/// 	SectionState = 2,
/// }
/// ```
///
/// Is transformed to:
///
/// ```ignore
/// use pina::*;
///
/// #[repr(u8)]
/// #[derive(
/// 	Debug, ::core::clone::Clone, ::core::marker::Copy, ::core::cmp::PartialEq, ::core::cmp::Eq,
/// )]
/// pub enum MyAccount {
/// 	ConfigState = 0,
/// 	GameState = 1,
/// 	SectionState = 2,
/// }
///
/// impl ::core::convert::From<MyAccount> for u8 {
/// 	#[inline]
/// 	fn from(enum_value: MyAccount) -> Self {
/// 		enum_value as Self
/// 	}
/// }
///
/// impl ::core::convert::TryFrom<u8> for MyAccount {
/// 	type Error = ::pina::ProgramError;
///
/// 	#[inline]
/// 	fn try_from(number: u8) -> ::core::result::Result<Self, ::pina::ProgramError> {
/// 		#![allow(non_upper_case_globals)]
/// 		const __CONFIG_STATE: u8 = 0;
/// 		const __GAME_STATE: u8 = 1;
/// 		const __SECTION_STATE: u8 = 2;
/// 		#[deny(unreachable_patterns)]
/// 		match number {
/// 			__CONFIG_STATE => ::core::result::Result::Ok(Self::ConfigState),
/// 			__GAME_STATE => ::core::result::Result::Ok(Self::GameState),
/// 			__SECTION_STATE => ::core::result::Result::Ok(Self::SectionState),
/// 			#[allow(unreachable_patterns)]
/// 			_ => {
/// 				::core::result::Result::Err(
/// 					::pina::PinaProgramError::InvalidDiscriminator.into(),
/// 				)
/// 			}
/// 		}
/// 	}
/// }
///
/// ::pina::into_discriminator!(MyAccount, u8);
/// ```
#[proc_macro_attribute]
pub fn discriminator(args: TokenStream, input: TokenStream) -> TokenStream {
	discriminator_impl(args.into(), input.into()).into()
}

fn discriminator_impl(
	args: proc_macro2::TokenStream,
	input: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	let nested_metas = match NestedMeta::parse_meta_list(args) {
		Ok(value) => value,
		Err(e) => return e.into_compile_error(),
	};

	let args = match DiscriminatorArgs::from_list(&nested_metas) {
		Ok(v) => v,
		Err(e) => return e.write_errors(),
	};

	let mut item_enum: ItemEnum = match syn::parse2(input) {
		Ok(v) => v,
		Err(e) => return e.to_compile_error(),
	};

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
	let derives_to_add: [syn::Path; 4] = [
		syn::parse_quote!(::core::clone::Clone),
		syn::parse_quote!(::core::marker::Copy),
		syn::parse_quote!(::core::cmp::PartialEq),
		syn::parse_quote!(::core::cmp::Eq),
	];

	let derive_attr = item_enum
		.attrs
		.iter_mut()
		.find(|attr| attr.path().is_ident("derive"));

	if let Some(derive_attr) = derive_attr {
		let existing_derives_result =
			derive_attr.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated);

		match existing_derives_result {
			Ok(mut existing_derives) => {
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
			Err(error) => return error.to_compile_error(),
		}
	} else {
		// No derive attribute exists, so create one
		let new_derive_attr: Attribute = syn::parse_quote!(#[derive(#(#derives_to_add),*)]);
		item_enum.attrs.push(new_derive_attr);
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
	for variant in &item_enum.variants {
		if let Some((_, discriminant)) = &variant.discriminant {
			let variant_name = &variant.ident;
			let const_ident =
				format_ident!("__{}", variant_name.to_string().to_shouty_snake_case());

			consts.push(quote! {
				const #const_ident: #primitive = #discriminant;
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

	quote! {
		#item_enum
		#implementations
	}
}

/// The account macro is used to annotate account data that will exist within a
/// solana account.
///
/// #### Properties
///
/// - `crate` - this defaults to `::pina` as the developer is expected to have
///   access to the `pina` crate in the dependencies. This is optional and
///   defaults to `::pina` assuming that `pina` is installed in the consuming
///   crate.
/// - `discriminator` - the discriminator enum to use for this account. May be
///   written as `Enum` (the variant defaults to the account struct name) or
///   `Enum::Variant`.
/// - `variant` - (optional) the variant of the discriminator enum to use for
///   this account. Cannot be combined with a `discriminator` path that already
///   includes a variant.
///
/// #### Codegen
///
/// It will transform the following:
///
/// ```ignore
/// use pina::*;
///
/// #[discriminator(crate = ::pina, primitive = u8, final)]
/// pub enum MyAccount {
/// 	ConfigState = 0,
/// 	GameState = 1,
/// 	SectionState = 2,
/// }
///
/// #[account(crate = ::pina, discriminator = MyAccount)]
/// #[derive(Debug)]
/// pub struct ConfigState {
/// 	/// The version of the state.
/// 	pub version: u8,
/// 	/// The authority which can update this config.
/// 	pub authority: Address,
/// 	/// Store the bump to save compute units.
/// 	pub bump: u8,
/// 	/// The treasury account bump where fees are sent and where the minted
/// 	/// tokens are transferred.
/// 	pub treasury_bump: u8,
/// 	/// The mint account bump.
/// 	pub mint_bit_bump: u8,
/// 	/// The mint account bump for KIBIBIT.
/// 	pub mint_kibibit_bump: u8,
/// 	/// The mint account bump for MEBIBIT.
/// 	pub mint_mebibit_bump: u8,
/// 	/// The mint account bump for GIBIBIT.
/// 	pub mint_gibibit_bump: u8,
/// 	/// There will be a maximum of 8 games.
/// 	pub game_index: u8,
/// }
/// ```
///
/// Into:
///
/// ```rust
/// use pina::*;
///
/// #[discriminator(crate = ::pina, primitive = u8, final)]
/// pub enum MyAccount {
/// 	ConfigState = 0,
/// 	GameState = 1,
/// 	SectionState = 2,
/// }
///
/// #[derive(Debug, ::pina::zeropod::ZeroPod)]
/// pub struct ConfigState {
/// 	// This discriminator is automatically injected as the first field in the struct. It must be
/// 	// present.
/// 	discriminator: [u8; MyAccount::BYTES],
/// 	/// The version of the state.
/// 	pub version: u8,
/// 	/// The authority which can update this config.
/// 	pub authority: Address,
/// 	/// Store the bump to save compute units.
/// 	pub bump: u8,
/// 	/// The treasury account bump where fees are sent and where the minted
/// 	/// tokens are transferred.
/// 	pub treasury_bump: u8,
/// 	/// The mint account bump.
/// 	pub mint_bit_bump: u8,
/// 	/// The mint account bump for KIBIBIT.
/// 	pub mint_kibibit_bump: u8,
/// 	/// The mint account bump for MEBIBIT.
/// 	pub mint_mebibit_bump: u8,
/// 	/// The mint account bump for GIBIBIT.
/// 	pub mint_gibibit_bump: u8,
/// 	/// There will be a maximum of 8 games.
/// 	pub game_index: u8,
/// }
///
/// impl ConfigState {
/// 	pub const SIZE: usize = <Self as ::pina::ZeroPodFixed>::SIZE;
///
/// 	pub fn try_from_bytes(data: &[u8]) -> Result<&ConfigStateZc, ::pina::ProgramError> {
/// 		// Validates the exact length, discriminator, and every zeropod field.
/// 		# unimplemented!()
/// 	}
///
/// 	pub fn initialize(data: &mut [u8]) -> Result<&mut ConfigStateZc, ::pina::ProgramError> {
/// 		// Zeroes caller-owned storage, writes the discriminator, then validates it.
/// 		# unimplemented!()
/// 	}
/// }
///
/// impl ::pina::HasDiscriminator for ConfigState {
/// 	type Type = MyAccount;
///
/// 	const VALUE: Self::Type = MyAccount::ConfigState;
/// }
///
/// impl ::pina::AccountValidation for ConfigStateZc {
/// 	#[track_caller]
/// 	fn assert<F>(&self, condition: F) -> Result<&Self, ::pina::ProgramError>
/// 	where
/// 		F: Fn(&Self) -> bool,
/// 	{
/// 		if condition(self) {
/// 			return Ok(self);
/// 		}
///
/// 		::pina::log!("Account is invalid");
/// 		::pina::log_caller();
///
/// 		Err(::pina::ProgramError::InvalidAccountData)
/// 	}
///
/// 	#[track_caller]
/// 	fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, ::pina::ProgramError>
/// 	where
/// 		F: Fn(&Self) -> bool,
/// 	{
/// 		match ::pina::assert(
/// 			condition(self),
/// 			::pina::ProgramError::InvalidAccountData,
/// 			msg,
/// 		) {
/// 			Err(err) => Err(err),
/// 			Ok(()) => Ok(self),
/// 		}
/// 	}
///
/// 	#[track_caller]
/// 	fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, ::pina::ProgramError>
/// 	where
/// 		F: Fn(&Self) -> bool,
/// 	{
/// 		if condition(self) {
/// 			return Ok(self);
/// 		}
///
/// 		::pina::log!("Account is invalid");
/// 		::pina::log_caller();
///
/// 		Err(::pina::ProgramError::InvalidAccountData)
/// 	}
///
/// 	#[track_caller]
/// 	fn assert_mut_msg<F>(
/// 		&mut self,
/// 		condition: F,
/// 		msg: &str,
/// 	) -> Result<&mut Self, ::pina::ProgramError>
/// 	where
/// 		F: Fn(&Self) -> bool,
/// 	{
/// 		match ::pina::assert(
/// 			condition(self),
/// 			::pina::ProgramError::InvalidAccountData,
/// 			msg,
/// 		) {
/// 			Err(err) => Err(err),
/// 			Ok(()) => Ok(self),
/// 		}
/// 	}
/// }
/// ```
#[proc_macro_attribute]
pub fn account(args: TokenStream, input: TokenStream) -> TokenStream {
	account_impl(args.into(), input.into()).into()
}

fn account_impl(
	args: proc_macro2::TokenStream,
	input: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	// Parse macro arguments
	let nested_metas = match NestedMeta::parse_meta_list(args) {
		Ok(value) => value,
		Err(e) => return e.into_compile_error(),
	};

	let args = match AccountArgs::from_list(&nested_metas) {
		Ok(v) => v,
		Err(e) => return e.write_errors(),
	};

	// Parse input struct
	let mut item_struct: ItemStruct = match syn::parse2(input) {
		Ok(v) => v,
		Err(e) => return e.to_compile_error(),
	};

	// Extract configuration
	let struct_name = item_struct.ident.clone();
	let zc_name = format_ident!("{}Zc", struct_name);

	let AccountArgs {
		crate_path,
		discriminator,
		variant,
	} = args;
	let (discriminator, variant) =
		match resolve_discriminator_variant(&discriminator, variant, &struct_name) {
			Ok(v) => v,
			Err(e) => return e.to_compile_error(),
		};

	// Add derive macros
	let derives_to_add: [syn::Path; 1] = [syn::parse_quote!(#crate_path::zeropod::ZeroPod)];

	let derive_attr = item_struct
		.attrs
		.iter_mut()
		.find(|attr| attr.path().is_ident("derive"));

	if let Some(derive_attr) = derive_attr {
		let existing_derives_result =
			derive_attr.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated);

		match existing_derives_result {
			Ok(mut existing_derives) => {
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
			Err(error) => return error.to_compile_error(),
		}
	} else {
		// No derive attribute exists, so create one
		let new_derive_attr: Attribute = syn::parse_quote!(#[derive(#(#derives_to_add),*)]);
		item_struct.attrs.push(new_derive_attr);
	}

	// Add discriminator field
	let Fields::Named(named_fields) = &mut item_struct.fields else {
		return syn::Error::new_spanned(item_struct, "Account structs must have named fields")
			.to_compile_error();
	};

	let discriminator_field = syn::parse_quote! {
		discriminator: [u8; #discriminator::BYTES]
	};
	named_fields.named.insert(0, discriminator_field);

	let view_helpers = generate_view_helpers(
		&crate_path,
		quote!(#crate_path::ProgramError::InvalidAccountData),
	);

	let implementations = quote! {
		impl #struct_name {
			#view_helpers
		}

		impl #crate_path::HasDiscriminator for #struct_name {
			type Type = #discriminator;

			const VALUE: Self::Type = #discriminator::#variant;
		}

		impl #crate_path::AccountValidation for #zc_name {
			#[track_caller]
			fn assert<F>(&self, condition: F) -> Result<&Self, #crate_path::ProgramError>
			where
				F: Fn(&Self) -> bool,
			{
				if condition(self) {
					return Ok(self);
				}

				#crate_path::log!("Account is invalid");
				#crate_path::log_caller();

				Err(#crate_path::ProgramError::InvalidAccountData)
			}

			#[track_caller]
			fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, #crate_path::ProgramError>
			where
				F: Fn(&Self) -> bool,
			{
				match #crate_path::assert(
					condition(self),
					#crate_path::ProgramError::InvalidAccountData,
					msg,
				) {
					Err(err) => Err(err),
					Ok(()) => Ok(self),
				}
			}

			#[track_caller]
			fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, #crate_path::ProgramError>
			where
				F: Fn(&Self) -> bool,
			{
				if condition(self) {
					return Ok(self);
				}

				#crate_path::log!("Account is invalid");
				#crate_path::log_caller();

				Err(#crate_path::ProgramError::InvalidAccountData)
			}

			#[track_caller]
			fn assert_mut_msg<F>(
				&mut self,
				condition: F,
				msg: &str,
			) -> Result<&mut Self, #crate_path::ProgramError>
			where
				F: Fn(&Self) -> bool,
			{
				match #crate_path::assert(
					condition(self),
					#crate_path::ProgramError::InvalidAccountData,
					msg,
				) {
					Err(err) => Err(err),
					Ok(()) => Ok(self),
				}
			}
		}
		impl #crate_path::PinaAccount for #struct_name {}

	};

	quote! {
		#item_struct
		#implementations
	}
}

/// The `#[pda(...)]` attribute macro declares the typed PDA seeds for an
/// `#[account]` struct and generates derivation, verification, and CPI
/// signing helpers.
///
/// #### Attributes
///
/// - `seeds` - (required) the typed PDA seed list. Each element is either a
///   byte-string literal (`b"counter"`) or a typed dynamic seed
///   (`authority: Address`). Supported types: `Address`, `u8`, `u16`, `u32`,
///   `u64`, and `[u8; N]` (with `N <= 32`). At most 16 seeds are allowed
///   before the bump seed.
/// - `bump` - (optional) the name of the field that stores the PDA bump
///   seed. When present, the generated `assert_seeds` method verifies the
///   account against the stored bump instead of searching for the canonical
///   one.
/// - `crate` - (optional) the path to the `pina` crate. Defaults to `::pina`.
///
/// #### Codegen
///
/// It will transform the following:
///
/// ```ignore
/// use pina::*;
///
/// #[account(discriminator = CounterAccount)]
/// #[pda(seeds = [b"counter", authority: Address], bump = bump)]
/// pub struct CounterState {
/// 	/// The authority whose address seeds the PDA.
/// 	pub authority: Address,
/// 	/// The PDA bump seed, stored on-chain so we don't need to re-derive it.
/// 	pub bump: u8,
/// }
/// ```
///
/// Is transformed to:
///
/// ```ignore
/// #[account(discriminator = CounterAccount)]
/// pub struct CounterState {
/// 	/// The authority whose address seeds the PDA.
/// 	pub authority: Address,
/// 	/// The PDA bump seed, stored on-chain so we don't need to re-derive it.
/// 	pub bump: u8,
/// }
///
/// /// The PDA seeds for `CounterState`.
/// pub struct CounterSeeds<'a> {
/// 	/// The `authority` seed.
/// 	pub authority: &'a Address,
/// }
///
/// /// The PDA seeds for `CounterState`, including the bump seed.
/// pub struct CounterSeedsWithBump<'a> {
/// 	inner: CounterSeeds<'a>,
/// 	_bump: [u8; 1],
/// }
///
/// impl CounterState {
/// 	/// Build the PDA seeds for this account.
/// 	pub fn seeds(authority: &Address) -> CounterSeeds<'_> {
/// 		CounterSeeds { authority }
/// 	}
///
/// 	/// Find the canonical PDA for this account and its bump seed.
/// 	pub fn try_find_pda(authority: &Address, program_id: &Address) -> Option<(Address, u8)> {
/// 		let seeds = Self::seeds(authority);
/// 		::pina::try_find_program_address(&seeds.as_slices(), program_id)
/// 	}
///
/// 	/// Find the canonical PDA for this account and its bump seed.
/// 	///
/// 	/// # Panics
/// 	///
/// 	/// Panics if no valid PDA exists for the given seeds.
/// 	pub fn find_pda(authority: &Address, program_id: &Address) -> (Address, u8) {
/// 		Self::try_find_pda(authority, program_id)
/// 			.unwrap_or_else(|| panic!("could not find program address from seeds"))
/// 	}
///
/// 	/// Assert that `account` is the PDA for the given seeds, using the
/// 	/// stored `bump` field.
/// 	pub fn assert_seeds(
/// 		account: &AccountView,
/// 		authority: &Address,
/// 		program_id: &Address,
/// 	) -> Result<(), ProgramError> {
/// 		let bump = ::pina::AsAccount::as_account::<Self>(account, program_id)?.bump;
/// 		let seeds = Self::seeds(authority).with_bump(bump);
/// 		::pina::AccountValidation::assert_seeds_with_bump(account, &seeds.as_slices(), program_id)
/// 	}
/// }
///
/// impl<'a> CounterSeeds<'a> {
/// 	/// The seeds as byte slices, without the bump seed.
/// 	pub fn as_slices(&self) -> [&[u8]; 2] {
/// 		[b"counter", self.authority.as_ref()]
/// 	}
///
/// 	/// Append the bump seed to the seeds.
/// 	pub fn with_bump(self, bump: u8) -> CounterSeedsWithBump<'a> {
/// 		CounterSeedsWithBump { inner: self, _bump: [bump] }
/// 	}
/// }
///
/// impl<'a> CounterSeedsWithBump<'a> {
/// 	/// The seeds as byte slices, including the bump seed.
/// 	pub fn as_slices(&self) -> [&[u8]; 3] {
/// 		[b"counter", self.inner.authority.as_ref(), &self._bump]
/// 	}
/// }
/// ```
#[proc_macro_attribute]
pub fn pda(args: TokenStream, input: TokenStream) -> TokenStream {
	pda_impl(args.into(), input.into()).into()
}

fn pda_impl(
	args: proc_macro2::TokenStream,
	input: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	// Parse macro arguments
	let args = match syn::parse2::<PdaArgs>(args) {
		Ok(v) => v,
		Err(e) => return e.to_compile_error(),
	};

	// Parse input struct
	let item_struct: ItemStruct = match syn::parse2(input) {
		Ok(v) => v,
		Err(e) => return e.to_compile_error(),
	};

	// Extract configuration
	let struct_name = &item_struct.ident;
	let crate_path = &args.crate_path;
	let seeds_name = format_ident!("{}Seeds", struct_name);
	let seeds_with_bump_name = format_ident!("{}SeedsWithBump", struct_name);

	// Validate the struct has named fields
	let named_fields = match &item_struct.fields {
		Fields::Named(named) => &named.named,
		_ => {
			return syn::Error::new_spanned(
				item_struct,
				"`#[pda]` can only be applied to structs with named fields",
			)
			.to_compile_error();
		}
	};

	// Validate the bump field exists and is a `u8`
	if let Some(bump_field) = &args.bump {
		let field = named_fields
			.iter()
			.find(|f| f.ident.as_ref() == Some(bump_field));
		let Some(field) = field else {
			return syn::Error::new_spanned(
				bump_field,
				format!("`bump` field `{bump_field}` was not found on `{struct_name}`"),
			)
			.to_compile_error();
		};

		let is_u8 = matches!(
			&field.ty,
			Type::Path(type_path) if type_path.path.is_ident("u8")
		);
		if !is_u8 {
			return syn::Error::new_spanned(
				&field.ty,
				format!("`bump` field `{bump_field}` must have type `u8`"),
			)
			.to_compile_error();
		}
	}

	// Build the seed struct fields, constructor params, and slice expressions
	let mut seed_fields = Vec::new();
	let mut seed_field_docs = Vec::new();
	let mut seed_params = Vec::new();
	let mut seeds_params_lt = Vec::new();
	let mut seed_param_names = Vec::new();
	let mut seed_stored_exprs = Vec::new();
	let mut seed_slice_exprs = Vec::new();
	let mut seed_slice_exprs_with_bump = Vec::new();
	let mut seed_constants = Vec::new();

	for seed in &args.seeds {
		match seed {
			PdaSeedArg::Constant(value) => {
				let lit = syn::LitByteStr::new(value, proc_macro2::Span::call_site());
				seed_constants.push(quote!(#lit));
			}
			PdaSeedArg::ConstantRef(path) => {
				seed_constants.push(quote!(#path));
			}
			PdaSeedArg::Variable { name, ty } => {
				let field_type = ty.field_type();
				let param_type = ty.param_type();
				let param_type_lt = ty.param_type_lt();
				let stored_expr = ty.to_stored_expr(name);
				let slice_expr = ty.slice_expr(name);
				let slice_expr_with_bump = ty.slice_expr_inner(name);
				let doc = format!("The `{name}` seed.");

				seed_fields.push(quote!(pub #name: #field_type));
				seed_field_docs.push(quote!(#[doc = #doc]));
				seed_params.push(quote!(#name: #param_type));
				seeds_params_lt.push(quote!(#name: #param_type_lt));
				seed_param_names.push(name.clone());
				seed_stored_exprs.push(stored_expr);
				seed_slice_exprs.push(slice_expr);
				seed_slice_exprs_with_bump.push(slice_expr_with_bump);
			}
		}
	}

	let seed_count = args.seeds.len();
	let seed_count_with_bump = seed_count + 1;
	let seeds_doc = format!("The PDA seeds for `{struct_name}`.");
	let seeds_with_bump_doc =
		format!("The PDA seeds for `{struct_name}`, including the bump seed.");

	// The `seeds()` constructor params (with a shared lifetime) and the
	// `try_find_pda`/`find_pda`/`assert_seeds` params
	let seeds_params = seeds_params_lt;
	let find_params = {
		let mut params = seed_params.clone();
		params.push(quote!(program_id: &Address));
		params
	};

	// The `assert_seeds` method (only when a bump field is declared)
	let assert_seeds = args.bump.as_ref().map(|bump_field| {
		let doc = format!(
			"Assert that `account` is the PDA for the given seeds, using the stored \
			 `{bump_field}` field."
		);
		quote! {
			#[doc = #doc]
			pub fn assert_seeds(
				account: &#crate_path::AccountView,
				#(#seed_params,)*
				program_id: &#crate_path::Address,
			) -> ::core::result::Result<(), #crate_path::ProgramError> {
				let bump = #crate_path::AsAccount::as_account::<Self>(account, program_id)?.bump;
				let seeds = Self::seeds(#(#seed_param_names,)*).with_bump(bump);
				<&#crate_path::AccountView as #crate_path::AccountInfoValidation>::assert_seeds_with_bump(
					account,
					&seeds.as_slices(),
					program_id,
				)
				.map(|_| ())
			}
		}
	});

	let generated = quote! {
		#[doc = #seeds_doc]
		#[derive(Clone, Copy)]
		pub struct #seeds_name<'a> {
			#(#seed_field_docs)*
			#(#seed_fields,)*
		}

		#[doc = #seeds_with_bump_doc]
		pub struct #seeds_with_bump_name<'a> {
			inner: #seeds_name<'a>,
			_bump: [u8; 1],
		}

		impl #struct_name {
			/// Build the PDA seeds for this account.
			pub fn seeds<'a>(#(#seeds_params,)*) -> #seeds_name<'a> {
				#seeds_name {
					#(#seed_param_names: #seed_stored_exprs,)*
				}
			}

			/// Find the canonical PDA for this account and its bump seed.
			pub fn try_find_pda(#(#find_params,)*) -> ::core::option::Option<(#crate_path::Address, u8)> {
				let seeds = Self::seeds(#(#seed_param_names,)*);
				#crate_path::try_find_program_address(&seeds.as_slices(), program_id)
			}

			/// Find the canonical PDA for this account and its bump seed.
			///
			/// # Panics
			///
			/// Panics if no valid PDA exists for the given seeds.
			pub fn find_pda(#(#find_params,)*) -> (#crate_path::Address, u8) {
				Self::try_find_pda(#(#seed_param_names,)* program_id)
					.unwrap_or_else(|| panic!("could not find program address from seeds"))
			}

			#assert_seeds
		}

		impl<'a> #seeds_name<'a> {
			/// The seeds as byte slices, without the bump seed.
			pub fn as_slices(&self) -> [&[u8]; #seed_count] {
				[#(#seed_constants,)* #(#seed_slice_exprs,)*]
			}

			/// Append the bump seed to the seeds.
			pub fn with_bump(&self, bump: u8) -> #seeds_with_bump_name<'a> {
				#seeds_with_bump_name {
					inner: *self,
					_bump: [bump],
				}
			}
		}

		impl<'a> #seeds_with_bump_name<'a> {
			/// The seeds as byte slices, including the bump seed.
			pub fn as_slices(&self) -> [&[u8]; #seed_count_with_bump] {
				[#(#seed_constants,)* #(#seed_slice_exprs_with_bump,)* &self._bump]
			}
		}
	};

	quote! {
		#item_struct
		#generated
	}
}

/// The instruction macro is used to annotate instruction data that will exist
/// within a solana instruction.
///
/// #### Attributes
///
/// - `discriminator` - the discriminator enum to use for this instruction. May
///   be written as `Enum` (the variant defaults to the instruction struct
///   name) or `Enum::Variant`.
/// - `variant` - (optional) the variant of the discriminator enum to use for
///   this instruction. Cannot be combined with a `discriminator` path that
///   already includes a variant.
///
/// #### Codegen
///
/// It will transform the following:
///
/// ```ignore
/// use pina::*;
///
/// #[discriminator(crate = ::pina, primitive = u8, final)]
/// pub enum MyInstruction {
/// 	Add = 0,
/// 	FlipBit = 1,
/// }
///
/// #[instruction(crate = ::pina, discriminator = MyInstruction)]
/// #[derive(Debug)]
/// pub struct FlipBit {
/// 	/// The data section being updated.
/// 	pub section_index: u8,
/// 	/// The index of the `u16` value in the array.
/// 	pub array_index: u8,
/// 	/// The offset of the bit being set.
/// 	pub offset: u8,
/// 	/// The value to set the bit to: `0` or `1`.
/// 	pub value: u8,
/// }
/// ```
///
/// Is transformed to:
///
/// ```ignore
/// use pina::*;
///
/// #[discriminator(crate = ::pina, primitive = u8, final)]
/// pub enum MyInstruction {
/// 	Add = 0,
/// 	FlipBit = 1,
/// }
///
/// #[derive(Debug, ::pina::zeropod::ZeroPod)]
/// pub struct FlipBit {
/// 	// This discriminator is automatically injected as the first field in the struct. It must be
/// 	// present.
/// 	discriminator: [u8; MyInstruction::BYTES],
/// 	/// The data section being updated.
/// 	pub section_index: u8,
/// 	/// The index of the `u16` value in the array.
/// 	pub array_index: u8,
/// 	/// The offset of the bit being set.
/// 	pub offset: u8,
/// 	/// The value to set the bit to: `0` or `1`.
/// 	pub value: u8,
/// }
///
/// impl FlipBit {
/// 	pub const SIZE: usize = <Self as ::pina::ZeroPodFixed>::SIZE;
///
/// 	pub fn try_from_bytes(data: &[u8]) -> Result<&FlipBitZc, ::pina::ProgramError> {
/// 		<Self as ::pina::ZeroPodFixed>::from_bytes(data)
/// 			.map_err(|_| ::pina::ProgramError::InvalidInstructionData)
/// 	}
///
/// 	pub fn initialize(data: &mut [u8]) -> Result<&mut FlipBitZc, ::pina::ProgramError> {
/// 		# unimplemented!()
/// 	}
/// }
///
/// impl ::pina::HasDiscriminator for FlipBit {
/// 	type Type = MyInstruction;
///
/// 	const VALUE: Self::Type = MyInstruction::FlipBit;
/// }
/// ```
#[proc_macro_attribute]
pub fn instruction(args: TokenStream, input: TokenStream) -> TokenStream {
	instruction_impl(args.into(), input.into()).into()
}

fn instruction_impl(
	args: proc_macro2::TokenStream,
	input: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	// Parse macro arguments
	let nested_metas = match NestedMeta::parse_meta_list(args) {
		Ok(value) => value,
		Err(e) => return e.into_compile_error(),
	};

	let args = match InstructionArgs::from_list(&nested_metas) {
		Ok(v) => v,
		Err(e) => return e.write_errors(),
	};

	// Parse input struct
	let mut item_struct: ItemStruct = match syn::parse2(input) {
		Ok(v) => v,
		Err(e) => return e.to_compile_error(),
	};

	// Extract configuration
	let struct_name = &item_struct.ident;

	let InstructionArgs {
		crate_path,
		discriminator,
		variant,
	} = args;
	let (discriminator, variant) =
		match resolve_discriminator_variant(&discriminator, variant, struct_name) {
			Ok(v) => v,
			Err(e) => return e.to_compile_error(),
		};

	// Add derive macros
	let derives_to_add: [syn::Path; 1] = [syn::parse_quote!(#crate_path::zeropod::ZeroPod)];

	let derive_attr = item_struct
		.attrs
		.iter_mut()
		.find(|attr| attr.path().is_ident("derive"));

	if let Some(derive_attr) = derive_attr {
		let existing_derives_result =
			derive_attr.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated);

		match existing_derives_result {
			Ok(mut existing_derives) => {
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
			Err(error) => return error.to_compile_error(),
		}
	} else {
		// No derive attribute exists, so create one
		let new_derive_attr: Attribute = syn::parse_quote!(#[derive(#(#derives_to_add),*)]);
		item_struct.attrs.push(new_derive_attr);
	}

	// Add discriminator field
	let Fields::Named(named_fields) = &mut item_struct.fields else {
		return syn::Error::new_spanned(item_struct, "Instruction structs must have named fields")
			.to_compile_error();
	};

	let discriminator_field = syn::parse_quote! {
		discriminator: [u8; #discriminator::BYTES]
	};
	named_fields.named.insert(0, discriminator_field);

	let view_helpers = generate_view_helpers(
		&crate_path,
		quote!(#crate_path::ProgramError::InvalidInstructionData),
	);

	let implementations = quote! {
		impl #struct_name {
			#view_helpers
		}

		impl #crate_path::HasDiscriminator for #struct_name {
			type Type = #discriminator;

			const VALUE: Self::Type = #discriminator::#variant;
		}
	};

	quote! {
		#item_struct
		#implementations
	}
}

/// The event macro is used to annotate event data that will be emitted from a
/// solana program.
///
/// #### Attributes
///
/// - `crate` - this defaults to `::pina` as the developer is expected to have
///   access to the `pina` crate in the dependencies.
/// - `discriminator` - the discriminator enum to use for this event. May be
///   written as `Enum` (the variant defaults to the event struct name) or
///   `Enum::Variant`.
/// - `variant` - (optional) the variant of the discriminator enum to use for
///   this event. Cannot be combined with a `discriminator` path that already
///   includes a variant.
///
/// #### Codegen
///
/// It will transform the following:
///
/// ```ignore
/// use pina::*;
///
/// #[discriminator(primitive = u8)]
/// pub enum Event {
/// 	Initialize = 0,
/// 	Abandon = 1,
/// }
///
/// #[event(crate = ::pina, discriminator = Event, variant = Initialize)]
/// #[derive(Debug)]
/// pub struct InitializeEvent {
/// 	pub choice: u8,
/// }
/// ```
///
/// Is transformed to:
///
/// ```ignore
/// # use pina::*;
/// # #[discriminator(primitive = u8)]
/// # pub enum Event {
/// # 	Initialize = 0,
/// # 	Abandon = 1,
/// # }
/// #[derive(Debug, ::pina::zeropod::ZeroPod)]
/// pub struct InitializeEvent {
/// 	discriminator: [u8; Event::BYTES],
/// 	pub choice: u8,
/// }
///
/// impl InitializeEvent {
/// 	pub const SIZE: usize = <Self as ::pina::ZeroPodFixed>::SIZE;
///
/// 	pub fn try_from_bytes(data: &[u8]) -> Result<&InitializeEventZc, ::pina::ProgramError> {
/// 		<Self as ::pina::ZeroPodFixed>::from_bytes(data)
/// 			.map_err(|_| ::pina::ProgramError::InvalidInstructionData)
/// 	}
///
/// 	pub fn initialize(data: &mut [u8]) -> Result<&mut InitializeEventZc, ::pina::ProgramError> {
/// 		# unimplemented!()
/// 	}
/// }
///
/// impl ::pina::HasDiscriminator for InitializeEvent {
/// 	type Type = Event;
///
/// 	const VALUE: Self::Type = Event::Initialize;
/// }
/// ```
#[proc_macro_attribute]
pub fn event(args: TokenStream, input: TokenStream) -> TokenStream {
	event_impl(args.into(), input.into()).into()
}

fn event_impl(
	args: proc_macro2::TokenStream,
	input: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	let nested_metas = match NestedMeta::parse_meta_list(args) {
		Ok(value) => value,
		Err(error) => return error.into_compile_error(),
	};
	let args = match EventArgs::from_list(&nested_metas) {
		Ok(value) => value,
		Err(error) => return error.write_errors(),
	};
	let mut item_struct: ItemStruct = match syn::parse2(input) {
		Ok(value) => value,
		Err(error) => return error.to_compile_error(),
	};

	let struct_name = item_struct.ident.clone();
	let EventArgs {
		crate_path,
		discriminator,
		variant,
	} = args;
	let (discriminator, variant) =
		match resolve_discriminator_variant(&discriminator, variant, &struct_name) {
			Ok(value) => value,
			Err(error) => return error.to_compile_error(),
		};

	if let Err(error) = add_derive(
		&mut item_struct,
		syn::parse_quote!(#crate_path::zeropod::ZeroPod),
	) {
		return error.to_compile_error();
	}

	let Fields::Named(named_fields) = &mut item_struct.fields else {
		return syn::Error::new_spanned(item_struct, "Event structs must have named fields")
			.to_compile_error();
	};
	let discriminator_field = syn::parse_quote! {
		discriminator: [u8; #discriminator::BYTES]
	};
	named_fields.named.insert(0, discriminator_field);

	let view_helpers = generate_view_helpers(
		&crate_path,
		quote!(#crate_path::ProgramError::InvalidInstructionData),
	);
	let implementations = quote! {
		impl #struct_name {
			#view_helpers
		}

		impl #crate_path::HasDiscriminator for #struct_name {
			type Type = #discriminator;

			const VALUE: Self::Type = #discriminator::#variant;
		}
	};

	quote! {
		#item_struct
		#implementations
	}
}
