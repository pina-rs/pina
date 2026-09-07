//! Closed field grammar for Pina's fixed zero-copy schemas.
//!
//! PinaPod's derive intentionally supports a fallback through `ZcField`.
//! That extension point is useful for direct PinaPod users, but Pina cannot
//! safely accept it at an account or instruction boundary: even though
//! `ZcField` is an unsafe trait, an unknown implementation is outside Pina's
//! closed schema contract. Pina therefore accepts only the concrete
//! representations audited below.

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

fn is_pinapod_attribute(attribute: &syn::Attribute) -> bool {
	attribute.path().is_ident("pinapod")
}

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

	if item.attrs.iter().any(is_pinapod_attribute) {
		return Err(syn::Error::new_spanned(
			item,
			"`#[pinapod(...)]` cannot override a Pina account, instruction, or event layout",
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
				.is_some_and(|segment| segment.ident == "PinaPod")
		}) {
			return Err(syn::Error::new_spanned(
				derive,
				"Pina owns the `PinaPod` derive for account, instruction, and event schemas; \
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
		let audited = classify_fixed_field(field, crate_path)?;
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

/// Compile-time proofs and size metadata for a compact account schema.
pub(crate) struct CompactSchema {
	pub(crate) proofs: proc_macro2::TokenStream,
}

enum CompactField {
	Inline(AuditedField),
	String {
		source: Type,
		audited: AuditedField,
		capacity: proc_macro2::TokenStream,
		prefix_size: usize,
		optional: bool,
	},
	Vec {
		element_source: Type,
		element: AuditedField,
		capacity: proc_macro2::TokenStream,
		prefix_size: usize,
		optional: bool,
	},
}

/// Validate the interoperable compact-account subset.
///
/// Compact fields form a suffix, matching PinaPod's compact schema grammar.
/// Every tail length lives in the fixed header and the active payloads are
/// concatenated after it in declaration order.
pub(crate) fn validate_compact_schema(
	item: &ItemStruct,
	crate_path: &syn::Path,
	discriminator: &syn::Path,
	header_name: &syn::Ident,
) -> syn::Result<CompactSchema> {
	validate_schema_container(item)?;

	let Fields::Named(fields) = &item.fields else {
		return Err(syn::Error::new_spanned(
			item,
			"Pina zero-copy schemas must have named fields",
		));
	};
	let struct_name = &item.ident;
	let mut field_proofs = Vec::with_capacity(fields.named.len());
	let mut header_sizes = Vec::with_capacity(fields.named.len());
	let mut tail_max_sizes = Vec::new();
	let mut tail_element_sizes = Vec::new();
	let mut tail_prefix_proofs = Vec::new();
	let mut seen_tail = false;

	for field in &fields.named {
		match classify_compact_field(field, crate_path)? {
			CompactField::Inline(audited) => {
				if seen_tail {
					return Err(syn::Error::new_spanned(
						field,
						"inline fields cannot follow a compact dynamic field; place every fixed \
						 field before the compact suffix",
					));
				}

				let source = &field.ty;
				let native = &audited.native;
				let pod = &audited.pod;
				field_proofs.push(mapping_proof(source, native, pod, crate_path));
				header_sizes.push(quote!(::core::mem::size_of::<#pod>()));
			}
			CompactField::String {
				source,
				audited,
				capacity,
				prefix_size,
				optional,
			} => {
				seen_tail = true;
				let native = &audited.native;
				let pod = &audited.pod;
				field_proofs.push(mapping_proof(&source, native, pod, crate_path));
				header_sizes.push(if optional {
					quote!(1usize)
				} else {
					quote!(#prefix_size)
				});
				tail_max_sizes.push(if optional {
					quote!(#prefix_size + #capacity)
				} else {
					quote!(#capacity)
				});
				tail_element_sizes.push(quote!(1usize));
				tail_prefix_proofs.push(compact_prefix_proof(&capacity, prefix_size));
			}
			CompactField::Vec {
				element_source,
				element,
				capacity,
				prefix_size,
				optional,
			} => {
				seen_tail = true;
				let native = &element.native;
				let pod = &element.pod;
				field_proofs.push(mapping_proof(&element_source, native, pod, crate_path));
				header_sizes.push(if optional {
					quote!(1usize)
				} else {
					quote!(#prefix_size)
				});
				let payload_max = quote!(#capacity * ::core::mem::size_of::<#pod>());
				tail_max_sizes.push(if optional {
					quote!(#prefix_size + #payload_max)
				} else {
					payload_max
				});
				tail_element_sizes.push(if optional {
					quote!(gcd(#prefix_size, ::core::mem::size_of::<#pod>()))
				} else {
					quote!(::core::mem::size_of::<#pod>())
				});
				tail_prefix_proofs.push(compact_vec_proof(&capacity, prefix_size, pod));
			}
		}
	}

	if !seen_tail {
		return Err(syn::Error::new_spanned(
			item,
			"compact accounts require at least one dynamic field: `String<N>`, `Vec<T, N>`, \
			 `Option<String<N>>`, or `Option<Vec<T, N>>`",
		));
	}

	let expected_header = quote!(#discriminator::BYTES #(+ #header_sizes)*);
	let max_size = quote!(#expected_header #(+ #tail_max_sizes)*);
	let tail_alignment = quote!({
		const fn gcd(mut left: usize, mut right: usize) -> usize {
			while right != 0 {
				let remainder = left % right;
				left = right;
				right = remainder;
			}
			left
		}

		let mut alignment = 0;
		#(alignment = gcd(alignment, #tail_element_sizes);)*
		alignment
	});
	let proofs = quote! {
		#(#field_proofs)*

		const _: fn() = || {
			fn assert_layout<T: #crate_path::PinaPodCompact<Header = #header_name>>() {}
			assert_layout::<#struct_name>();
		};

		const _: () = {
			::core::assert!(::core::mem::align_of::<#header_name>() == 1);
			::core::assert!(::core::mem::size_of::<#header_name>() == #expected_header);
			::core::assert!(
				<#struct_name as #crate_path::PinaPodCompact>::MIN_SIZE == #expected_header
			);
			::core::assert!(
				<#struct_name as #crate_path::PinaPodCompact>::MAX_SIZE == #max_size
			);
			::core::assert!(
				<#struct_name as #crate_path::PinaPodCompact>::TAIL_ALIGNMENT == #tail_alignment
			);
			#(#tail_prefix_proofs)*
		};
	};

	Ok(CompactSchema { proofs })
}

fn validate_schema_container(item: &ItemStruct) -> syn::Result<()> {
	if !item.generics.params.is_empty() || item.generics.where_clause.is_some() {
		return Err(syn::Error::new_spanned(
			&item.generics,
			"Pina zero-copy schemas cannot be generic; use concrete audited field types",
		));
	}

	if item.attrs.iter().any(is_pinapod_attribute) {
		return Err(syn::Error::new_spanned(
			item,
			"`#[pinapod(...)]` cannot override a Pina account, instruction, or event layout",
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
				.is_some_and(|segment| segment.ident == "PinaPod")
		}) {
			return Err(syn::Error::new_spanned(
				derive,
				"Pina owns the `PinaPod` derive for account, instruction, and event schemas; \
				 remove the manual derive",
			));
		}
	}

	Ok(())
}

fn classify_compact_field(field: &Field, crate_path: &syn::Path) -> syn::Result<CompactField> {
	if field.attrs.iter().any(is_pinapod_attribute) {
		return Err(syn::Error::new_spanned(
			field,
			"`#[pinapod(...)]` field overrides are not supported by Pina schemas",
		));
	}

	let Type::Path(type_path) = &field.ty else {
		return classify_fixed_type(&field.ty, crate_path).map(CompactField::Inline);
	};
	let Some(segment) = type_path.path.segments.last() else {
		return Err(compact_supported_error(&field.ty));
	};

	match segment.ident.to_string().as_str() {
		"String" | "PodString" => classify_compact_string(&field.ty, crate_path, false),
		"Vec" | "PodVec" => classify_compact_vec(&field.ty, crate_path, false),
		"Option" => classify_compact_option(&field.ty, segment, crate_path),
		_ => classify_fixed_type(&field.ty, crate_path).map(CompactField::Inline),
	}
}

fn classify_compact_string(
	ty: &Type,
	crate_path: &syn::Path,
	optional: bool,
) -> syn::Result<CompactField> {
	let Type::Path(type_path) = ty else {
		return Err(compact_supported_error(ty));
	};
	let Some(segment) = type_path.path.segments.last() else {
		return Err(compact_supported_error(ty));
	};
	let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
		return Err(compact_supported_error(ty));
	};
	let is_alias = segment.ident == "String";
	let valid_argument_count = if is_alias {
		arguments.args.len() == 1
	} else {
		(1..=2).contains(&arguments.args.len())
	};
	if !valid_argument_count {
		return Err(compact_supported_error(ty));
	}
	let capacity = literal_const_argument(arguments.args.first(), ty, "string capacity")?;
	let capacity = quote!(#capacity);
	let prefix_size = if is_alias {
		1
	} else {
		literal_prefix_argument(arguments.args.iter().nth(1), 1, ty)?
	};
	let source = if optional {
		syn::parse_quote!(::core::option::Option<#ty>)
	} else {
		ty.clone()
	};
	let audited = classify_fixed_type(&source, crate_path)?;

	Ok(CompactField::String {
		source,
		audited,
		capacity,
		prefix_size,
		optional,
	})
}

fn classify_compact_vec(
	ty: &Type,
	crate_path: &syn::Path,
	optional: bool,
) -> syn::Result<CompactField> {
	let Type::Path(type_path) = ty else {
		return Err(compact_supported_error(ty));
	};
	let Some(segment) = type_path.path.segments.last() else {
		return Err(compact_supported_error(ty));
	};
	let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
		return Err(compact_supported_error(ty));
	};
	let is_alias = segment.ident == "Vec";
	let valid_argument_count = if is_alias {
		arguments.args.len() == 2
	} else {
		(2..=3).contains(&arguments.args.len())
	};
	if !valid_argument_count {
		return Err(compact_supported_error(ty));
	}
	let Some(GenericArgument::Type(element)) = arguments.args.first() else {
		return Err(compact_supported_error(ty));
	};
	if optional && is_compact_string_type(element) {
		return Err(compact_supported_error(ty));
	}
	if !is_compact_string_type(element) && contains_dynamic_compact_type(element) {
		return Err(compact_supported_error(ty));
	}
	let capacity = match arguments.args.iter().nth(1) {
		Some(GenericArgument::Const(value)) if is_integer_literal(value) => quote!(#value),
		_ => return Err(compact_supported_error(ty)),
	};
	let prefix_size = if is_alias {
		2
	} else {
		literal_prefix_argument(arguments.args.iter().nth(2), 2, ty)?
	};
	let audited =
		classify_fixed_type(element, crate_path).map_err(|_| compact_supported_error(ty))?;

	Ok(CompactField::Vec {
		element_source: element.clone(),
		element: audited,
		capacity,
		prefix_size,
		optional,
	})
}

fn classify_compact_option(
	ty: &Type,
	segment: &syn::PathSegment,
	crate_path: &syn::Path,
) -> syn::Result<CompactField> {
	let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
		return Err(compact_supported_error(ty));
	};
	if arguments.args.len() != 1 {
		return Err(compact_supported_error(ty));
	}
	let Some(GenericArgument::Type(inner)) = arguments.args.first() else {
		return Err(compact_supported_error(ty));
	};

	if is_compact_string_type(inner) {
		return classify_compact_string(inner, crate_path, true);
	}
	if is_compact_vec_type(inner) {
		return classify_compact_vec(inner, crate_path, true);
	}
	if contains_dynamic_compact_type(inner) {
		return Err(compact_supported_error(ty));
	}

	classify_fixed_type(ty, crate_path).map(CompactField::Inline)
}

fn is_compact_string_type(ty: &Type) -> bool {
	last_type_name(ty).is_some_and(|name| name == "String" || name == "PodString")
}

fn is_compact_vec_type(ty: &Type) -> bool {
	last_type_name(ty).is_some_and(|name| name == "Vec" || name == "PodVec")
}

fn contains_dynamic_compact_type(ty: &Type) -> bool {
	if is_compact_string_type(ty) || is_compact_vec_type(ty) {
		return true;
	}
	if last_type_name(ty).is_none_or(|name| name != "Option") {
		return false;
	}

	let Type::Path(type_path) = ty else {
		return false;
	};
	let Some(segment) = type_path.path.segments.last() else {
		return false;
	};
	let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
		return false;
	};

	arguments.args.first().is_some_and(|argument| {
		matches!(argument, GenericArgument::Type(inner) if contains_dynamic_compact_type(inner))
	})
}

fn last_type_name(ty: &Type) -> Option<&syn::Ident> {
	let Type::Path(type_path) = ty else {
		return None;
	};
	type_path.path.segments.last().map(|segment| &segment.ident)
}

fn mapping_proof(
	source: &Type,
	native: &proc_macro2::TokenStream,
	pod: &proc_macro2::TokenStream,
	crate_path: &syn::Path,
) -> proc_macro2::TokenStream {
	quote! {
		const _: fn(#source) -> #native = |value| value;

		const _: fn() = || {
			fn assert_mapping<T: #crate_path::ZcField<Pod = #pod>>() {}
			fn assert_storage<T: #crate_path::ZcElem>() {}

			assert_mapping::<#source>();
			assert_storage::<#pod>();
		};

		const _: () = {
			::core::assert!(::core::mem::align_of::<#pod>() == 1);
		};
	}
}

fn compact_supported_error(ty: &Type) -> syn::Error {
	syn::Error::new_spanned(
		ty,
		"unsupported compact field; supported dynamic forms are `String<N>`, `Vec<T, N>` for \
		 fixed `T`, `Option<String<N>>`, `Option<Vec<T, N>>` for fixed `T`, and `Vec<String<M>, \
		 N>`; `Option<T>` is supported inline when `T` has a fixed representation",
	)
}

fn compact_prefix_proof(
	capacity: &proc_macro2::TokenStream,
	prefix_size: usize,
) -> proc_macro2::TokenStream {
	let prefix_max = match prefix_size {
		1 => quote!(::core::primitive::u8::MAX as usize),
		2 => quote!(::core::primitive::u16::MAX as usize),
		4 => quote!(::core::primitive::u32::MAX as usize),
		8 => quote!(usize::MAX),
		_ => unreachable!("validated compact prefix size"),
	};

	quote!(::core::assert!(#capacity <= #prefix_max);)
}

fn compact_vec_proof(
	capacity: &proc_macro2::TokenStream,
	prefix_size: usize,
	pod: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	let prefix_proof = compact_prefix_proof(capacity, prefix_size);

	quote! {
		::core::assert!(::core::mem::size_of::<#pod>() > 0);
		#prefix_proof
	}
}

struct AuditedField {
	native: proc_macro2::TokenStream,
	pod: proc_macro2::TokenStream,
}

fn classify_fixed_field(field: &Field, crate_path: &syn::Path) -> syn::Result<AuditedField> {
	if field.attrs.iter().any(is_pinapod_attribute) {
		return Err(syn::Error::new_spanned(
			field,
			"`#[pinapod(...)]` field overrides are not supported by Pina schemas",
		));
	}

	classify_fixed_type(&field.ty, crate_path)
}

fn classify_fixed_type(ty: &Type, crate_path: &syn::Path) -> syn::Result<AuditedField> {
	match ty {
		Type::Array(array) => classify_byte_array(array, crate_path),
		Type::Path(path) if path.qself.is_none() => classify_path(ty, path, crate_path),
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
		"String" | "PodString" => classify_string(ty, segment, crate_path),
		"Vec" | "PodVec" => classify_vec(ty, segment, crate_path),
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

fn classify_string(
	ty: &Type,
	segment: &syn::PathSegment,
	crate_path: &syn::Path,
) -> syn::Result<AuditedField> {
	let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
		return Err(unsupported(ty));
	};
	let is_alias = segment.ident == "String";
	let valid_argument_count = if is_alias {
		arguments.args.len() == 1
	} else {
		(1..=2).contains(&arguments.args.len())
	};

	if !valid_argument_count {
		return Err(syn::Error::new_spanned(
			ty,
			"`String<N>` requires one literal capacity; `PodString<N, PFX>` accepts an optional \
			 literal prefix width",
		));
	}

	let capacity = literal_const_argument(arguments.args.first(), ty, "string capacity")?;
	let prefix_size = if is_alias {
		1
	} else {
		literal_prefix_argument(arguments.args.iter().nth(1), 1, ty)?
	};
	let native = if is_alias {
		quote!(#crate_path::String<#capacity>)
	} else {
		quote!(#crate_path::PodString<#capacity, #prefix_size>)
	};

	Ok(AuditedField {
		native: native.clone(),
		pod: native,
	})
}

fn classify_vec(
	ty: &Type,
	segment: &syn::PathSegment,
	crate_path: &syn::Path,
) -> syn::Result<AuditedField> {
	let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
		return Err(unsupported(ty));
	};
	let is_alias = segment.ident == "Vec";
	let valid_argument_count = if is_alias {
		arguments.args.len() == 2
	} else {
		(2..=3).contains(&arguments.args.len())
	};

	if !valid_argument_count {
		return Err(syn::Error::new_spanned(
			ty,
			"`Vec<T, N>` requires a fixed element type and literal capacity; `PodVec<T, N, PFX>` \
			 also accepts a literal prefix width",
		));
	}

	let Some(GenericArgument::Type(element)) = arguments.args.first() else {
		return Err(unsupported(ty));
	};
	let element = classify_fixed_type(element, crate_path)?;
	let element_native = element.native;
	let capacity = literal_const_argument(arguments.args.iter().nth(1), ty, "vector capacity")?;
	let prefix_size = if is_alias {
		2
	} else {
		literal_prefix_argument(arguments.args.iter().nth(2), 2, ty)?
	};
	let native = if is_alias {
		quote!(#crate_path::Vec<#element_native, #capacity>)
	} else {
		quote!(#crate_path::PodVec<#element_native, #capacity, #prefix_size>)
	};

	Ok(AuditedField {
		native: native.clone(),
		pod: native,
	})
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
			"`Option` fields require exactly one fixed PinaPod type",
		));
	}
	let Some(GenericArgument::Type(inner)) = arguments.args.first() else {
		return Err(syn::Error::new_spanned(
			ty,
			"`Option` fields require a fixed PinaPod type",
		));
	};
	let audited = classify_fixed_type(inner, crate_path)?;
	let native_inner = audited.native;
	let pod_inner = audited.pod;

	Ok(AuditedField {
		native: quote!(::core::option::Option<#native_inner>),
		pod: quote!(#crate_path::PodOption<#pod_inner>),
	})
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

fn literal_const_argument<'a>(
	argument: Option<&'a GenericArgument>,
	ty: &Type,
	name: &str,
) -> syn::Result<&'a Expr> {
	match argument {
		Some(GenericArgument::Const(value)) if is_integer_literal(value) => Ok(value),
		_ => {
			Err(syn::Error::new_spanned(
				ty,
				format!("{name} must be an integer literal"),
			))
		}
	}
}

fn literal_prefix_argument(
	argument: Option<&GenericArgument>,
	default: usize,
	ty: &Type,
) -> syn::Result<usize> {
	let Some(argument) = argument else {
		return Ok(default);
	};
	let GenericArgument::Const(Expr::Lit(ExprLit {
		lit: Lit::Int(value),
		..
	})) = argument
	else {
		return Err(syn::Error::new_spanned(
			ty,
			"PinaPod prefix widths must be integer literals: 1, 2, 4, or 8",
		));
	};
	let prefix_size = value
		.base10_parse::<usize>()
		.map_err(|_| syn::Error::new_spanned(ty, "PinaPod prefix widths must be 1, 2, 4, or 8"))?;

	if !matches!(prefix_size, 1 | 2 | 4 | 8) {
		return Err(syn::Error::new_spanned(
			ty,
			"PinaPod prefix widths must be 1, 2, 4, or 8",
		));
	}

	Ok(prefix_size)
}

fn custom_mapping(ty: &Type) -> syn::Error {
	syn::Error::new_spanned(
		ty,
		"custom `ZcField` mappings and nested schema types are unsupported because Pina cannot \
		 prove their alignment and bit validity; use an audited scalar, `Address`, `[u8; N]`, \
		 `String<N>`, `Vec<T, N>`, or `Option<T>` where `T` is one of these fixed types",
	)
}

fn unsupported(ty: &Type) -> syn::Error {
	syn::Error::new_spanned(
		ty,
		"unsupported Pina zero-copy field; expected an audited scalar, `Address`, `[u8; N]`, \
		 `String<N>`, `Vec<T, N>`, or `Option<T>` where `T` is one of these fixed types",
	)
}
