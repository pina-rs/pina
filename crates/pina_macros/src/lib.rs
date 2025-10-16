use args::AccountArgs;
use args::DiscriminatorArgs;
use args::ErrorArgs;
use darling::ast::NestedMeta;
use darling::FromMeta;
use heck::ToShoutySnakeCase;
use proc_macro::TokenStream;
use quote::format_ident;
use quote::quote;
use syn::parse_macro_input;
use syn::punctuated::Punctuated;
use syn::Attribute;
use syn::Fields;
use syn::ItemEnum;
use syn::ItemStruct;
use syn::Token;

use crate::args::InstructionArgs;

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

	let enum_name = &item_enum.ident;
	let impls = quote! {
		impl ::core::convert::From<#enum_name> for #crate_path::ProgramError {
			fn from(e: #enum_name) -> Self {
				#crate_path::ProgramError::Custom(e as u32)
			}
		}
	};

	let output = quote! {
		#item_enum
		#impls
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
/// ```rust
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
/// 			_ => ::core::result::Result::Err(::pina::PinaError::InvalidDiscriminator.into()),
/// 		}
/// 	}
/// }
///
/// unsafe impl Pod for MyAccount {}
/// unsafe impl Zeroable for MyAccount {}
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
	let derives_to_add: [syn::Path; 4] = [
		syn::parse_quote!(::core::clone::Clone),
		syn::parse_quote!(::core::marker::Copy),
		syn::parse_quote!(::core::cmp::PartialEq),
		syn::parse_quote!(::core::cmp::Eq),
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
			.to_compile_error()
			.into();
		}
	}

	let implementations = quote! {
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
					_ => ::core::result::Result::Err(#crate_path::PinaError::InvalidDiscriminator.into()),
				}
			}
		}

		unsafe impl #crate_path::Zeroable for #enum_name {}
		unsafe impl #crate_path::Pod for #enum_name {}
		#crate_path::into_discriminator!(#enum_name, #primitive);
	};

	let output = quote! {
		#item_enum
		#implementations
	};

	output.into()
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
/// - `discriminator` - the discriminator enum to use for this account. The
///   variant should match the name of the account struct.
///
/// #### Codegen
///
/// It will transform the following:
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
/// #[account(crate = ::pina, discriminator = MyAccount)]
/// #[derive(Debug)]
/// pub struct ConfigState {
/// 	/// The version of the state.
/// 	pub version: u8,
/// 	/// The authority which can update this config.
/// 	pub authority: Pubkey,
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
/// #[repr(C)]
/// #[derive(
/// 	Debug,
/// 	::core::clone::Clone,
/// 	::core::marker::Copy,
/// 	::core::cmp::PartialEq,
/// 	::core::cmp::Eq,
/// 	::pina::Pod,
/// 	::pina::Zeroable,
/// 	::pina::TypedBuilder,
/// )]
/// #[bytemuck(crate = "::pina::bytemuck")]
/// #[builder(builder_method(vis = "", name = __builder))]
/// pub struct ConfigState {
/// 	// This discriminator is automatically injected as the first field in the struct. It must be
/// 	// present.
/// 	discriminator: [u8; MyAccount::BYTES],
/// 	/// The version of the state.
/// 	pub version: u8,
/// 	/// The authority which can update this config.
/// 	pub authority: Pubkey,
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
/// // This type is generated to match the `TypedBuilder` type with the
/// // discriminator already set.
/// type ConfigStateBuilderType = ConfigStateBuilder<(
/// 	([u8; MyAccount::BYTES],), /* `discriminator`: automatically applied in the builder method
/// 	                            * below. */
/// 	(), // `version`
/// 	(), // `authority`
/// 	(), // `bump`
/// 	(), // `treasury_bump`
/// 	(), // `mint_bit_bump`
/// 	(), // `mint_kibibit_bump`
/// 	(), // `mint_mebibit_bump`
/// 	(), // `mint_gibibit_bump`
/// 	(), // `game_index`
/// )>;
///
/// impl ConfigState {
/// 	pub fn to_bytes(&self) -> &[u8] {
/// 		::pina::bytemuck::bytes_of(self)
/// 	}
///
/// 	pub fn builder() -> ConfigStateBuilderType {
/// 		let mut bytes = [0u8; MyAccount::BYTES];
/// 		<Self as ::pina::HasDiscriminator>::VALUE.write_discriminator(&mut bytes);
///
/// 		Self::__builder().discriminator(bytes)
/// 	}
/// }
///
/// impl ::pina::HasDiscriminator for ConfigState {
/// 	type Type = MyAccount;
///
/// 	const VALUE: Self::Type = MyAccount::ConfigState;
/// }
///
/// impl ::pina::AccountValidation for ConfigState {
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
/// 		if !condition(self) {
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
	let nested_metas = match NestedMeta::parse_meta_list(args.into()) {
		Ok(value) => value,
		Err(e) => {
			return e.into_compile_error().into();
		}
	};

	let args = match AccountArgs::from_list(&nested_metas) {
		Ok(v) => v,
		Err(e) => {
			return e.write_errors().into();
		}
	};

	let mut item_struct = parse_macro_input!(input as ItemStruct);
	let struct_name = &item_struct.ident;
	let builder_name = format_ident!("{}Builder", struct_name);

	let AccountArgs {
		crate_path,
		discriminator,
	} = args;

	// Add #[repr(C)]
	let repr_attr: Attribute = syn::parse_quote!(#[repr(C)]);
	item_struct.attrs.push(repr_attr);

	// Add builder attribute
	let builder_attr: Attribute =
		syn::parse_quote!(#[builder(builder_method(vis = "", name = __builder))]);
	item_struct.attrs.push(builder_attr);

	// Add derive macros
	let derives_to_add: [syn::Path; 7] = [
		syn::parse_quote!(#crate_path::TypedBuilder),
		syn::parse_quote!(#crate_path::Pod),
		syn::parse_quote!(#crate_path::Zeroable),
		syn::parse_quote!(::core::clone::Clone),
		syn::parse_quote!(::core::marker::Copy),
		syn::parse_quote!(::core::cmp::PartialEq),
		syn::parse_quote!(::core::cmp::Eq),
	];

	if let Some(derive_attr) = item_struct
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
		item_struct.attrs.push(new_derive_attr);
	}

	let bytemuck_crate_str = format!(
		"{}::bytemuck",
		quote!(#crate_path).to_string().replace(' ', "")
	);
	let bytemuck_attr: Attribute = syn::parse_quote!(#[bytemuck(crate = #bytemuck_crate_str)]);
	item_struct.attrs.push(bytemuck_attr);

	// Add discriminator field
	if let Fields::Named(named_fields) = &mut item_struct.fields {
		let discriminator_field = syn::parse_quote! {
			discriminator: [u8; #discriminator::BYTES]
		};
		named_fields.named.insert(0, discriminator_field);
	} else {
		return syn::Error::new_spanned(item_struct, "Account structs must have named fields")
			.to_compile_error()
			.into();
	}

	let builder_generics = (0..item_struct.fields.len() - 1)
		.map(|_| quote! { () })
		.collect::<Vec<_>>();

	let builder_type_alias = format_ident!("{}BuilderType", struct_name);

	let implementations = quote! {
		#[allow(dead_code)]
		type #builder_type_alias = #builder_name<(
			([u8; #discriminator::BYTES],),
			#(#builder_generics,)*
		)>;

		impl #struct_name {
			pub fn to_bytes(&self) -> &[u8] {
				#crate_path::bytemuck::bytes_of(self)
			}

			pub fn builder() -> #builder_type_alias {
				let mut bytes = [0u8; #discriminator::BYTES];
				<Self as #crate_path::HasDiscriminator>::VALUE.write_discriminator(&mut bytes);

				Self::__builder().discriminator(bytes)
			}
		}

		impl #crate_path::HasDiscriminator for #struct_name {
			type Type = #discriminator;

			const VALUE: Self::Type = #discriminator::#struct_name;
		}

		impl #crate_path::AccountValidation for #struct_name {
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
				if !condition(self) {
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
	};

	let output = quote! {
		#item_struct
		#implementations
	};

	output.into()
}

/// The instruction macro is used to annotate instruction data that will exist
/// within a solana instruction.
///
/// #### Attributes
///
/// - `discriminator` - the discriminator enum to use for this instruction. The
///   variant should match the name of the instruction struct.
///
/// #### Codegen
///
/// It will transform the following:
///
/// ```rust
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
/// ```rust
/// use pina::*;
///
/// #[discriminator(crate = ::pina, primitive = u8, final)]
/// pub enum MyInstruction {
/// 	Add = 0,
/// 	FlipBit = 1,
/// }
///
/// #[repr(C)]
/// #[derive(
/// 	Debug,
/// 	::core::clone::Clone,
/// 	::core::marker::Copy,
/// 	::core::cmp::PartialEq,
/// 	::core::cmp::Eq,
/// 	::pina::Pod,
/// 	::pina::Zeroable,
/// 	::pina::TypedBuilder,
/// )]
/// #[builder(builder_method(vis = "", name = __builder))]
/// #[bytemuck(crate = "::pina::bytemuck")]
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
/// // This type is generated to match the `TypedBuilder` type with the
/// // discriminator already set.
/// type FlipBitBuilderType = FlipBitBuilder<(
/// 	([u8; MyInstruction::BYTES],), /* `discriminator`: automatically applied in the builder
/// 	                                * method below. */
/// 	(), // `section_index`
/// 	(), // `array_index`
/// 	(), // `offset`
/// 	(), // `value`
/// )>;
///
/// impl FlipBit {
/// 	pub fn to_bytes(&self) -> &[u8] {
/// 		::pina::bytemuck::bytes_of(self)
/// 	}
///
/// 	pub fn try_from_bytes(data: &[u8]) -> Result<&Self, ::pina::ProgramError> {
/// 		::pina::bytemuck::try_from_bytes::<Self>(data)
/// 			.or(Err(::pina::ProgramError::InvalidInstructionData))
/// 	}
///
/// 	pub fn builder() -> FlipBitBuilderType {
/// 		let mut bytes = [0u8; MyInstruction::BYTES];
/// 		<Self as ::pina::HasDiscriminator>::VALUE.write_discriminator(&mut bytes);
///
/// 		Self::__builder().discriminator(bytes)
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
	let nested_metas = match NestedMeta::parse_meta_list(args.into()) {
		Ok(value) => value,
		Err(e) => {
			return e.into_compile_error().into();
		}
	};

	let args = match InstructionArgs::from_list(&nested_metas) {
		Ok(v) => v,
		Err(e) => {
			return e.write_errors().into();
		}
	};

	let mut item_struct = parse_macro_input!(input as ItemStruct);
	let struct_name = &item_struct.ident;
	let builder_name = format_ident!("{}Builder", struct_name);

	let InstructionArgs {
		crate_path,
		discriminator,
	} = args;

	// Add #[repr(C)]
	let repr_attr: Attribute = syn::parse_quote!(#[repr(C)]);
	item_struct.attrs.push(repr_attr);

	// Add builder attribute
	let builder_attr: Attribute =
		syn::parse_quote!(#[builder(builder_method(vis = "", name = __builder))]);
	item_struct.attrs.push(builder_attr);

	// Add derive macros
	let derives_to_add: [syn::Path; 8] = [
		syn::parse_quote!(#crate_path::TypedBuilder),
		syn::parse_quote!(#crate_path::Pod),
		syn::parse_quote!(#crate_path::Zeroable),
		syn::parse_quote!(::core::clone::Clone),
		syn::parse_quote!(::core::marker::Copy),
		syn::parse_quote!(::core::cmp::PartialEq),
		syn::parse_quote!(::core::cmp::Eq),
		syn::parse_quote!(::core::fmt::Debug),
	];

	if let Some(derive_attr) = item_struct
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
		item_struct.attrs.push(new_derive_attr);
	}

	let bytemuck_crate_str = format!(
		"{}::bytemuck",
		quote!(#crate_path).to_string().replace(' ', "")
	);
	let bytemuck_attr: Attribute = syn::parse_quote!(#[bytemuck(crate = #bytemuck_crate_str)]);
	item_struct.attrs.push(bytemuck_attr);

	// Add discriminator field
	if let Fields::Named(named_fields) = &mut item_struct.fields {
		let discriminator_field = syn::parse_quote! {
			discriminator: [u8; #discriminator::BYTES]
		};
		named_fields.named.insert(0, discriminator_field);
	} else {
		return syn::Error::new_spanned(item_struct, "Instruction structs must have named fields")
			.to_compile_error()
			.into();
	}

	let builder_generics = (0..item_struct.fields.len() - 1)
		.map(|_| quote! { () })
		.collect::<Vec<_>>();

	let builder_type_alias = format_ident!("{}BuilderType", struct_name);

	let implementations = quote! {
		#[allow(dead_code)]
		type #builder_type_alias = #builder_name<(
			([u8; #discriminator::BYTES],),
			#(#builder_generics,)*
		)>;

		impl #struct_name {
			pub fn to_bytes(&self) -> &[u8] {
				#crate_path::bytemuck::bytes_of(self)
			}

			pub fn try_from_bytes(data: &[u8]) -> Result<&Self, #crate_path::ProgramError> {
				#crate_path::bytemuck::try_from_bytes::<Self>(data)
					.or(Err(#crate_path::ProgramError::InvalidInstructionData))
			}

			pub fn builder() -> #builder_type_alias {
				let mut bytes = [0u8; #discriminator::BYTES];
				<Self as #crate_path::HasDiscriminator>::VALUE.write_discriminator(&mut bytes);

				Self::__builder().discriminator(bytes)
			}
		}

		impl #crate_path::HasDiscriminator for #struct_name {
			type Type = #discriminator;

			const VALUE: Self::Type = #discriminator::#struct_name;
		}
	};

	let output = quote! {
		#item_struct
		#implementations
	};

	output.into()
}
