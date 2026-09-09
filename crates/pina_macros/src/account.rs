//! Expansion for `#[account]`.

use darling::FromMeta;
use darling::ast::NestedMeta;
use quote::format_ident;
use quote::quote;
use syn::Fields;
use syn::ItemStruct;

use crate::args::AccountArgs;
use crate::migration::MigrationExpansion;
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
	// Parse macro arguments
	let nested_metas = match NestedMeta::parse_meta_list(args) {
		Ok(value) => value,
		Err(e) => return e.into_compile_error(),
	};

	let args = match AccountArgs::from_list(&nested_metas) {
		Ok(v) => v,
		Err(error) => return validation::attribute_error(&error, "account"),
	};

	// Parse input struct
	let mut item_struct: ItemStruct = match syn::parse2(input) {
		Ok(v) => v,
		Err(e) => return e.to_compile_error(),
	};
	let field_validations = match validation::take_value_validations(&mut item_struct) {
		Ok(value) => value,
		Err(error) => return error.to_compile_error(),
	};

	// Extract configuration
	let struct_name = item_struct.ident.clone();
	let zc_name = format_ident!("{}Zc", struct_name);
	let header_name = format_ident!("{}Header", struct_name);
	let ref_name = format_ident!("{}Ref", struct_name);
	let patch_name = format_ident!("{}Patch", struct_name);

	let AccountArgs {
		crate_path,
		discriminator,
		variant,
		compact,
		migrations,
		validate,
	} = args;
	#[cfg(not(feature = "validation"))]
	if validation::validation_requested(&field_validations, validate.as_ref()) {
		return validation::feature_error(&item_struct);
	}
	let (discriminator, variant) =
		match resolve_discriminator_variant(&discriminator, variant, &struct_name) {
			Ok(v) => v,
			Err(e) => return e.to_compile_error(),
		};
	let compact = compact.is_present();
	let migration = if migrations.is_present() {
		match MigrationExpansion::load(
			&item_struct,
			pina_abi::ContractKind::Account,
			if compact {
				pina_abi::LayoutKind::Compact
			} else {
				pina_abi::LayoutKind::Fixed
			},
		) {
			Ok(value) => Some(value),
			Err(error) => return error.to_compile_error(),
		}
	} else {
		None
	};
	let migration_bytes = migration
		.as_ref()
		.map_or(0, MigrationExpansion::version_bytes);
	#[cfg(not(feature = "compact"))]
	if compact {
		return syn::Error::new_spanned(
			&item_struct.ident,
			"compact account support requires enabling the `compact` feature",
		)
		.to_compile_error();
	}
	let (schema_proofs, compact_schema) = if compact {
		match schema::validate_compact_schema(
			&item_struct,
			&crate_path,
			&discriminator,
			&header_name,
			migration_bytes,
		) {
			Ok(schema) => (schema.proofs.clone(), Some(schema)),
			Err(error) => return error.to_compile_error(),
		}
	} else {
		match schema::validate_fixed_schema(
			&item_struct,
			&crate_path,
			&discriminator,
			&zc_name,
			migration_bytes,
		) {
			Ok(proofs) => (proofs, None),
			Err(error) => return error.to_compile_error(),
		}
	};

	let derives = [syn::parse_quote!(#crate_path::pinapod::PinaPod)];

	if let Err(error) = add_derives(&mut item_struct.attrs, &derives) {
		return error.to_compile_error();
	}
	item_struct
		.attrs
		.push(syn::parse_quote!(#[pinapod(crate = #crate_path::pinapod, no_inherent)]));
	if compact {
		item_struct
			.attrs
			.push(syn::parse_quote!(#[pinapod(compact)]));
	}

	// Add discriminator field
	let Fields::Named(named_fields) = &mut item_struct.fields else {
		return syn::Error::new_spanned(item_struct, "Account structs must have named fields")
			.to_compile_error();
	};

	let discriminator_field = syn::parse_quote! {
		#[pinapod(skip_accessor, skip_patch)]
		discriminator: [u8; #discriminator::BYTES]
	};
	named_fields.named.insert(0, discriminator_field);
	if let Some(migration) = &migration {
		named_fields.named.insert(1, migration.field(true));
	}

	let error = quote!(#crate_path::ProgramError::InvalidAccountData);
	let view_helpers = if let Some(schema) = &compact_schema {
		generate_compact_view_helpers(
			&crate_path,
			&error,
			&ref_name,
			&patch_name,
			schema,
			migration.as_ref(),
		)
	} else {
		generate_view_helpers(&crate_path, &error, true, migration.as_ref())
	};
	let validation_type = if compact { &header_name } else { &zc_name };
	let validation_impl = generate_validation_impl(&crate_path, validation_type);
	#[cfg(feature = "validation")]
	let value_validation_impl = validation::generate_value_validation(
		&crate_path,
		compact_schema.as_ref().map_or_else(
			|| ValueTarget::Fixed(&zc_name),
			|schema| {
				ValueTarget::Compact {
					target: &ref_name,
					tails: &schema.tails,
				}
			},
		),
		&field_validations,
		validate.as_ref(),
		&quote!(#crate_path::ProgramError::InvalidAccountData),
	);
	#[cfg(not(feature = "validation"))]
	let value_validation_impl = quote! {};
	#[cfg(feature = "validation")]
	let application_validation_hook = (!compact).then(|| {
		quote! {
			fn validate_account_value(value: &Self::Zc) -> #crate_path::ProgramResult {
				<#zc_name as #crate_path::PinaValidate>::validate(value)
			}
		}
	});
	#[cfg(not(feature = "validation"))]
	let application_validation_hook: Option<proc_macro2::TokenStream> = None;
	#[cfg(feature = "validation")]
	let compact_validation_hook = quote! {
		fn validate_account_data(data: &[u8]) -> Result<(), #crate_path::ProgramError> {
			Self::try_from_bytes(data).map(|_| ())
		}
	};
	#[cfg(not(feature = "validation"))]
	let compact_validation_hook = quote! {};
	let account_impl = if compact {
		quote! {
			impl #crate_path::PinaCompactAccount for #struct_name {
				type Ref<'data> = #ref_name<'data>;
				type Patch<'patch> = #patch_name<'patch>;

				fn try_from_bytes(
					data: &[u8],
				) -> Result<Self::Ref<'_>, #crate_path::ProgramError> {
					Self::try_from_bytes(data)
				}

				#compact_validation_hook

				fn updated_len(
					data: &[u8],
					patch: &Self::Patch<'_>,
				) -> Result<usize, #crate_path::ProgramError> {
					Self::updated_len(data, patch)
				}

				fn update(
					data: &mut [u8],
					patch: &Self::Patch<'_>,
				) -> Result<usize, #crate_path::ProgramError> {
					Self::update(data, patch)
				}

				fn initialize(
					data: &mut [u8],
					patch: &Self::Patch<'_>,
				) -> Result<usize, #crate_path::ProgramError> {
					Self::initialize(data, patch)
				}
			}

			impl<'__pina_patch> #crate_path::PinaCompactPatch<#struct_name>
				for #patch_name<'__pina_patch>
			{
				#[inline(always)]
				fn as_pina_patch(
					&self,
				) -> &<#struct_name as #crate_path::PinaCompactAccount>::Patch<'_> {
					self
				}
			}

		}
	} else {
		let write_version = migration.as_ref().map(MigrationExpansion::write_zc_version);
		quote! {
			impl #crate_path::PinaAccount for #struct_name {
				#application_validation_hook

				fn write_zc_discriminator(
					value: &mut <Self as #crate_path::PinaPodFixed>::Zc,
				) {
					<Self as #crate_path::HasDiscriminator>::write_discriminator(
						&mut value.discriminator,
					);
					#write_version
				}
			}
		}
	};
	let migration_impl = migration.as_ref().map(|migration| {
		migration.implementation(&crate_path, &struct_name, &discriminator, &variant)
	});
	let account_migration = match migration.as_ref() {
		Some(migration) => {
			match migration.fixed_account_implementation(&crate_path, &struct_name) {
				Ok(implementation) => implementation,
				Err(error) => return error.to_compile_error(),
			}
		}
		None => None,
	};

	let implementations = quote! {
		impl #struct_name {
			#view_helpers
		}

		impl #crate_path::HasDiscriminator for #struct_name {
			type Type = #discriminator;

			const VALUE: Self::Type = #discriminator::#variant;
		}

		#migration_impl

		#validation_impl
		#value_validation_impl

		#account_impl
		#account_migration
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
	patch_name: &syn::Ident,
	schema: &schema::CompactSchema,
	migration: Option<&MigrationExpansion>,
) -> proc_macro2::TokenStream {
	let require_current = migration.map(|migration| migration.require_current(crate_path));
	let write_current = migration.map(|migration| migration.write_current(crate_path));
	#[cfg(feature = "validation")]
	let read_value = quote! {
		let value = #ref_name::new(data).map_err(|_| #error)?;
		<#ref_name<'_> as #crate_path::PinaValidate>::validate(&value)?;

		Ok(value)
	};
	#[cfg(not(feature = "validation"))]
	let read_value = quote! {
		#ref_name::new(data).map_err(|_| #error)
	};
	#[cfg(feature = "validation")]
	let validate_initialized = quote! {
		let value = #ref_name::new(&data[..encoded_len]).map_err(|_| #error)?;
		if let Err(error) = <#ref_name<'_> as #crate_path::PinaValidate>::validate(&value) {
			data.fill(0);
			return Err(error);
		}
	};
	#[cfg(not(feature = "validation"))]
	let validate_initialized = quote! {};
	#[cfg(feature = "validation")]
	let validate_updated = quote! {
		let value = #ref_name::new(&data[..encoded_len]).map_err(|_| #error)?;
		<#ref_name<'_> as #crate_path::PinaValidate>::validate(&value)?;
	};
	#[cfg(not(feature = "validation"))]
	let validate_updated = quote! {};
	let capacity_constants = schema.tails.iter().map(|tail| {
		let name = format_ident!("{}_CAPACITY", tail.name.to_string().to_uppercase());
		let field_name = tail.name.to_string();
		let capacity = &tail.capacity;
		let documentation = format!("Maximum element count for the `{field_name}` compact tail.");

		quote! {
			#[doc = #documentation]
			pub const #name: usize = #capacity;
		}
	});
	let projected_bytes = if schema.tails.iter().any(|tail| tail.optional) {
		quote!()
	} else {
		let count_arguments: Vec<_> = schema
			.tails
			.iter()
			.map(|tail| format_ident!("{}_count", tail.name))
			.collect();
		let capacity_checks = schema
			.tails
			.iter()
			.zip(&count_arguments)
			.map(|(tail, count)| {
				let capacity = format_ident!("{}_CAPACITY", tail.name.to_string().to_uppercase());

				quote! {
					if #count > Self::#capacity {
						return Err(#error);
					}
				}
			});
		let size_additions = schema
			.tails
			.iter()
			.zip(&count_arguments)
			.map(|(tail, count)| {
				let pod = &tail.pod;

				quote! {
					let tail_size = #count
						.checked_mul(::core::mem::size_of::<#pod>())
						.ok_or(#error)?;
					let size = size.checked_add(tail_size).ok_or(#error)?;
				}
			});

		quote! {
			/// Calculate the exact encoded size for the requested compact tail counts.
			///
			/// Count arguments follow the compact tails' declaration order.
			///
			/// # Errors
			///
			/// Returns `InvalidAccountData` when any count exceeds its declared capacity or the
			/// byte-size calculation overflows.
			pub fn projected_bytes(
				#(#count_arguments: usize),*
			) -> Result<usize, #crate_path::ProgramError> {
				#(#capacity_checks)*

				let size = Self::HEADER_SIZE;
				#(#size_additions)*

				Ok(size)
			}
		}
	};

	quote! {
		/// The fixed header size, including the discriminator and tail length prefix.
		pub const HEADER_SIZE: usize = <Self as #crate_path::PinaPodCompact>::HEADER_SIZE;

		/// The minimum encoded and allocated size permitted by this compact schema.
		pub const MIN_SIZE: usize = <Self as #crate_path::PinaPodCompact>::MIN_SIZE;

		/// The maximum encoded size permitted by this compact schema.
		pub const MAX_SIZE: usize = <Self as #crate_path::PinaPodCompact>::MAX_SIZE;

		/// Byte granularity of valid compact account allocations.
		pub const TAIL_ALIGNMENT: usize = <Self as #crate_path::PinaPodCompact>::TAIL_ALIGNMENT;

		#(#capacity_constants)*
		#projected_bytes

		/// Validate and borrow a compact account view.
		pub fn try_from_bytes(data: &[u8]) -> Result<#ref_name<'_>, #crate_path::ProgramError> {
			<Self as #crate_path::PinaPodCompact>::validate_storage_len(data.len())
				.map_err(|_| #error)?;

			if !<Self as #crate_path::HasDiscriminator>::matches_discriminator(data) {
				return Err(#error);
			}
			#require_current

			#read_value
		}

		/// Calculate the encoded length after applying `patch` without changing `data`.
		pub fn updated_len(
			data: &[u8],
			patch: &#patch_name<'_>,
		) -> Result<usize, #crate_path::ProgramError> {
			<Self as #crate_path::PinaPodCompact>::validate_storage_len(data.len())
				.map_err(|_| #error)?;

			if !<Self as #crate_path::HasDiscriminator>::matches_discriminator(data) {
				return Err(#error);
			}
			#require_current

			patch.updated_len(data).map_err(|_| #error)
		}

		/// Apply `patch` to initialized compact account storage.
		///
		/// Structural patch failures are atomic. With the `validation` feature,
		/// application validation runs after the patch is written. Propagate an
		/// application-validation error so the Solana runtime rolls the instruction
		/// back instead of committing the rejected representation.
		pub fn update(
			data: &mut [u8],
			patch: &#patch_name<'_>,
		) -> Result<usize, #crate_path::ProgramError> {
			<Self as #crate_path::PinaPodCompact>::validate_storage_len(data.len())
				.map_err(|_| #error)?;

			if !<Self as #crate_path::HasDiscriminator>::matches_discriminator(data) {
				return Err(#error);
			}
			#require_current

			let encoded_len = patch.update(data).map_err(|_| #error)?;
			<Self as #crate_path::HasDiscriminator>::write_discriminator(data);
			#write_current
			#validate_updated

			Ok(encoded_len)
		}

		/// Initialize compact account storage from one complete patch.
		pub fn initialize(
			data: &mut [u8],
			patch: &#patch_name<'_>,
		) -> Result<usize, #crate_path::ProgramError> {
			let encoded_len = patch.initialize(data).map_err(|_| #error)?;
			<Self as #crate_path::HasDiscriminator>::write_discriminator(data);
			#write_current
			#validate_initialized

			Ok(encoded_len)
		}
	}
}

#[cfg(all(test, not(feature = "compact")))]
mod tests {
	use quote::quote;

	#[test]
	fn compact_accounts_require_the_feature() {
		let expanded = super::expand(
			quote!(discriminator = AccountType::State, compact),
			quote! {
				struct State {
					values: pina::Vec<u64, 4>,
				}
			},
		)
		.to_string();

		assert!(
			expanded.contains("compact account support requires enabling the `compact` feature")
		);
	}
}
