//! Expansion for `#[event]`.

use darling::FromMeta;
use darling::ast::NestedMeta;
use quote::format_ident;
use quote::quote;
use syn::Fields;
use syn::ItemStruct;

use crate::args::EventArgs;
use crate::schema;
use crate::support::add_derives;
use crate::support::generate_view_helpers;
use crate::support::resolve_discriminator_variant;
use crate::validation;
#[cfg(feature = "validation")]
use crate::validation::ValueTarget;

pub(crate) fn expand(
	args: proc_macro2::TokenStream,
	input: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	let nested_metas = match NestedMeta::parse_meta_list(args) {
		Ok(value) => value,
		Err(error) => return error.into_compile_error(),
	};
	let args = match EventArgs::from_list(&nested_metas) {
		Ok(value) => value,
		Err(error) => return validation::attribute_error(&error, "event"),
	};
	let mut item_struct: ItemStruct = match syn::parse2(input) {
		Ok(value) => value,
		Err(error) => return error.to_compile_error(),
	};
	let field_validations = match validation::take_value_validations(&mut item_struct) {
		Ok(value) => value,
		Err(error) => return error.to_compile_error(),
	};

	let struct_name = item_struct.ident.clone();
	let zc_name = format_ident!("{}Zc", struct_name);
	let EventArgs {
		crate_path,
		discriminator,
		variant,
		validate,
	} = args;
	#[cfg(not(feature = "validation"))]
	if validation::validation_requested(&field_validations, validate.as_ref()) {
		return validation::feature_error(&item_struct);
	}
	let (discriminator, variant) =
		match resolve_discriminator_variant(&discriminator, variant, &struct_name) {
			Ok(value) => value,
			Err(error) => return error.to_compile_error(),
		};
	let schema_proofs =
		match schema::validate_fixed_schema(&item_struct, &crate_path, &discriminator, &zc_name) {
			Ok(proofs) => proofs,
			Err(error) => return error.to_compile_error(),
		};

	let derives = [syn::parse_quote!(#crate_path::pinapod::PinaPod)];

	if let Err(error) = add_derives(&mut item_struct.attrs, &derives) {
		return error.to_compile_error();
	}
	item_struct
		.attrs
		.push(syn::parse_quote!(#[pinapod(crate = #crate_path::pinapod, no_inherent)]));

	let Fields::Named(named_fields) = &mut item_struct.fields else {
		return syn::Error::new_spanned(item_struct, "Event structs must have named fields")
			.to_compile_error();
	};
	let discriminator_field = syn::parse_quote! {
		#[pinapod(skip_accessor)]
		discriminator: [u8; #discriminator::BYTES]
	};
	named_fields.named.insert(0, discriminator_field);

	let view_helpers = generate_view_helpers(
		&crate_path,
		&quote!(#crate_path::ProgramError::InvalidInstructionData),
		false,
	);
	#[cfg(feature = "validation")]
	let value_validation_impl = validation::generate_value_validation(
		&crate_path,
		ValueTarget::Fixed(&zc_name),
		&field_validations,
		validate.as_ref(),
		&quote!(#crate_path::ProgramError::InvalidInstructionData),
	);
	#[cfg(not(feature = "validation"))]
	let value_validation_impl = quote! {};
	let implementations = quote! {
		impl #struct_name {
			#view_helpers
		}

		impl #crate_path::HasDiscriminator for #struct_name {
			type Type = #discriminator;

			const VALUE: Self::Type = #discriminator::#variant;
		}

		#value_validation_impl
	};

	quote! {
		#item_struct
		#schema_proofs
		#implementations
	}
}
