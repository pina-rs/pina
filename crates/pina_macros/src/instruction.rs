//! Expansion for `#[instruction]`.

use darling::FromMeta;
use darling::ast::NestedMeta;
use quote::format_ident;
use quote::quote;
use syn::Fields;
use syn::ItemStruct;

use crate::args::InstructionArgs;
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
	let zc_name = format_ident!("{}Zc", struct_name);

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
		return syn::Error::new_spanned(item_struct, "Instruction structs must have named fields")
			.to_compile_error();
	};

	let discriminator_field = syn::parse_quote! {
		discriminator: [u8; #discriminator::BYTES]
	};
	named_fields.named.insert(0, discriminator_field);

	let view_helpers = generate_view_helpers(
		&crate_path,
		&quote!(#crate_path::ProgramError::InvalidInstructionData),
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
		#schema_proofs
		#implementations
	}
}
