//! Expansion for `#[derive(Accounts)]`.

use darling::FromDeriveInput;
use darling::ast::Style;
use quote::quote;
use syn::DeriveInput;
use syn::Type;

use crate::args::AccountsInput;

pub(crate) fn expand(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
	// Parse input
	let input: DeriveInput = match syn::parse2(input) {
		Ok(v) => v,
		Err(e) => return e.to_compile_error(),
	};

	let args = match AccountsInput::from_derive_input(&input) {
		Ok(v) => v,
		Err(e) => return e.write_errors(),
	};

	// Extract configuration
	let struct_name = &args.ident;
	let (impl_generics, ty_generics, where_clause) = args.generics.split_for_impl();
	let crate_path = &args.crate_path;
	let fields = match args.data.take_struct() {
		Some(fields) if fields.style == Style::Struct => fields,
		Some(_) => {
			return syn::Error::new_spanned(&args.ident, "Accounts structs must have named fields")
				.to_compile_error();
		}
		None => {
			return syn::Error::new_spanned(&args.ident, "Accounts derive only supports structs")
				.to_compile_error();
		}
	};

	// Get lifetime parameter
	let lifetime = match args.generics.lifetimes().next() {
		Some(lt) => &lt.lifetime,
		None => {
			return syn::Error::new_spanned(
				&args.ident,
				"Accounts struct must have **ONE** lifetime parameter",
			)
			.to_compile_error();
		}
	};

	// Process fields
	let mut field_idents = Vec::new();
	let mut parse_fields = Vec::new();
	let mut remaining_field = None;
	let field_count = fields.len();
	let mut seen_remaining = false;

	for field in fields.iter() {
		if field.distinct.is_present() && !field.remaining.is_present() {
			return syn::Error::new_spanned(
				&field.ident,
				"`distinct` is only valid with `#[pina(remaining)]`",
			)
			.to_compile_error();
		}

		if !field.remaining.is_present() {
			continue;
		}

		if seen_remaining {
			return syn::Error::new_spanned(
				&field.ident,
				"Only one field can be marked as `remaining`",
			)
			.to_compile_error();
		}

		seen_remaining = true;
	}

	for (index, field) in fields.iter().enumerate() {
		let ident = field
			.ident
			.as_ref()
			.unwrap_or_else(|| panic!("internal error: `Accounts` field without an ident"));

		if field.remaining.is_present() {
			if index + 1 != field_count {
				return syn::Error::new_spanned(
					&field.ident,
					"`#[pina(remaining)]` field must be the last field",
				)
				.to_compile_error();
			}

			let is_mut = is_mut_reference(&field.ty);
			if field.distinct.is_present() && !is_mut {
				return syn::Error::new_spanned(
					&field.ident,
					"`#[pina(remaining, distinct)]` requires a mutable account slice",
				)
				.to_compile_error();
			}

			remaining_field = field
				.ident
				.as_ref()
				.map(|ident| (ident, is_mut, field.distinct.is_present()));
			continue;
		}

		field_idents.push(ident);
		let parse_field = match account_field_kind(&field.ty) {
			Ok(AccountFieldKind::Mutable) => quote! { let #ident = cursor.next_mut()?; },
			Ok(AccountFieldKind::Immutable) => quote! { let #ident = cursor.next()?; },
			Ok(AccountFieldKind::OptionalMutable) => {
				quote! { let #ident = cursor.next_mut_opt()?; }
			}
			Ok(AccountFieldKind::OptionalImmutable) => {
				quote! { let #ident = cursor.next_opt()?; }
			}
			Ok(AccountFieldKind::Nested) => {
				let ty = &field.ty;
				quote! { let #ident = <#ty as #crate_path::ParseAccounts>::parse_accounts(cursor)?; }
			}
			Err(error) => return error.to_compile_error(),
		};
		parse_fields.push(parse_field);
	}

	let finish_exact = remaining_field.is_none().then(|| {
		quote! {
			cursor.finish_exact()?;
		}
	});
	let remaining_binding = remaining_field.map(|(field, is_mut, is_distinct)| {
		if is_distinct {
			quote! { let #field = cursor.remaining_mut_distinct()?; }
		} else if is_mut {
			quote! { let #field = cursor.remaining_mut()?; }
		} else {
			quote! { let #field = cursor.take_remaining(); }
		}
	});
	let remaining_field_ident = remaining_field.map(|(field, ..)| quote!(#field,));

	quote! {
		impl #impl_generics #crate_path::ParseAccounts #ty_generics for #struct_name #ty_generics #where_clause {
			fn parse_accounts(
				cursor: &mut #crate_path::AccountsCursor<#lifetime>,
			) -> ::core::result::Result<Self, #crate_path::ProgramError> {
				#(#parse_fields)*
				#remaining_binding

				Ok(Self {
					#(#field_idents,)*
					#remaining_field_ident
				})
			}
		}

		impl #impl_generics #crate_path::TryFromAccountInfos #ty_generics for #struct_name #ty_generics #where_clause {
			fn try_from_account_infos(
				program_id: &#crate_path::Address,
				accounts: & #lifetime mut [#crate_path::AccountView],
			) -> ::core::result::Result<Self, #crate_path::ProgramError> {
				let mut cursor = #crate_path::AccountsCursor::new(*program_id, accounts);
				let parsed = <Self as #crate_path::ParseAccounts>::parse_accounts(&mut cursor)?;
				#finish_exact

				Ok(parsed)
			}
		}

		impl #impl_generics ::core::convert::TryFrom<(& #lifetime #crate_path::Address, & #lifetime mut [#crate_path::AccountView])> for #struct_name #ty_generics #where_clause {
			type Error = #crate_path::ProgramError;

			fn try_from(
				(program_id, accounts): (& #lifetime #crate_path::Address, & #lifetime mut [#crate_path::AccountView]),
			) -> ::core::result::Result<Self, Self::Error> {
				<Self as #crate_path::TryFromAccountInfos>::try_from_account_infos(program_id, accounts)
			}
		}
	}
}

fn is_reference(ty: &Type) -> bool {
	matches!(ty, Type::Reference(_))
}

fn is_mut_reference(ty: &Type) -> bool {
	matches!(ty, Type::Reference(reference) if reference.mutability.is_some())
}

/// How `#[derive(Accounts)]` parses a single named field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccountFieldKind {
	/// `&AccountView` — required immutable slot.
	Immutable,
	/// `&mut AccountView` — required writable slot.
	Mutable,
	/// `Option<&AccountView>` — optional immutable slot.
	OptionalImmutable,
	/// `Option<&mut AccountView>` — optional writable slot.
	OptionalMutable,
	/// Any other type — delegated to its own `ParseAccounts` impl.
	Nested,
}

/// Classify an `Accounts` field type for code generation.
///
/// Returns an error for `Option<T>` wrappers whose inner type is not an
/// account reference, since those cannot be mapped onto fixed account slots.
fn account_field_kind(ty: &Type) -> Result<AccountFieldKind, syn::Error> {
	if let Some(kind) = option_inner_kind(ty)? {
		return Ok(kind);
	}

	if is_mut_reference(ty) {
		return Ok(AccountFieldKind::Mutable);
	}

	if is_reference(ty) {
		return Ok(AccountFieldKind::Immutable);
	}

	Ok(AccountFieldKind::Nested)
}

/// Detect `Option<...>` wrappers and classify their inner reference.
///
/// Returns `Ok(None)` when the type is not an `Option` at all and `Err`
/// when the wrapped type cannot be used as a fixed account slot.
fn option_inner_kind(ty: &Type) -> Result<Option<AccountFieldKind>, syn::Error> {
	let Type::Path(type_path) = ty else {
		return Ok(None);
	};

	let Some(segment) = type_path.path.segments.last() else {
		return Ok(None);
	};

	if segment.ident != "Option" {
		return Ok(None);
	}

	let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
		return Err(syn::Error::new_spanned(
			ty,
			"`Option` account fields require explicit type arguments, e.g. `Option<&AccountView>`",
		));
	};

	let [syn::GenericArgument::Type(inner)] = arguments.args.iter().collect::<Vec<_>>().as_slice()
	else {
		return Err(syn::Error::new_spanned(
			ty,
			"`Option` account fields take exactly one type argument",
		));
	};

	match inner {
		Type::Reference(reference) if reference.mutability.is_some() => {
			Ok(Some(AccountFieldKind::OptionalMutable))
		}
		Type::Reference(_) => Ok(Some(AccountFieldKind::OptionalImmutable)),
		_ => {
			Err(syn::Error::new_spanned(
				ty,
				"only `Option<&AccountView>` and `Option<&mut AccountView>` fields are supported",
			))
		}
	}
}
