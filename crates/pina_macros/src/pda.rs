//! Expansion for `#[pda]`.

use quote::format_ident;
use quote::quote;
use syn::Fields;
use syn::ItemStruct;
use syn::Type;

use crate::args::PdaArgs;
use crate::args::PdaSeedArg;

pub(crate) fn expand(
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
	let mut find_seed_params = Vec::new();
	let mut constructor_seed_params = Vec::new();
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
				let stored_expr = ty.stored_expr(name);
				let slice_expr = ty.slice_expr(name);
				let slice_expr_with_bump = ty.slice_expr_inner(name);
				let doc = format!("The `{name}` seed.");

				seed_fields.push(quote!(pub #name: #field_type));
				seed_field_docs.push(quote!(#[doc = #doc]));
				find_seed_params.push(quote!(#name: #param_type));
				constructor_seed_params.push(quote!(#name: #param_type_lt));
				seed_param_names.push(name.clone());
				seed_stored_exprs.push(stored_expr);
				seed_slice_exprs.push(slice_expr);
				seed_slice_exprs_with_bump.push(slice_expr_with_bump);
			}
		}
	}

	let seed_count = args.seeds.len();
	let seed_count_with_bump = seed_count + 1;
	let lifetime_marker = seed_fields
		.is_empty()
		.then(|| quote!(_marker: ::core::marker::PhantomData<&'a ()>,));
	let lifetime_marker_init = seed_fields
		.is_empty()
		.then(|| quote!(_marker: ::core::marker::PhantomData,));
	let seeds_doc = format!("The PDA seeds for `{struct_name}`.");
	let seeds_with_bump_doc =
		format!("The PDA seeds for `{struct_name}`, including the bump seed.");

	// The `seeds()` constructor params (with a shared lifetime) and the
	// `try_find_pda`/`find_pda`/`assert_seeds` params
	let find_params = {
		let mut params = find_seed_params.clone();
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
				#(#find_seed_params,)*
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
			#lifetime_marker
		}

		#[doc = #seeds_with_bump_doc]
		pub struct #seeds_with_bump_name<'a> {
			inner: #seeds_name<'a>,
			_bump: [u8; 1],
		}

		impl #struct_name {
			/// Build the PDA seeds for this account.
			pub fn seeds<'a>(#(#constructor_seed_params,)*) -> #seeds_name<'a> {
				#seeds_name {
					#(#seed_param_names: #seed_stored_exprs,)*
					#lifetime_marker_init
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

			/// The seeds as Pinocchio CPI seed values, including the bump seed.
			pub fn as_seed_array(&self) -> [#crate_path::Seed<'_>; #seed_count_with_bump] {
				self.as_slices().map(#crate_path::Seed::from)
			}

			/// The seeds as an owned PDA signer helper.
			pub fn to_signer(&self) -> #crate_path::PdaSigner<'_, #seed_count_with_bump> {
				#crate_path::PdaSigner::from_seed_array(self.as_seed_array())
			}
		}
	};

	quote! {
		#item_struct
		#generated
	}
}
