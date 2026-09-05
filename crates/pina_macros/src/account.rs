//! Expansion for `#[account]`.

use darling::FromMeta;
use darling::ast::NestedMeta;
use quote::format_ident;
use quote::quote;
use syn::Fields;
use syn::ItemStruct;

use crate::args::AccountArgs;
use crate::schema;
use crate::support::add_derives;
use crate::support::generate_view_helpers;
use crate::support::resolve_discriminator_variant;

pub(crate) fn expand(
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
	let header_name = format_ident!("{}Header", struct_name);
	let ref_name = format_ident!("{}Ref", struct_name);
	let mut_name = format_ident!("{}Mut", struct_name);

	let AccountArgs {
		crate_path,
		discriminator,
		variant,
		compact,
	} = args;
	let (discriminator, variant) =
		match resolve_discriminator_variant(&discriminator, variant, &struct_name) {
			Ok(v) => v,
			Err(e) => return e.to_compile_error(),
		};
	let compact = compact.is_present();
	let (schema_proofs, compact_schema) = if compact {
		match schema::validate_compact_schema(
			&item_struct,
			&crate_path,
			&discriminator,
			&header_name,
		) {
			Ok(schema) => {
				let max_size = schema.max_size;
				let element_size = schema.element_size;
				(schema.proofs, Some((max_size, element_size)))
			}
			Err(error) => return error.to_compile_error(),
		}
	} else {
		match schema::validate_fixed_schema(&item_struct, &crate_path, &discriminator, &zc_name) {
			Ok(proofs) => (proofs, None),
			Err(error) => return error.to_compile_error(),
		}
	};

	let derives = [syn::parse_quote!(#crate_path::zeropod::ZeroPod)];

	if let Err(error) = add_derives(&mut item_struct.attrs, &derives) {
		return error.to_compile_error();
	}
	if compact {
		item_struct
			.attrs
			.push(syn::parse_quote!(#[zeropod(compact)]));
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

	let error = quote!(#crate_path::ProgramError::InvalidAccountData);
	let view_helpers = if let Some((max_size, _)) = &compact_schema {
		generate_compact_view_helpers(&crate_path, &error, &ref_name, &mut_name, max_size)
	} else {
		generate_view_helpers(&crate_path, &error)
	};
	let validation_type = if compact { &header_name } else { &zc_name };
	let validation_impl = generate_validation_impl(&crate_path, validation_type);
	let account_impl = if let Some((max_size, element_size)) = compact_schema {
		quote! {
			impl #crate_path::PinaCompactAccount for #struct_name {
				type Ref<'data> = #ref_name<'data>;
				type Mut<'data> = #mut_name<'data>;

				const MAX_SIZE: usize = #max_size;
				const TAIL_ELEMENT_SIZE: usize = #element_size;

				fn try_from_bytes(
					data: &[u8],
				) -> Result<Self::Ref<'_>, #crate_path::ProgramError> {
					Self::try_from_bytes(data)
				}

				fn try_from_bytes_mut(
					data: &mut [u8],
				) -> Result<Self::Mut<'_>, #crate_path::ProgramError> {
					Self::try_from_bytes_mut(data)
				}

				fn initialize(
					data: &mut [u8],
				) -> Result<Self::Mut<'_>, #crate_path::ProgramError> {
					Self::initialize(data)
				}
			}
		}
	} else {
		quote!(impl #crate_path::PinaAccount for #struct_name {})
	};

	let implementations = quote! {
		impl #struct_name {
			#view_helpers
		}

		impl #crate_path::HasDiscriminator for #struct_name {
			type Type = #discriminator;

			const VALUE: Self::Type = #discriminator::#variant;
		}

		#validation_impl

		#account_impl
	};

	quote! {
		#item_struct
		#schema_proofs
		#implementations
	}
}

fn generate_validation_impl(
	crate_path: &syn::Path,
	validation_type: &syn::Ident,
) -> proc_macro2::TokenStream {
	quote! {
		impl #crate_path::AccountValidation for #validation_type {
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
	}
}

fn generate_compact_view_helpers(
	crate_path: &syn::Path,
	error: &proc_macro2::TokenStream,
	ref_name: &syn::Ident,
	mut_name: &syn::Ident,
	max_size: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	quote! {
		/// The fixed header size, including the discriminator and tail length prefix.
		pub const HEADER_SIZE: usize = <Self as #crate_path::ZeroPodCompact>::HEADER_SIZE;

		/// The maximum encoded size permitted by this compact schema.
		pub const MAX_SIZE: usize = #max_size;

		/// Validate and borrow a compact account view.
		pub fn try_from_bytes(data: &[u8]) -> Result<#ref_name<'_>, #crate_path::ProgramError> {
			<Self as #crate_path::PinaCompactAccount>::validate_account_data(data)?;
			#ref_name::new(data).map_err(|_| #error)
		}

		/// Validate and mutably borrow a compact account view.
		pub fn try_from_bytes_mut(
			data: &mut [u8],
		) -> Result<#mut_name<'_>, #crate_path::ProgramError> {
			<Self as #crate_path::PinaCompactAccount>::validate_account_data(data)?;
			#mut_name::new(data).map_err(|_| #error)
		}

		/// Initialize compact account storage and return its mutable view.
		pub fn initialize(data: &mut [u8]) -> Result<#mut_name<'_>, #crate_path::ProgramError> {
			<Self as #crate_path::PinaCompactAccount>::validate_size(data.len())?;
			data.fill(0);
			<Self as #crate_path::HasDiscriminator>::write_discriminator(data);
			#mut_name::new(data).map_err(|_| #error)
		}
	}
}
