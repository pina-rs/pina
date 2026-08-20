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
/// initialized storage, and zeropod returns the generated zero-copy companion
/// that is allowed to observe and mutate that storage.
pub(crate) fn generate_view_helpers(
	crate_path: &Path,
	error: &proc_macro2::TokenStream,
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
			///
			/// Every accepted field has an audited all-zero representation. The macro
			/// rejects custom types and other layouts whose zero state cannot be
			/// established by Pina's closed schema grammar.
			///
			/// # Errors
			///
			/// Returns the generated invalid-data error when `data` has the wrong
			/// length or zeroed storage is not a valid zeropod representation.
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
