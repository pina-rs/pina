//! Helpers shared by macro expanders.

use quote::quote;
use syn::Attribute;
use syn::Error;
use syn::Ident;
use syn::Path;
use syn::Token;
use syn::punctuated::Punctuated;

/// Add derives to an item without duplicating an existing derive name.
pub(crate) fn add_derives(attributes: &mut Vec<Attribute>, additions: &[Path]) -> syn::Result<()> {
	if let Some(attribute) = attributes
		.iter_mut()
		.find(|attribute| attribute.path().is_ident("derive"))
	{
		let mut derives =
			attribute.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)?;

		for addition in additions {
			let Some(name) = addition.segments.last() else {
				return Err(Error::new_spanned(addition, "derive path cannot be empty"));
			};
			let is_present = derives.iter().any(|existing| {
				existing
					.segments
					.last()
					.is_some_and(|segment| segment.ident == name.ident)
			});

			if !is_present {
				derives.push(addition.clone());
			}
		}

		*attribute = syn::parse_quote!(#[derive(#derives)]);
	} else {
		attributes.push(syn::parse_quote!(#[derive(#(#additions),*)]));
	}

	Ok(())
}

/// Generates bytes-first construction and validated viewing helpers.
///
/// These helpers never turn a native schema value into bytes. Callers provide
/// initialized storage, and `PinaPod` returns the generated zero-copy companion
/// that is allowed to observe and mutate that storage.
pub(crate) fn generate_view_helpers(
	crate_path: &Path,
	error: &proc_macro2::TokenStream,
	account_boundary: bool,
) -> proc_macro2::TokenStream {
	let initialize = if account_boundary {
		quote! {
			<Self as #crate_path::PinaAccount>::initialize(data, initialize)
		}
	} else {
		quote! {
			<Self as #crate_path::PinaPodFixed>::initialize(data, |value| {
				<Self as #crate_path::HasDiscriminator>::write_discriminator(
					&mut value.discriminator,
				);
				initialize(value)
			})
			.map_err(|_| #error)
		}
	};
	#[cfg(feature = "validation")]
	let validate_value = quote! {
		<<Self as #crate_path::PinaPodFixed>::Zc as #crate_path::PinaValidate>::validate(value)?;
	};
	#[cfg(feature = "validation")]
	let validate_initialized = if account_boundary {
		quote! {}
	} else {
		quote! {
			if let Err(error) =
				<<Self as #crate_path::PinaPodFixed>::Zc as #crate_path::PinaValidate>::validate(value)
			{
				#crate_path::__clear_zc(value);
				return Err(error);
			}
		}
	};
	#[cfg(feature = "validation")]
	let read = quote! {
		let value = <Self as #crate_path::PinaPodFixed>::read_exact(data)
			.map_err(|_| #error)?;
		#validate_value

		Ok(value)
	};
	#[cfg(not(feature = "validation"))]
	let read = quote! {
		<Self as #crate_path::PinaPodFixed>::read_exact(data).map_err(|_| #error)
	};
	#[cfg(feature = "validation")]
	let initialize_body = quote! {
		let value = #initialize?;
		#validate_initialized

		Ok(value)
	};
	#[cfg(not(feature = "validation"))]
	let initialize_body = initialize;
	#[cfg(feature = "validation")]
	let initialization_failure_docs = quote! {
		/// structural or application validation fails, the complete slice is
		/// zeroed again. Application validation returns its declared `ProgramError`.
	};
	#[cfg(not(feature = "validation"))]
	let initialization_failure_docs = quote! {
		/// validation fails, `PinaPod` zeros the complete slice again.
	};
	#[cfg(feature = "validation")]
	let initialization_error_docs = quote! {
		/// Returns the generated invalid-data error when `data` has the wrong length,
		/// the closure fails, or structural validation rejects the representation.
		/// Application validation returns its declared `ProgramError`.
	};
	#[cfg(not(feature = "validation"))]
	let initialization_error_docs = quote! {
		/// Returns the generated invalid-data error when `data` has the wrong length,
		/// the closure fails, or the completed representation is invalid.
	};

	quote! {
		/// The exact number of bytes required by the `PinaPod` representation.
		pub const SIZE: usize = ::core::mem::size_of::<<Self as #crate_path::PinaPodFixed>::Zc>();

		/// Validate `data` and return `PinaPod`'s immutable zero-copy companion.
		pub fn try_from_bytes(
			data: &[u8],
		) -> Result<&<Self as #crate_path::PinaPodFixed>::Zc, #crate_path::ProgramError> {
			if data.len() != Self::SIZE
				|| !<Self as #crate_path::HasDiscriminator>::matches_discriminator(data)
			{
				return Err(#error);
			}

			#read
		}

			/// Initialize caller-owned storage with a complete typed configuration.
			///
		/// `PinaPod` zeros the complete slice before calling `initialize`, then
		/// validates the finished representation once. The discriminator is written
		/// before the caller configures the remaining fields. If the closure or final
		#initialization_failure_docs
			///
			/// # Errors
			///
			#initialization_error_docs
			pub fn initialize<'data>(
			data: &'data mut [u8],
			initialize: impl FnOnce(
				&mut <Self as #crate_path::PinaPodFixed>::Zc,
			) -> Result<(), #crate_path::PinaPodError>,
		) -> Result<&'data mut <Self as #crate_path::PinaPodFixed>::Zc, #crate_path::ProgramError> {
			#initialize_body
		}
	}
}

/// Split the final segment from a qualified `Enum::Variant` path.
fn split_discriminator_path(path: &Path) -> Result<(Path, Ident), Error> {
	let Some(variant) = path.segments.last() else {
		return Err(Error::new_spanned(
			path,
			"`discriminator` path cannot be empty",
		));
	};
	let mut enum_segments = Punctuated::new();

	for segment in path.segments.iter().take(path.segments.len() - 1) {
		enum_segments.push(segment.clone());
	}

	if enum_segments.is_empty() {
		return Err(Error::new_spanned(
			path,
			"`discriminator` must include an enum before its variant",
		));
	}

	Ok((
		Path {
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
pub(crate) fn resolve_discriminator_variant(
	discriminator: &Path,
	explicit_variant: Option<Ident>,
	struct_name: &Ident,
) -> Result<(Path, Ident), Error> {
	if let Some(variant) = explicit_variant {
		return Ok((discriminator.clone(), variant));
	}

	if discriminator.segments.len() == 1 {
		return Ok((discriminator.clone(), struct_name.clone()));
	}

	split_discriminator_path(discriminator)
}
