//! Closed field grammar for Pina's fixed zero-copy schemas.
//!
//! Zeropod's derive intentionally supports a fallback through `ZcField`.
//! That extension point is useful for direct zeropod users, but Pina cannot
//! safely accept it at an account or instruction boundary: `ZcField` and
//! `ZcValidate` are safe traits, so an unknown mapping is not proof that its
//! generated storage type may be referenced before validation. Pina therefore
//! accepts only the concrete representations audited below.

use quote::quote;
use syn::Expr;
use syn::ExprLit;
use syn::Field;
use syn::Fields;
use syn::GenericArgument;
use syn::ItemStruct;
use syn::Lit;
use syn::PathArguments;
use syn::Token;
use syn::Type;
use syn::punctuated::Punctuated;

/// Validate a schema and emit compile-time type and layout proofs.
pub(crate) fn validate_fixed_schema(
	item: &ItemStruct,
	crate_path: &syn::Path,
	discriminator: &syn::Path,
	zc_name: &syn::Ident,
) -> syn::Result<proc_macro2::TokenStream> {
	if !item.generics.params.is_empty() || item.generics.where_clause.is_some() {
		return Err(syn::Error::new_spanned(
			&item.generics,
			"Pina zero-copy schemas cannot be generic; use concrete audited field types",
		));
	}

	if item
		.attrs
		.iter()
		.any(|attribute| attribute.path().is_ident("zeropod"))
	{
		return Err(syn::Error::new_spanned(
			item,
			"`#[zeropod(...)]` cannot override a Pina account, instruction, or event layout",
		));
	}

	for attribute in item
		.attrs
		.iter()
		.filter(|attribute| attribute.path().is_ident("derive"))
	{
		let derives =
			attribute.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)?;
		if let Some(derive) = derives.iter().find(|derive| {
			derive
				.segments
				.last()
				.is_some_and(|segment| segment.ident == "ZeroPod")
		}) {
			return Err(syn::Error::new_spanned(
				derive,
				"Pina owns the `ZeroPod` derive for account, instruction, and event schemas; \
				 remove the manual derive",
			));
		}
	}

	let Fields::Named(fields) = &item.fields else {
		return Err(syn::Error::new_spanned(
			item,
			"Pina zero-copy schemas must have named fields",
		));
	};
	let mut field_proofs = Vec::with_capacity(fields.named.len());
	let mut field_sizes = Vec::with_capacity(fields.named.len());

	for field in &fields.named {
		let audited = classify_field(field, crate_path)?;
		let source = &field.ty;
		let native = &audited.native;
		let pod = &audited.pod;

		field_proofs.push(quote! {
			const _: fn(#source) -> #native = |value| value;

			const _: fn() = || {
				fn assert_mapping<T: #crate_path::ZcField<Pod = #pod>>() {}
				fn assert_storage<T: #crate_path::ZcElem>() {}

				assert_mapping::<#source>();
				assert_storage::<#pod>();
			};

			const _: () = {
				::core::assert!(::core::mem::align_of::<#pod>() == 1);
				::core::assert!(
					::core::mem::size_of::<#pod>()
						== <#source as #crate_path::ZcField>::POD_SIZE,
				);
			};
		});
		field_sizes.push(quote!(::core::mem::size_of::<#pod>()));
	}

	let expected_size = quote!(#discriminator::BYTES #(+ #field_sizes)*);

	Ok(quote! {
		#(#field_proofs)*

		const _: fn() = || {
			fn assert_storage<T: #crate_path::ZcElem>() {}

			assert_storage::<#zc_name>();
		};

		const _: () = {
			::core::assert!(::core::mem::align_of::<#zc_name>() == 1);
			::core::assert!(::core::mem::size_of::<#zc_name>() == #expected_size);
		};
	})
}

struct AuditedField {
	native: proc_macro2::TokenStream,
	pod: proc_macro2::TokenStream,
}

fn classify_field(field: &Field, crate_path: &syn::Path) -> syn::Result<AuditedField> {
	if field
		.attrs
		.iter()
		.any(|attribute| attribute.path().is_ident("zeropod"))
	{
		return Err(syn::Error::new_spanned(
			field,
			"`#[zeropod(...)]` field overrides are not supported by Pina schemas",
		));
	}

	match &field.ty {
		Type::Array(array) => classify_byte_array(array, crate_path),
		Type::Path(path) if path.qself.is_none() => classify_path(&field.ty, path, crate_path),
		other => Err(unsupported(other)),
	}
}

fn classify_byte_array(
	array: &syn::TypeArray,
	_crate_path: &syn::Path,
) -> syn::Result<AuditedField> {
	let Type::Path(element) = array.elem.as_ref() else {
		return Err(syn::Error::new_spanned(
			&array.elem,
			"only one-dimensional `[u8; N]` byte arrays are supported in Pina schemas",
		));
	};

	if !element.path.is_ident("u8") || !is_integer_literal(&array.len) {
		return Err(syn::Error::new_spanned(
			array,
			"only `[u8; N]` arrays with a literal length are supported in Pina schemas",
		));
	}

	let length = &array.len;

	Ok(AuditedField {
		native: quote!([::core::primitive::u8; #length]),
		pod: quote!([::core::primitive::u8; #length]),
	})
}

fn classify_path(
	ty: &Type,
	type_path: &syn::TypePath,
	crate_path: &syn::Path,
) -> syn::Result<AuditedField> {
	let Some(segment) = type_path.path.segments.last() else {
		return Err(unsupported(ty));
	};

	if segment.arguments.is_empty() {
		return classify_plain_path(ty, segment, crate_path);
	}

	classify_parameterized_path(ty, segment, crate_path)
}

fn classify_plain_path(
	ty: &Type,
	segment: &syn::PathSegment,
	crate_path: &syn::Path,
) -> syn::Result<AuditedField> {
	if let Some(audited) = classify_scalar(segment, crate_path) {
		return Ok(audited);
	}

	match segment.ident.to_string().as_str() {
		"PodU16" | "PodU32" | "PodU64" | "PodU128" | "PodI16" | "PodI32" | "PodI64" | "PodI128"
		| "PodBool" => {
			Ok(AuditedField {
				native: {
					let ident = &segment.ident;
					quote!(#crate_path::#ident)
				},
				pod: {
					let ident = &segment.ident;
					quote!(#crate_path::#ident)
				},
			})
		}
		"Address" => {
			Ok(AuditedField {
				native: quote!(#crate_path::Address),
				pod: quote!(#crate_path::Address),
			})
		}
		"char" => {
			Err(syn::Error::new_spanned(
				ty,
				"`char` is not a zero-copy field: not every 32-bit pattern is a valid character",
			))
		}
		name if name.starts_with("NonZero") => {
			Err(syn::Error::new_spanned(
				ty,
				"`NonZero*` types are not zero-copy fields because an all-zero bit pattern is \
				 invalid",
			))
		}
		_ => Err(custom_mapping(ty)),
	}
}

fn classify_parameterized_path(
	ty: &Type,
	segment: &syn::PathSegment,
	crate_path: &syn::Path,
) -> syn::Result<AuditedField> {
	match segment.ident.to_string().as_str() {
		"Option" => classify_option(ty, segment, crate_path),
		"String" | "PodString" | "Vec" | "PodVec" => {
			Err(syn::Error::new_spanned(
				ty,
				"fixed-capacity `String` and `Vec` fields are temporarily unsupported because \
				 upstream collection defaults can contain uninitialized inactive capacity; use \
				 fully initialized fixed fields such as `[u8; N]`",
			))
		}
		"PodOption" => {
			Err(syn::Error::new_spanned(
				ty,
				"raw `PodOption` fields are unsupported; use semantic `Option<scalar>` so Pina \
				 can prove the exact storage mapping",
			))
		}
		_ => Err(custom_mapping(ty)),
	}
}

fn classify_option(
	ty: &Type,
	segment: &syn::PathSegment,
	crate_path: &syn::Path,
) -> syn::Result<AuditedField> {
	let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
		return Err(unsupported(ty));
	};

	if arguments.args.len() != 1 {
		return Err(syn::Error::new_spanned(
			ty,
			"`Option` fields require exactly one audited scalar type",
		));
	}
	let Some(GenericArgument::Type(inner)) = arguments.args.first() else {
		return Err(syn::Error::new_spanned(
			ty,
			"`Option` fields require an audited scalar type",
		));
	};
	let audited = classify_option_scalar(inner, crate_path)?;
	let native_inner = audited.native;
	let pod_inner = audited.pod;

	Ok(AuditedField {
		native: quote!(::core::option::Option<#native_inner>),
		pod: quote!(#crate_path::PodOption<#pod_inner>),
	})
}

fn classify_option_scalar(ty: &Type, crate_path: &syn::Path) -> syn::Result<AuditedField> {
	let Type::Path(type_path) = ty else {
		return Err(nested_option(ty));
	};
	let Some(segment) = type_path.path.segments.last() else {
		return Err(nested_option(ty));
	};

	if !segment.arguments.is_empty() {
		return Err(nested_option(ty));
	}
	classify_scalar(segment, crate_path).ok_or_else(|| nested_option(ty))
}

fn classify_scalar(segment: &syn::PathSegment, crate_path: &syn::Path) -> Option<AuditedField> {
	Some(match segment.ident.to_string().as_str() {
		"u8" | "i8" => audited_direct_scalar(&segment.ident),
		"u16" => audited_native_scalar(&segment.ident, quote!(#crate_path::PodU16)),
		"u32" => audited_native_scalar(&segment.ident, quote!(#crate_path::PodU32)),
		"u64" => audited_native_scalar(&segment.ident, quote!(#crate_path::PodU64)),
		"u128" => audited_native_scalar(&segment.ident, quote!(#crate_path::PodU128)),
		"i16" => audited_native_scalar(&segment.ident, quote!(#crate_path::PodI16)),
		"i32" => audited_native_scalar(&segment.ident, quote!(#crate_path::PodI32)),
		"i64" => audited_native_scalar(&segment.ident, quote!(#crate_path::PodI64)),
		"i128" => audited_native_scalar(&segment.ident, quote!(#crate_path::PodI128)),
		"bool" => audited_native_scalar(&segment.ident, quote!(#crate_path::PodBool)),
		_ => return None,
	})
}

fn audited_direct_scalar(ident: &syn::Ident) -> AuditedField {
	AuditedField {
		native: quote!(::core::primitive::#ident),
		pod: quote!(::core::primitive::#ident),
	}
}

fn audited_native_scalar(ident: &syn::Ident, pod: proc_macro2::TokenStream) -> AuditedField {
	AuditedField {
		native: quote!(::core::primitive::#ident),
		pod,
	}
}

fn is_integer_literal(expr: &Expr) -> bool {
	matches!(
		expr,
		Expr::Lit(ExprLit {
			lit: Lit::Int(_),
			..
		})
	)
}

fn nested_option(ty: &Type) -> syn::Error {
	syn::Error::new_spanned(
		ty,
		"unsupported `Option` payload; only native integer and boolean scalars are accepted (no \
		 nested options, collections, arrays, addresses, pod wrappers, or custom types)",
	)
}

fn custom_mapping(ty: &Type) -> syn::Error {
	syn::Error::new_spanned(
		ty,
		"custom `ZcField` mappings and nested schema types are unsupported because Pina cannot \
		 prove their alignment and bit validity; use an audited scalar, `Address`, `[u8; N]`, or \
		 `Option<scalar>`",
	)
}

fn unsupported(ty: &Type) -> syn::Error {
	syn::Error::new_spanned(
		ty,
		"unsupported Pina zero-copy field; expected an audited scalar, `Address`, `[u8; N]`, or \
		 `Option<scalar>`",
	)
}
