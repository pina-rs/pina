//! Closed field grammar for Pina's fixed zero-copy schemas.
//!
//! `PinaPod`'s derive intentionally supports a fallback through `ZcField`.
//! That extension point is useful for direct `PinaPod` users, but Pina cannot
//! safely accept it at an account or instruction boundary: even though
//! `ZcField` is an unsafe trait, an unknown implementation is outside Pina's
//! closed schema contract. Pina therefore accepts only the concrete
//! representations audited below, including the `fixed` crate's `FixedI*`
//! and `FixedU*` schema types whose every bit pattern is a valid value.

use pina_abi::SchemaConsts;
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

/// Rewrite named capacities in the schema to the numbers they evaluate to, and
/// return the proof tokens that keep the referenced constants alive.
///
/// A capacity may be a `const` item rather than a literal. Everything
/// downstream — the generated proofs, the ABI document, `MAX_SIZE`, and
/// `projected_bytes` — needs a concrete number, so the constants are resolved
/// here and the declaration is rewritten in place. A literal-only schema is
/// returned untouched, which keeps the two spellings byte-identical from this
/// point on.
///
/// A macro sees only the item it is expanding, so a constant declared elsewhere
/// in the crate is read back from source. The table is cached for the whole
/// crate compilation, so this costs one directory walk no matter how many
/// schemas the program declares.
pub(crate) fn resolve_capacities(item: &mut ItemStruct) -> proc_macro2::TokenStream {
	capacity_consts().normalize_item(item)
}

/// Build the constant table for the crate being expanded.
///
/// Both the expanding crate and the discovered program directory are scanned.
/// A Surfpool harness source-includes the program (`#[path =
/// "../../src/lib.rs"]`), so its macros expand with the harness's manifest
/// directory; scanning only that one would miss every constant the program
/// declares.
fn capacity_consts() -> &'static SchemaConsts {
	use std::sync::OnceLock;

	static CONSTS: OnceLock<SchemaConsts> = OnceLock::new();

	CONSTS.get_or_init(|| {
		let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
			return SchemaConsts::empty();
		};
		let manifest_dir = std::path::PathBuf::from(manifest_dir);

		let mut table = SchemaConsts::from_source_dir(&manifest_dir.join("src"));
		if let Some((program_dir, _)) = crate::migration::discover_program_dir() {
			let program_src = program_dir.join("src");
			if program_src != manifest_dir.join("src") {
				table.extend(SchemaConsts::from_source_dir(&program_src));
			}
		}

		table
	})
}

/// Validate a schema and emit compile-time type and layout proofs.
pub(crate) fn validate_fixed_schema(
	item: &ItemStruct,
	crate_path: &syn::Path,
	discriminator: &syn::Path,
	zc_name: &syn::Ident,
	extra_header_bytes: usize,
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

		field_proofs.push(mapping_proof(source, native, pod, crate_path));
		field_sizes.push(quote!(::core::mem::size_of::<#pod>()));
	}

	let expected_size = quote!(#discriminator::BYTES + #extra_header_bytes #(+ #field_sizes)*);

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
	pub(crate) tails: Vec<CompactTail>,
}

/// Size metadata for one compact tail.
pub(crate) struct CompactTail {
	pub(crate) name: syn::Ident,
	pub(crate) pod: proc_macro2::TokenStream,
	pub(crate) capacity: proc_macro2::TokenStream,
	pub(crate) optional: bool,
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
/// Compact fields form a suffix, matching `PinaPod`'s compact schema grammar.
/// Every tail length lives in the fixed header and the active payloads are
/// concatenated after it in declaration order.
pub(crate) fn validate_compact_schema(
	item: &ItemStruct,
	crate_path: &syn::Path,
	discriminator: &syn::Path,
	header_name: &syn::Ident,
	extra_header_bytes: usize,
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
	let mut tails = Vec::new();
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
				let pod = &audited.pod;
				field_proofs.push(mapping_proof(source, &audited.native, pod, crate_path));
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
				field_proofs.push(mapping_proof(
					&source,
					&audited.native,
					&audited.pod,
					crate_path,
				));
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
				tails.push(CompactTail {
					name: field.ident.clone().ok_or_else(|| {
						syn::Error::new_spanned(field, "compact tails must be named")
					})?,
					pod: quote!(::core::primitive::u8),
					capacity,
					optional,
				});
			}
			CompactField::Vec {
				element_source,
				element,
				capacity,
				prefix_size,
				optional,
			} => {
				seen_tail = true;
				let pod = &element.pod;
				field_proofs.push(mapping_proof(
					&element_source,
					&element.native,
					pod,
					crate_path,
				));
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
				tails.push(CompactTail {
					name: field.ident.clone().ok_or_else(|| {
						syn::Error::new_spanned(field, "compact tails must be named")
					})?,
					pod: pod.clone(),
					capacity,
					optional,
				});
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

	let expected_header = quote!(#discriminator::BYTES + #extra_header_bytes #(+ #header_sizes)*);
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

	Ok(CompactSchema { proofs, tails })
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
		literal_prefix_argument(arguments.args.iter().nth(1), 1, ty).map_err(|_| {
			syn::Error::new_spanned(ty, "compact `String` prefixes must use 1, 2, 4, or 8 bytes")
		})?
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
		// A capacity that survived normalization cannot be evaluated here, so
		// report it by name instead of the generic supported-forms error.
		Some(argument) => {
			return Err(capacity_argument_error(argument, "vector capacity"));
		}
		None => return Err(compact_supported_error(ty)),
	};
	let prefix_size = if is_alias {
		2
	} else {
		literal_prefix_argument(arguments.args.iter().nth(2), 2, ty).map_err(|_| {
			syn::Error::new_spanned(ty, "compact `Vec` prefixes must use 1, 2, 4, or 8 bytes")
		})?
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
	// `pinapod` implements `ZcField` for every accepted field type, including
	// the `f32`/`f64` primitives, so each field keeps its own spelling all the
	// way to the derive.
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

/// Report a capacity argument that is neither a literal nor a resolvable name.
///
/// A capacity written as a `const` item is resolved before the grammar runs, so
/// reaching this point means the argument is not one this crate can evaluate.
fn capacity_argument_error(argument: &GenericArgument, name: &str) -> syn::Error {
	let requirement = format!("{name} must be a length this crate can evaluate");

	match argument {
		GenericArgument::Const(value) => unresolved_capacity_error(value, &requirement),
		GenericArgument::Type(ty) => {
			let printed = expression_text(&quote!(#ty));
			syn::Error::new_spanned(
				ty,
				format!(
					"{requirement}, but `{printed}` is not a `const` item; write an integer \
					 literal, or declare a free `const NAME: usize = ...;` at the crate root or \
					 in a module (a value on a type is not resolved, and `const {printed}: usize` \
					 would not compile)"
				),
			)
		}
		other => syn::Error::new_spanned(other, requirement),
	}
}

/// Report a capacity that survived normalization, naming the expression.
///
/// A capacity written as a `const` item is resolved before the grammar runs, so
/// reaching this point means the expression cannot be evaluated at expansion
/// time — an associated constant, a function call, or a name this crate does not
/// declare. The message names the expression and the way out rather than
/// reporting a generic unsupported-field error.
fn unresolved_capacity_error(expression: &Expr, requirement: &str) -> syn::Error {
	let printed = expression_text(&quote!(#expression));
	syn::Error::new_spanned(
		expression,
		format!(
			"{requirement}, but `{printed}` cannot be evaluated at expansion time; write an \
			 integer literal, or declare a free `const NAME: usize = ...;` in this crate (Pina \
			 resolves named constants, including arithmetic over them)"
		),
	)
}

/// Render tokens the way the source would spell them.
fn expression_text(tokens: &proc_macro2::TokenStream) -> String {
	tokens.to_string().replace(" :: ", "::")
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

#[derive(Debug)]
struct AuditedField {
	native: proc_macro2::TokenStream,
	pod: proc_macro2::TokenStream,
}

impl AuditedField {
	fn new(native: proc_macro2::TokenStream, pod: proc_macro2::TokenStream) -> Self {
		Self { native, pod }
	}
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
		Type::Array(array) => classify_fixed_array(array, crate_path),
		Type::Path(path) if path.qself.is_none() => classify_path(ty, path, crate_path),
		other => Err(unsupported(other)),
	}
}

fn classify_fixed_array(
	array: &syn::TypeArray,
	crate_path: &syn::Path,
) -> syn::Result<AuditedField> {
	if !is_integer_literal(&array.len) {
		return Err(unresolved_capacity_error(
			&array.len,
			"`[T; N]` arrays require a length this crate can evaluate",
		));
	}

	// The element classifies through the same closed grammar, so audited
	// scalars, `Address`, nested byte arrays, and dynamic-free collections
	// compose: `[u64; 8]` stores `[PodU64; 8]` little-endian with no length
	// prefix, and validation recurses per element.
	let element = classify_fixed_type(&array.elem, crate_path).map_err(|error| {
		syn::Error::new(
			syn::spanned::Spanned::span(array.elem.as_ref()),
			format!("`[T; N]` array elements must be fixed Pina schema types: {error}"),
		)
	})?;
	let native_element = &element.native;
	let pod_element = &element.pod;
	let length = &array.len;

	Ok(AuditedField::new(
		quote!([#native_element; #length]),
		quote!([#pod_element; #length]),
	))
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
			Ok(AuditedField::new(
				{
					let ident = &segment.ident;
					quote!(#crate_path::#ident)
				},
				{
					let ident = &segment.ident;
					quote!(#crate_path::#ident)
				},
			))
		}
		"Address" => {
			Ok(AuditedField::new(
				quote!(#crate_path::Address),
				quote!(#crate_path::Address),
			))
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
		name if fixed_point_pod_name(name).is_some() => {
			Err(syn::Error::new_spanned(
				ty,
				format!(
					"`{name}` requires a fractional-bits parameter, for example `{name}<U16>`, \
					 and the `fixed` feature on `pina`"
				),
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
		// `FixedI*`/`FixedU*` types land here; every other parameterized name
		// falls through to the custom-mapping rejection.
		_ => classify_fixed_point(ty, segment, crate_path),
	}
}

/// Maps a `fixed` crate type name to the name of its audited alignment-one
/// storage pod.
///
/// The mapping mirrors `pinapod`'s `ZcField` implementations for
/// `FixedI*`/`FixedU*`: every bit pattern of the backing little-endian
/// integer is a valid fixed-point value, so storage needs no validity
/// metadata beyond the pod itself.
fn fixed_point_pod_name(name: &str) -> Option<&'static str> {
	match name {
		"FixedI8" => Some("i8"),
		"FixedI16" => Some("PodI16"),
		"FixedI32" => Some("PodI32"),
		"FixedI64" => Some("PodI64"),
		"FixedI128" => Some("PodI128"),
		"FixedU8" => Some("u8"),
		"FixedU16" => Some("PodU16"),
		"FixedU32" => Some("PodU32"),
		"FixedU64" => Some("PodU64"),
		"FixedU128" => Some("PodU128"),
		_ => None,
	}
}

fn classify_fixed_point(
	ty: &Type,
	segment: &syn::PathSegment,
	crate_path: &syn::Path,
) -> syn::Result<AuditedField> {
	let name = segment.ident.to_string();
	let Some(pod_name) = fixed_point_pod_name(&name) else {
		return Err(custom_mapping(ty));
	};

	let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
		return Err(syn::Error::new_spanned(ty, "unsupported generic form"));
	};

	if arguments.args.len() != 1 {
		return Err(syn::Error::new_spanned(
			ty,
			format!(
				"`{name}<Frac>` requires exactly one fractional-bits parameter, for example \
				 `{name}<U16>`"
			),
		));
	}

	let pod = if pod_name.starts_with("Pod") {
		let pod = proc_macro2::Ident::new(pod_name, segment.ident.span());
		quote!(#crate_path::#pod)
	} else {
		let pod = proc_macro2::Ident::new(pod_name, segment.ident.span());
		quote!(::core::primitive::#pod)
	};

	// The fractional-bits parameter is a `typenum` type-level integer; echo it
	// verbatim and let the compiler reject spellings that do not form a real
	// `fixed` type. The source spelling is reused as the native type so the
	// identity proof in `validate_fixed_schema` binds the user's own import.
	Ok(AuditedField::new(quote!(#ty), pod))
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

	Ok(AuditedField::new(native.clone(), native))
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
	let element_native = classify_fixed_type(element, crate_path)?.native;
	let capacity = literal_const_argument(arguments.args.iter().nth(1), ty, "vector capacity")?;
	let prefix_size = if is_alias {
		2
	} else {
		literal_prefix_argument(arguments.args.iter().nth(2), 2, ty)?
	};
	// `pinapod`'s `Vec<T, N>` alias normalizes through `T: ZcField`, and that
	// mapping exists for every accepted element type, so the alias can carry
	// the element exactly as declared.
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
		// Float fields convert to and from their bit pattern under the hood;
		// storage is the alignment-one `PodF32`/`PodF64` byte container.
		"f32" => audited_native_scalar(&segment.ident, quote!(#crate_path::PodF32)),
		"f64" => audited_native_scalar(&segment.ident, quote!(#crate_path::PodF64)),
		_ => return None,
	})
}

fn audited_direct_scalar(ident: &syn::Ident) -> AuditedField {
	AuditedField::new(
		quote!(::core::primitive::#ident),
		quote!(::core::primitive::#ident),
	)
}

fn audited_native_scalar(ident: &syn::Ident, pod: proc_macro2::TokenStream) -> AuditedField {
	AuditedField::new(quote!(::core::primitive::#ident), pod)
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
		Some(argument) => Err(capacity_argument_error(argument, name)),
		None => {
			Err(syn::Error::new_spanned(
				ty,
				format!("{name} must be an integer literal or a `const` item in this crate"),
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
		 prove their alignment and bit validity; use an audited scalar, `Address`, `[T; N]` where \
		 `T` is a fixed type, `FixedI*<Frac>`, `FixedU*<Frac>`, `String<N>`, `Vec<T, N>`, or \
		 `Option<T>` where `T` is one of these fixed types",
	)
}

fn unsupported(ty: &Type) -> syn::Error {
	syn::Error::new_spanned(
		ty,
		"unsupported Pina zero-copy field; expected an audited scalar, `Address`, `[T; N]` where \
		 `T` is a fixed type, `FixedI*<Frac>`, `FixedU*<Frac>`, `String<N>`, `Vec<T, N>`, or \
		 `Option<T>` where `T` is one of these fixed types",
	)
}

#[cfg(test)]
mod tests {
	use quote::ToTokens as _;

	use super::*;

	fn classify(ty: Type) -> Result<AuditedField, syn::Error> {
		let crate_path: syn::Path = syn::parse_quote!(pina);
		classify_fixed_type(&ty, &crate_path)
	}

	fn classify_spelled(ty: &str) -> Result<AuditedField, syn::Error> {
		let parsed: Type = syn::parse_str(ty).expect("test type parses");
		classify(parsed)
	}

	#[test]
	fn classifies_fixed_point_scalars_to_their_integer_pods() {
		let cases: &[(&str, &str)] = &[
			("FixedI8<U1>", ":: core :: primitive :: i8"),
			("FixedI16<U2>", "pina :: PodI16"),
			("FixedI32<U12>", "pina :: PodI32"),
			("FixedI64<U32>", "pina :: PodI64"),
			("FixedI128<U64>", "pina :: PodI128"),
			("FixedU8<U4>", ":: core :: primitive :: u8"),
			("FixedU16<U8>", "pina :: PodU16"),
			("FixedU32<U16>", "pina :: PodU32"),
			("FixedU64<U16>", "pina :: PodU64"),
			("FixedU128<U127>", "pina :: PodU128"),
		];

		for (source, pod) in cases {
			let audited = classify_spelled(source)
				.unwrap_or_else(|error| panic!("`{source}` should classify: {error}"));
			assert_eq!(
				audited.pod.to_string(),
				*pod,
				"unexpected pod mapping for `{source}`"
			);
			assert_eq!(
				audited.native.to_string(),
				syn::parse_str::<Type>(source)
					.expect("test type parses")
					.to_token_stream()
					.to_string(),
				"native type must echo the source spelling for `{source}`"
			);
		}
	}

	#[test]
	fn classifies_qualified_fixed_point_paths() {
		let audited = classify_spelled("pina::fixed::FixedU64<U16>")
			.unwrap_or_else(|error| panic!("qualified path should classify: {error}"));
		assert_eq!(audited.pod.to_string(), "pina :: PodU64");
		assert_eq!(
			audited.native.to_string(),
			"pina :: fixed :: FixedU64 < U16 >"
		);
	}

	#[test]
	fn classifies_typed_arrays_through_their_element_pods() {
		let cases: &[(&str, &str, &str)] = &[
			(
				"[u8; 32]",
				"[:: core :: primitive :: u8 ; 32]",
				"[:: core :: primitive :: u8 ; 32]",
			),
			(
				"[u16; 4]",
				"[:: core :: primitive :: u16 ; 4]",
				"[pina :: PodU16 ; 4]",
			),
			(
				"[u64; 8]",
				"[:: core :: primitive :: u64 ; 8]",
				"[pina :: PodU64 ; 8]",
			),
			(
				"[PodU64; 8]",
				"[pina :: PodU64 ; 8]",
				"[pina :: PodU64 ; 8]",
			),
			(
				"[bool; 2]",
				"[:: core :: primitive :: bool ; 2]",
				"[pina :: PodBool ; 2]",
			),
			(
				"[Address; 4]",
				"[pina :: Address ; 4]",
				"[pina :: Address ; 4]",
			),
			(
				"[[u8; 4]; 2]",
				"[[:: core :: primitive :: u8 ; 4] ; 2]",
				"[[:: core :: primitive :: u8 ; 4] ; 2]",
			),
			(
				"[FixedU64<U16>; 2]",
				"[FixedU64 < U16 > ; 2]",
				"[pina :: PodU64 ; 2]",
			),
		];

		for (source, native, pod) in cases {
			let audited = classify_spelled(source)
				.unwrap_or_else(|error| panic!("`{source}` should classify: {error}"));
			assert_eq!(
				&audited.native.to_string(),
				native,
				"unexpected native mapping for `{source}`"
			);
			assert_eq!(
				&audited.pod.to_string(),
				pod,
				"unexpected pod mapping for `{source}`"
			);
		}
	}

	#[test]
	fn rejects_typed_arrays_with_non_literal_lengths() {
		let error = classify_spelled("[u64; WIDTH]")
			.expect_err("non-literal array lengths must be rejected");
		// The message must name the expression and the workaround, not report a
		// generic unsupported field: the reader needs to know `WIDTH` is what
		// could not be resolved, and a free `const` is what to write.
		assert!(
			error.to_string().contains("WIDTH"),
			"the diagnostic must name the expression: {error}"
		);
		assert!(
			error.to_string().contains("free `const NAME: usize"),
			"the diagnostic must point at the workaround: {error}"
		);
	}

	#[test]
	fn rejects_typed_array_elements_outside_the_closed_grammar() {
		let error =
			classify_spelled("[char; 4]").expect_err("restricted-domain elements must be rejected");
		assert!(
			error
				.to_string()
				.contains("array elements must be fixed Pina schema types"),
			"unexpected error: {error}"
		);
	}

	#[test]
	fn classifies_fixed_point_inside_collections() {
		// The storage pod of `Vec<T, N>` carries the element's native type;
		// `ZcField` resolves it to the integer pod at the type level.
		let vector = classify_spelled("Vec<FixedU64<U16>, 4>")
			.unwrap_or_else(|error| panic!("fixed vector should classify: {error}"));
		assert!(vector.pod.to_string().contains("FixedU64 < U16 >"));

		let option = classify_spelled("Option<FixedI32<U24>>")
			.unwrap_or_else(|error| panic!("fixed option should classify: {error}"));
		assert!(option.pod.to_string().contains("PodI32"));
	}

	#[test]
	fn rejects_bare_fixed_point_names() {
		for name in [
			"FixedI8",
			"FixedI16",
			"FixedI32",
			"FixedI64",
			"FixedI128",
			"FixedU8",
			"FixedU16",
			"FixedU32",
			"FixedU64",
			"FixedU128",
		] {
			let error =
				classify_spelled(name).expect_err("bare fixed-point names must be rejected");
			assert!(
				error
					.to_string()
					.contains("requires a fractional-bits parameter"),
				"unexpected error for `{name}`: {error}"
			);
		}
	}

	#[test]
	fn rejects_extra_fixed_point_arguments() {
		let two = classify_spelled("FixedU64<U16, U32>")
			.expect_err("two generic arguments must be rejected");
		assert!(two.to_string().contains("requires exactly one"));
	}

	#[test]
	fn rejects_unknown_parameterized_types_as_custom_mappings() {
		let error =
			classify_spelled("CustomType<u32>").expect_err("custom mappings must be rejected");
		assert!(error.to_string().contains("custom `ZcField` mappings"));
	}

	#[test]
	fn classifies_float_scalars_to_their_float_pods() {
		for (source, pod) in [("f32", "pina :: PodF32"), ("f64", "pina :: PodF64")] {
			let audited = classify_spelled(source)
				.unwrap_or_else(|error| panic!("`{source}` should classify: {error}"));
			// The field keeps its own spelling; `pinapod` supplies the mapping.
			assert_eq!(
				audited.native.to_string(),
				format!(":: core :: primitive :: {source}")
			);
			assert_eq!(audited.pod.to_string(), pod);
		}
	}

	#[test]
	fn classifies_floats_inside_collections() {
		// `pinapod` implements `ZcField` for `f32`/`f64`, so the `Vec<T, N>`
		// alias normalizes the element straight through.
		let vector = classify_spelled("Vec<f32, 4>")
			.unwrap_or_else(|error| panic!("float vector should classify: {error}"));
		assert_eq!(
			vector.native.to_string(),
			"pina :: Vec < :: core :: primitive :: f32 , 4 >"
		);
		assert_eq!(
			vector.pod.to_string(),
			"pina :: Vec < :: core :: primitive :: f32 , 4 >"
		);

		// `Option<T>` stores the mapped pod payload, so the pod spelling names
		// `PodF64` while the native spelling keeps the primitive.
		let option = classify_spelled("Option<f64>")
			.unwrap_or_else(|error| panic!("float option should classify: {error}"));
		assert_eq!(
			option.native.to_string(),
			":: core :: option :: Option < :: core :: primitive :: f64 >"
		);
		assert_eq!(
			option.pod.to_string(),
			"pina :: PodOption < pina :: PodF64 >"
		);
	}

	#[test]
	fn float_schema_proofs_bind_the_native_spelling() {
		let crate_path: syn::Path = syn::parse_quote!(pina);
		let item: ItemStruct = syn::parse_quote! {
			struct FloatState {
				reading: f32,
			}
		};
		let discriminator: syn::Path = syn::parse_quote!(Discriminator);
		let zc_name: syn::Ident = syn::parse_quote!(FloatStateZc);
		let proofs = super::validate_fixed_schema(&item, &crate_path, &discriminator, &zc_name, 0)
			.expect("float schema should validate");
		let expanded = proofs.to_string();

		assert!(
			expanded.contains("pina :: PodF32"),
			"pod proof missing: {expanded}"
		);
		assert!(
			expanded.contains("f32"),
			"native identity proof missing: {expanded}"
		);
	}

	#[test]
	fn fixed_point_schema_proofs_bind_the_source_type() {
		let crate_path: syn::Path = syn::parse_quote!(pina);
		let item: ItemStruct = syn::parse_quote! {
			struct PriceState {
				price: FixedU64<U16>,
				authority: pina::Address,
			}
		};
		let discriminator: syn::Path = syn::parse_quote!(Discriminator);
		let zc_name: syn::Ident = syn::parse_quote!(PriceStateZc);
		let proofs = super::validate_fixed_schema(&item, &crate_path, &discriminator, &zc_name, 0)
			.expect("fixed-point schema should validate");
		let expanded = proofs.to_string();

		assert!(
			expanded.contains("pina :: PodU64"),
			"pod proof missing: {expanded}"
		);
		assert!(
			expanded.contains("FixedU64 < U16 >"),
			"source-type proof missing: {expanded}"
		);
	}
}
