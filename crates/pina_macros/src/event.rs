//! Expansion for `#[event]`.

use darling::FromMeta;
use darling::ast::NestedMeta;
use quote::format_ident;
use quote::quote;
use syn::Fields;
use syn::ItemStruct;

use crate::args::EventArgs;
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
	let capacity_proofs = schema::resolve_capacities(&mut item_struct);

	let struct_name = item_struct.ident.clone();
	let zc_name = format_ident!("{}Zc", struct_name);
	let EventArgs {
		crate_path,
		discriminator,
		variant,
		migrations,
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
	let migration = match crate::migration::expansion(
		&item_struct,
		pina_abi::ContractKind::Event,
		pina_abi::LayoutKind::Fixed,
		migrations.map(crate::args::MigrationsArg::is_enabled),
	) {
		Ok(value) => value,
		Err(error) => return error.to_compile_error(),
	};
	let migration_bytes = migration
		.as_ref()
		.map_or(0, MigrationExpansion::version_bytes);
	let schema_proofs = match schema::validate_fixed_schema(
		&item_struct,
		&crate_path,
		&discriminator,
		&zc_name,
		migration_bytes,
	) {
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
	if let Some(migration) = &migration {
		named_fields.named.insert(1, migration.field(false));
	}

	let view_helpers = generate_view_helpers(
		&crate_path,
		&quote!(#crate_path::ProgramError::InvalidInstructionData),
		false,
		migration.as_ref(),
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
	let migration_impl = migration.as_ref().map(|migration| {
		migration.implementation(&crate_path, &struct_name, &discriminator, &variant)
	});
	let event_migration_impl = match migration.as_ref() {
		Some(migration) => {
			match migration.event_implementation(&crate_path, &struct_name) {
				Ok(value) => Some(value),
				Err(error) => return error.to_compile_error(),
			}
		}
		None => None,
	};
	let emit_helper = generate_emit_helper(&crate_path);
	// `emit` materializes the complete record in one stack frame, so an
	// oversized schema would exhaust the 4 KiB SBF stack at runtime instead of
	// failing here. The bound is a public constant so the number a caller reads
	// is the number the macro enforces.
	let stack_budget_assertion = quote! {
		const _: () = assert!(
			#struct_name::SIZE <= #crate_path::MAX_EVENT_RECORD_BYTES,
			concat!(
				"event record for ",
				stringify!(#struct_name),
				" exceeds the SBF stack budget; reduce the payload or field count",
			),
		);
	};
	let implementations = quote! {
		#stack_budget_assertion

		impl #struct_name {
			#view_helpers
			#emit_helper
		}

		impl #crate_path::HasDiscriminator for #struct_name {
			type Type = #discriminator;

			const VALUE: Self::Type = #discriminator::#variant;
		}

		#migration_impl
		#event_migration_impl

		#value_validation_impl
	};

	quote! {
		#item_struct
		#capacity_proofs
		#schema_proofs
		#implementations
	}
}

/// Generate the `emit` helper for one event struct.
///
/// The record is built through the same generated [`initialize`] used by
/// `try_from_bytes`, so emission writes the discriminant, any migration
/// version envelope, and the payload through one validated path. Callers
/// cannot emit a record that the generated decoders would reject.
///
/// [`initialize`]: https://docs.rs/pina/latest/pina/
fn generate_emit_helper(crate_path: &syn::Path) -> proc_macro2::TokenStream {
	quote! {
		/// Build this event's complete record and emit it to the transaction log.
		///
		/// This is the on-chain producer for the `Program data:` records that
		/// generated Rust, TypeScript, and Dart clients decode. The record is
		/// built by [`Self::initialize`], so it carries the discriminator and
		/// any schema version envelope and passes the same structural and
		/// application validation that [`Self::try_from_bytes`] enforces.
		///
		/// Emission is a log write, so an emitted event is observable to
		/// clients but does not affect program state. Propagate the returned
		/// error with `?`: a program built without the `logs` feature cannot
		/// emit, and silently discarding the record hides that.
		///
		/// # Errors
		///
		/// Returns the generated invalid-data error when the record has the
		/// wrong length, the closure fails, or validation rejects the
		/// completed representation. Returns `UnsupportedSysvar` when the
		/// `logs` feature is disabled.
		pub fn emit(
			initialize: impl FnOnce(
				&mut <Self as #crate_path::PinaPodFixed>::Zc,
			) -> Result<(), #crate_path::PinaPodError>,
		) -> Result<(), #crate_path::ProgramError> {
			let mut record = [0u8; Self::SIZE];
			let _ = Self::initialize(&mut record, initialize)?;

			#crate_path::emit_event(&record)
		}
	}
}
