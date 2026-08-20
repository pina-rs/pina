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
	let schema_proofs =
		match schema::validate_fixed_schema(&item_struct, &crate_path, &discriminator, &zc_name) {
			Ok(proofs) => proofs,
			Err(error) => return error.to_compile_error(),
		};

	let derives = [syn::parse_quote!(#crate_path::zeropod::ZeroPod)];

	if let Err(error) = add_derives(&mut item_struct.attrs, &derives) {
		return error.to_compile_error();
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
		&quote!(#crate_path::ProgramError::InvalidAccountData),
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
		#schema_proofs
		#implementations
	}
}
