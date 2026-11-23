//! Expansion for `#[derive(Accounts)]`.

use darling::FromDeriveInput;
use darling::ast::Style;
use quote::quote;
use syn::DeriveInput;
#[cfg(feature = "validation")]
use syn::Expr;
#[cfg(feature = "validation")]
use syn::Ident;
use syn::Type;

#[cfg(feature = "validation")]
use crate::args::AccountsField;
use crate::args::AccountsInput;
#[cfg(feature = "validation")]
use crate::args::AccountsValidation;

pub(crate) fn expand(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
	// Parse input
	let input: DeriveInput = match syn::parse2(input) {
		Ok(v) => v,
		Err(e) => return e.to_compile_error(),
	};

	let args = match AccountsInput::from_derive_input(&input) {
		Ok(v) => v,
		Err(error) => {
			let reason = crate::validation::darling_reason(&error);

			return syn::Error::new(
				error.span(),
				format!(
					"could not parse the `#[derive(Accounts)]` input: {reason} Struct options are \
					 `crate = path` and `validate(with = function)`. Field options are \
					 `remaining`, `distinct`, `unique`, and `validate(...)`; account validation \
					 rules are `signer`, `writable`, `executable`, `address`, `addresses`, \
					 `owner`, `owners`, `program`, `sysvar`, `empty`, `not_empty`, `data_len`, \
					 `distinct_from`, and `error`"
				),
			)
			.to_compile_error();
		}
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
	#[cfg(not(feature = "validation"))]
	if args.validate.is_some() || fields.iter().any(|field| !field.validate.is_empty()) {
		return syn::Error::new_spanned(
			&args.ident,
			"`#[pina(validate(...))]` requires Pina's `validation` feature; enable it with `pina \
			 = { version = \"...\", features = [\"validation\"] }` (or enable \
			 `pina_macros/validation` when using the proc-macro crate directly)",
		)
		.to_compile_error();
	}

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
	let mut field_kinds = Vec::new();
	let mut remaining_field = None;
	let field_count = fields.len();
	let mut seen_remaining = false;

	for field in fields.iter() {
		if field.distinct.is_some() && !field.remaining.is_present() {
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
			if field.unique.is_present() {
				return syn::Error::new_spanned(
					&field.ident,
					"`#[pina(unique)]` cannot be applied to a `remaining` account slice; mutable \
					 remaining slices already enforce distinctness by default",
				)
				.to_compile_error();
			}
			if !field.validate.is_empty() {
				return syn::Error::new_spanned(
					&field.ident,
					"`#[pina(validate(...))]` cannot be applied to a `remaining` account slice; \
					 validate the slice in the struct-level `validate(with = function)` hook",
				)
				.to_compile_error();
			}
			if index + 1 != field_count {
				return syn::Error::new_spanned(
					&field.ident,
					"`#[pina(remaining)]` field must be the last field",
				)
				.to_compile_error();
			}

			let is_mut = is_mut_reference(&field.ty);
			if field.distinct.is_some() && !is_mut {
				return syn::Error::new_spanned(
					&field.ident,
					"`distinct` is only valid for mutable remaining account slices",
				)
				.to_compile_error();
			}

			remaining_field = field
				.ident
				.as_ref()
				.map(|ident| (ident, is_mut, is_mut && field.distinct.unwrap_or(true)));
			continue;
		}

		field_idents.push(ident);
		let field_kind = match account_field_kind(&field.ty) {
			Ok(kind) => kind,
			Err(error) => return error.to_compile_error(),
		};
		if field.unique.is_present() && field_kind == AccountFieldKind::Nested {
			return syn::Error::new_spanned(
				&field.ident,
				"`#[pina(unique)]` cannot be applied to a nested accounts struct; place \
				 `#[pina(unique)]` on the individual `AccountView` fields inside that struct",
			)
			.to_compile_error();
		}
		let parse_field = match field_kind {
			AccountFieldKind::Mutable => quote! { let #ident = cursor.next_mut()?; },
			AccountFieldKind::Immutable => quote! { let #ident = cursor.next()?; },
			AccountFieldKind::OptionalMutable => {
				quote! { let #ident = cursor.next_mut_opt()?; }
			}
			AccountFieldKind::OptionalImmutable => {
				quote! { let #ident = cursor.next_opt()?; }
			}
			AccountFieldKind::Nested => {
				let ty = &field.ty;
				quote! { let #ident = <#ty as #crate_path::ParseAccounts>::parse_accounts(cursor)?; }
			}
		};
		parse_fields.push(parse_field);
		field_kinds.push((field, field_kind));
	}

	// A positional field after an optional field is rejected at compile time.
	// An absent optional consumes its program-address filler slot, so an
	// account list that drops the filler while preserving the slot count makes
	// every later binding shift by one: the parser still runs each field's
	// property validations, but against the wrong accounts. Trailing optionals
	// and a trailing `remaining` slice are the supported shapes.
	for (index, (field, kind)) in field_kinds.iter().enumerate() {
		let optional = matches!(
			kind,
			AccountFieldKind::OptionalImmutable | AccountFieldKind::OptionalMutable
		);
		if !optional {
			continue;
		}
		for (later_field, later_kind) in &field_kinds[index + 1..] {
			if matches!(
				later_kind,
				AccountFieldKind::Immutable | AccountFieldKind::Mutable | AccountFieldKind::Nested
			) {
				return syn::Error::new_spanned(
					field
						.ident
						.as_ref()
						.expect("internal error: `Accounts` field without an ident"),
					format!(
						"optional account `{}` cannot be followed by positional account `{}`: an \
						 absent optional consumes its program-address filler slot, so dropping \
						 the filler shifts every later binding by one slot and each validation \
						 runs against the wrong account; move the optional fields to the end",
						field
							.ident
							.as_ref()
							.expect("internal error: `Accounts` field without an ident"),
						later_field
							.ident
							.as_ref()
							.expect("internal error: `Accounts` field without an ident"),
					),
				)
				.to_compile_error();
			}
		}
	}
	#[cfg(feature = "validation")]
	let validation_impl = match generate_validation_impl(
		struct_name,
		&args.generics,
		crate_path,
		&field_kinds,
		args.validate.as_ref(),
	) {
		Ok(value) => value,
		Err(error) => return error.to_compile_error(),
	};
	#[cfg(not(feature = "validation"))]
	let validation_impl = quote! {};
	#[cfg(feature = "validation")]
	let validate_parsed = quote! {
		<#struct_name #ty_generics as #crate_path::PinaValidate>::validate(&parsed)?;
	};
	#[cfg(not(feature = "validation"))]
	let validate_parsed = quote! {};
	#[cfg(feature = "validation")]
	let validate_accounts = quote! {
		#[inline]
		fn validate_accounts(&self) -> #crate_path::ProgramResult {
			<Self as #crate_path::PinaValidate>::validate(self)
		}
	};
	#[cfg(not(feature = "validation"))]
	let validate_accounts = quote! {};

	let finish_exact_const = remaining_field.is_none();
	let remaining_binding = remaining_field.map(|(field, is_mut, is_distinct)| {
		if is_mut && is_distinct {
			quote! { let #field = cursor.remaining_mut_distinct()?; }
		} else if is_mut {
			quote! { let #field = cursor.remaining_mut()?; }
		} else {
			quote! { let #field = cursor.take_remaining(); }
		}
	});
	let remaining_field_ident = remaining_field.map(|(field, ..)| quote!(#field,));

	// Positional fields contribute one slot each. A `remaining` slice contributes
	// one slot as well: it is the declaration the entrypoint reserves for, and a
	// trailing slice can hold more accounts than any constant could count.
	let account_bound = {
		let positional = field_kinds
			.iter()
			.filter(|(_, kind)| *kind != AccountFieldKind::Nested)
			.count() + usize::from(remaining_field.is_some());
		// A suffixed literal keeps `3.saturating_add(..)` from lexing as a float
		// member access.
		let positional = proc_macro2::Literal::usize_suffixed(positional);
		let nested = field_kinds
			.iter()
			.filter(|(_, kind)| *kind == AccountFieldKind::Nested)
			.map(|(field, _)| {
				let ty = &field.ty;

				quote! {
					.saturating_add(<#ty as #crate_path::ParseAccounts #ty_generics>::ACCOUNT_BOUND)
				}
			})
			.collect::<Vec<_>>();

		// The limit counts the same positional slots but has no slot for a
		// `remaining` slice: a struct with one consumes every account left.
		let limit = if remaining_field.is_some() {
			quote! { <Self as #crate_path::ParseAccounts #ty_generics>::UNBOUNDED }
		} else {
			let nested_limits = field_kinds
				.iter()
				.filter(|(_, kind)| *kind == AccountFieldKind::Nested)
				.map(|(field, _)| {
					let ty = &field.ty;

					quote! {
						.saturating_add(<#ty as #crate_path::ParseAccounts #ty_generics>::ACCOUNT_LIMIT)
					}
				});

			quote! { #positional #(#nested_limits)* }
		};

		// Only positional fields are required: an optional field or a
		// `remaining` slice may be absent.
		let minimum = {
			let required = field_kinds
				.iter()
				.filter(|(_, kind)| {
					matches!(
						kind,
						AccountFieldKind::Immutable | AccountFieldKind::Mutable
					)
				})
				.count();
			let required = proc_macro2::Literal::usize_suffixed(required);
			let nested_minimums = field_kinds
				.iter()
				.filter(|(_, kind)| *kind == AccountFieldKind::Nested)
				.map(|(field, _)| {
					let ty = &field.ty;

					quote! {
						.saturating_add(<#ty as #crate_path::ParseAccounts #ty_generics>::ACCOUNT_MINIMUM)
					}
				});

			quote! { #required #(#nested_minimums)* }
		};

		let alias_layout_steps = field_kinds
			.iter()
			.enumerate()
			.map(|(index, (field, kind))| {
				let is_last = index + 1 == field_kinds.len() && remaining_field.is_none();
				let is_unique = field.unique.is_present();
				let mark_unique = is_unique.then(|| {
					quote! {
						any_unique = true;
						unique_mask = #crate_path::__mask256_with_bit(unique_mask, offset);
					}
				});

				match kind {
					AccountFieldKind::Immutable => {
						quote! {
							if offset >= 256 {
								static_layout = false;
							}
							#mark_unique
							offset += 1;
						}
					}
					AccountFieldKind::Mutable => {
						quote! {
							if offset >= 256 {
								static_layout = false;
							}
							mut_mask = #crate_path::__mask256_with_bit(mut_mask, offset);
							#mark_unique
							offset += 1;
						}
					}
					AccountFieldKind::OptionalImmutable => {
						quote! {
							if offset >= 256 {
								static_layout = false;
							}
							optional_mask = #crate_path::__mask256_with_bit(optional_mask, offset);
							#mark_unique
							offset += 1;
						}
					}
					AccountFieldKind::OptionalMutable => {
						quote! {
							if offset >= 256 {
								static_layout = false;
							}
							mut_mask = #crate_path::__mask256_with_bit(mut_mask, offset);
							optional_mask = #crate_path::__mask256_with_bit(optional_mask, offset);
							#mark_unique
							offset += 1;
						}
					}
					AccountFieldKind::Nested => {
						let ty = &field.ty;
						quote! {
							{
								let nested_layout = <#ty as #crate_path::ParseAccounts #ty_generics>::ALIAS_LAYOUT;
								let nested_min = <#ty as #crate_path::ParseAccounts #ty_generics>::ACCOUNT_MINIMUM;
								let nested_limit = <#ty as #crate_path::ParseAccounts #ty_generics>::ACCOUNT_LIMIT;
								let nested_fixed = nested_min == nested_limit && nested_limit < 256;
								if #crate_path::__mask256_any(nested_layout.unique_mask) {
									any_unique = true;
								}
								if static_layout && nested_layout.static_layout && (nested_fixed || #is_last) {
									mut_mask = #crate_path::__mask256_or_shifted(
										mut_mask,
										nested_layout.mut_mask,
										offset,
									);
									optional_mask = #crate_path::__mask256_or_shifted(
										optional_mask,
										nested_layout.optional_mask,
										offset,
									);
									unique_mask = #crate_path::__mask256_or_shifted(
										unique_mask,
										nested_layout.unique_mask,
										offset,
									);
									if nested_layout.remaining_mut_start != usize::MAX {
										remaining_mut_start = offset + nested_layout.remaining_mut_start;
										remaining_mut_distinct = nested_layout.remaining_mut_distinct;
									}
									if nested_fixed {
										offset += nested_limit;
										if offset > 256 {
											static_layout = false;
										}
									} else if nested_limit != <#ty as #crate_path::ParseAccounts #ty_generics>::UNBOUNDED {
										if offset.saturating_add(nested_limit) > 256 {
											static_layout = false;
										}
									} else if offset
										.saturating_add(<#ty as #crate_path::ParseAccounts #ty_generics>::ACCOUNT_BOUND)
										> 256
									{
										static_layout = false;
									}
								} else {
									static_layout = false;
								}
							}
						}
					}
				}
			});

		let remaining_alias_step = remaining_field.and_then(|(_, is_mut, is_distinct)| {
			is_mut.then(|| {
				quote! {
					if static_layout {
						remaining_mut_start = offset;
						remaining_mut_distinct = #is_distinct;
					}
				}
			})
		});

		quote! {
			const ACCOUNT_BOUND: usize = #positional #(#nested)*;
			const ACCOUNT_LIMIT: usize = #limit;
			const ACCOUNT_MINIMUM: usize = #minimum;
			#[allow(unused_assignments, unused_mut)]
			const ALIAS_LAYOUT: #crate_path::__AliasLayout = {
				let mut static_layout = true;
				let mut mut_mask = [0_u64; 4];
				let mut optional_mask = [0_u64; 4];
				let mut unique_mask = [0_u64; 4];
				let mut remaining_mut_start = usize::MAX;
				let mut remaining_mut_distinct = true;
				let mut offset = 0_usize;
				let mut any_unique = false;

				#(#alias_layout_steps)*
				#remaining_alias_step

				assert!(
					!any_unique || static_layout,
					"`#[pina(unique)]` requires a statically known account layout; avoid placing \
					 `#[pina(unique)]` alongside hand-written or variable-length nested \
					 `ParseAccounts` fields before other fields",
				);

				#crate_path::__AliasLayout {
					static_layout,
					mut_mask,
					optional_mask,
					unique_mask,
					remaining_mut_start,
					remaining_mut_distinct,
				}
			};
		}
	};

	let has_unique_or_nested = field_kinds
		.iter()
		.any(|(field, kind)| field.unique.is_present() || *kind == AccountFieldKind::Nested);
	let child_unique_guard = has_unique_or_nested.then(|| {
		quote! {
			if const { #crate_path::__mask256_any(<Self as #crate_path::ParseAccounts #ty_generics>::ALIAS_LAYOUT.unique_mask) } {
				cursor.__validate_child_aliasing::<Self>()?;
			}
		}
	});

	quote! {
		impl #impl_generics #crate_path::ParseAccounts #ty_generics for #struct_name #ty_generics #where_clause {
			#account_bound

			fn parse_accounts(
				cursor: &mut #crate_path::AccountsCursor<#lifetime>,
			) -> ::core::result::Result<Self, #crate_path::ProgramError> {
				#child_unique_guard
				#(#parse_fields)*
				#remaining_binding

				Ok(Self {
					#(#field_idents,)*
					#remaining_field_ident
				})
			}

			#validate_accounts
		}

		impl #impl_generics #crate_path::TryFromAccountInfos #ty_generics for #struct_name #ty_generics #where_clause {
			const __USES_DERIVED_ROUTE: bool = true;

			fn try_from_account_infos(
				program_id: &#crate_path::Address,
				accounts: & #lifetime mut [#crate_path::AccountView],
			) -> ::core::result::Result<Self, #crate_path::ProgramError> {
				let parsed = #crate_path::__parse_derived_accounts::<Self, #finish_exact_const>(
					program_id,
					accounts,
				)?;
				#validate_parsed

				Ok(parsed)
			}

			#[inline(always)]
			fn try_from_route(
				program_id: &#crate_path::Address,
				route: #crate_path::RouteAccounts<#lifetime>,
			) -> ::core::result::Result<Self, #crate_path::ProgramError> {
				let parsed =
					#crate_path::__parse_derived_route::<Self, #finish_exact_const>(program_id, route)?;
				#validate_parsed

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

		#validation_impl
	}
}

#[cfg(feature = "validation")]
fn generate_validation_impl(
	struct_name: &Ident,
	generics: &syn::Generics,
	crate_path: &syn::Path,
	fields: &[(&AccountsField, AccountFieldKind)],
	hook: Option<&crate::args::ValidationHook>,
) -> syn::Result<proc_macro2::TokenStream> {
	let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
	let mut header_checks = Vec::new();
	let mut identity_checks = Vec::new();
	let mut data_checks = Vec::new();
	let mut relationship_checks = Vec::new();
	let mut nested_checks = Vec::new();

	for (field, kind) in fields {
		let ident = field
			.ident
			.as_ref()
			.unwrap_or_else(|| panic!("internal error: `Accounts` field without an ident"));

		if *kind == AccountFieldKind::Nested {
			if !field.validate.is_empty() {
				return Err(syn::Error::new_spanned(
					ident,
					"field-level account validators require `&AccountView`, `&mut AccountView`, \
					 `Option<&AccountView>`, or `Option<&mut AccountView>`; put validation for a \
					 nested accounts struct on that struct, or use the parent `validate(with = \
					 function)` hook",
				));
			}

			let ty = &field.ty;
			nested_checks.push(quote! {
				<#ty as #crate_path::ParseAccounts>::validate_accounts(&self.#ident)?;
			});
			continue;
		}

		validate_account_groups(ident, *kind, &field.validate)?;

		for group in &field.validate {
			let error = group.error.as_ref();

			if group.signer.is_present() {
				let call = quote! {
					<&#crate_path::AccountView as #crate_path::AccountInfoValidation>::assert_signer(
						__pina_account,
					)
				};
				let check = validation_call(&call, error, crate_path);
				header_checks.push(for_account(ident, *kind, &check));
			}

			if group.writable.is_present() {
				let call = quote! {
					<&#crate_path::AccountView as #crate_path::AccountInfoValidation>::assert_writable(
						__pina_account,
					)
				};
				let check = validation_call(&call, error, crate_path);
				header_checks.push(for_account(ident, *kind, &check));
			}

			if group.executable.is_present() {
				let call = quote! {
					<&#crate_path::AccountView as #crate_path::AccountInfoValidation>::assert_executable(
						__pina_account,
					)
				};
				let check = validation_call(&call, error, crate_path);
				header_checks.push(for_account(ident, *kind, &check));
			}

			for (value, method) in [
				(group.address.as_ref(), "assert_address"),
				(group.addresses.as_ref(), "assert_addresses"),
				(group.owner.as_ref(), "assert_owner"),
				(group.owners.as_ref(), "assert_owners"),
				(group.program.as_ref(), "assert_program"),
				(group.sysvar.as_ref(), "assert_sysvar"),
			] {
				let Some(value) = value else {
					continue;
				};
				let method = Ident::new(method, ident.span());
				let call = quote! {
					<&#crate_path::AccountView as #crate_path::AccountInfoValidation>::#method(
						__pina_account,
						&(#value),
					)
				};
				let check = validation_call(&call, error, crate_path);
				identity_checks.push(for_account(ident, *kind, &check));
			}

			if group.empty.is_present() {
				let call = quote! {
					<&#crate_path::AccountView as #crate_path::AccountInfoValidation>::assert_empty(
						__pina_account,
					)
				};
				let check = validation_call(&call, error, crate_path);
				data_checks.push(for_account(ident, *kind, &check));
			}

			if group.not_empty.is_present() {
				let call = quote! {
					<&#crate_path::AccountView as #crate_path::AccountInfoValidation>::assert_not_empty(
						__pina_account,
					)
				};
				let check = validation_call(&call, error, crate_path);
				data_checks.push(for_account(ident, *kind, &check));
			}

			if let Some(data_len) = &group.data_len {
				let call = quote! {
					<&#crate_path::AccountView as #crate_path::AccountInfoValidation>::assert_data_len(
						__pina_account,
						#data_len,
					)
				};
				let check = validation_call(&call, error, crate_path);
				data_checks.push(for_account(ident, *kind, &check));
			}

			if let Some(target) = &group.distinct_from {
				let target_kind = fields
					.iter()
					.find_map(|(candidate, kind)| {
						(candidate.ident.as_ref() == Some(target)).then_some(*kind)
					})
					.ok_or_else(|| {
						syn::Error::new_spanned(
							target,
							format!(
								"`distinct_from = {target}` on field `{ident}` must name another \
								 AccountView field in the same Accounts struct"
							),
						)
					})?;

				if target_kind == AccountFieldKind::Nested {
					return Err(syn::Error::new_spanned(
						target,
						"`distinct_from` must name an AccountView field, not a nested accounts \
						 struct",
					));
				}

				let left = optional_address(ident, *kind);
				let right = optional_address(target, target_kind);
				let failure = error.map_or_else(
					|| quote!(#crate_path::ProgramError::InvalidAccountData),
					|error| quote!((#error).into()),
				);
				relationship_checks.push(quote! {
					if matches!((#left, #right), (Some(left), Some(right)) if left == right) {
						return Err(#failure);
					}
				});
			}
		}
	}

	let hook = hook.map(|hook| {
		let with = &hook.with;

		quote! {
			#with(self)?;
		}
	});

	Ok(quote! {
		impl #impl_generics #crate_path::PinaValidate for #struct_name #ty_generics #where_clause {
			#[inline]
			fn validate(&self) -> #crate_path::ProgramResult {
				#(#header_checks)*
				#(#identity_checks)*
				#(#data_checks)*
				#(#relationship_checks)*
				#(#nested_checks)*
				#hook

				Ok(())
			}
		}
	})
}

#[cfg(feature = "validation")]
fn validate_account_groups(
	field: &Ident,
	kind: AccountFieldKind,
	groups: &[AccountsValidation],
) -> syn::Result<()> {
	let mut seen = AccountValidationSeen::default();

	for group in groups {
		let count = usize::from(group.signer.is_present())
			+ usize::from(group.writable.is_present())
			+ usize::from(group.executable.is_present())
			+ usize::from(group.address.is_some())
			+ usize::from(group.addresses.is_some())
			+ usize::from(group.owner.is_some())
			+ usize::from(group.owners.is_some())
			+ usize::from(group.program.is_some())
			+ usize::from(group.sysvar.is_some())
			+ usize::from(group.empty.is_present())
			+ usize::from(group.not_empty.is_present())
			+ usize::from(group.data_len.is_some())
			+ usize::from(group.distinct_from.is_some());

		if count == 0 {
			return Err(syn::Error::new_spanned(
				field,
				"empty account `validate(...)` annotation; add an account constraint such as \
				 `signer`, `address = ID`, or `owner = ID`, or remove the annotation",
			));
		}

		seen.record(group, field)?;
	}

	if seen.writable
		&& matches!(
			kind,
			AccountFieldKind::Mutable | AccountFieldKind::OptionalMutable
		) {
		return Err(syn::Error::new_spanned(
			field,
			format!(
				"`writable` validation on `{field}` is redundant because its mutable AccountView \
				 type already requires writability; remove `writable` and keep the mutable field \
				 type"
			),
		));
	}

	if seen.program && (seen.executable || seen.address || seen.addresses) {
		return Err(syn::Error::new_spanned(
			field,
			"`program` already checks both the account address and executable flag; remove \
			 `executable`, `address`, or `addresses` from this field",
		));
	}

	if seen.sysvar && (seen.address || seen.addresses || seen.owner || seen.owners || seen.program)
	{
		return Err(syn::Error::new_spanned(
			field,
			"`sysvar` already checks the canonical sysvar address and owner; remove `address`, \
			 `addresses`, `owner`, `owners`, or `program` from this field",
		));
	}

	if seen.address && seen.addresses {
		return Err(exclusive_error(field, "address", "addresses"));
	}

	if seen.owner && seen.owners {
		return Err(exclusive_error(field, "owner", "owners"));
	}

	if seen.empty && seen.not_empty {
		return Err(exclusive_error(field, "empty", "not_empty"));
	}

	Ok(())
}

#[cfg(feature = "validation")]
#[derive(Default)]
#[allow(clippy::struct_excessive_bools)]
struct AccountValidationSeen {
	signer: bool,
	writable: bool,
	executable: bool,
	address: bool,
	addresses: bool,
	owner: bool,
	owners: bool,
	program: bool,
	sysvar: bool,
	empty: bool,
	not_empty: bool,
	data_len: bool,
	distinct_from: Vec<String>,
}

#[cfg(feature = "validation")]
impl AccountValidationSeen {
	fn record(&mut self, group: &AccountsValidation, field: &Ident) -> syn::Result<()> {
		self.signer = record_flag(self.signer, group.signer.is_present(), "signer", field)?;
		self.writable = record_flag(
			self.writable,
			group.writable.is_present(),
			"writable",
			field,
		)?;
		self.executable = record_flag(
			self.executable,
			group.executable.is_present(),
			"executable",
			field,
		)?;
		self.address = record_option(self.address, group.address.as_ref(), "address", field)?;
		self.addresses =
			record_option(self.addresses, group.addresses.as_ref(), "addresses", field)?;
		self.owner = record_option(self.owner, group.owner.as_ref(), "owner", field)?;
		self.owners = record_option(self.owners, group.owners.as_ref(), "owners", field)?;
		self.program = record_option(self.program, group.program.as_ref(), "program", field)?;
		self.sysvar = record_option(self.sysvar, group.sysvar.as_ref(), "sysvar", field)?;
		self.empty = record_flag(self.empty, group.empty.is_present(), "empty", field)?;
		self.not_empty = record_flag(
			self.not_empty,
			group.not_empty.is_present(),
			"not_empty",
			field,
		)?;
		self.data_len = record_option(self.data_len, group.data_len.as_ref(), "data_len", field)?;

		if let Some(target) = &group.distinct_from {
			if target == field {
				return Err(syn::Error::new_spanned(
					target,
					"`distinct_from` cannot refer to its own field; name a different AccountView \
					 field",
				));
			}

			let target_name = target.to_string();
			if self.distinct_from.contains(&target_name) {
				return Err(duplicate_account_constraint(field, "distinct_from"));
			}
			self.distinct_from.push(target_name);
		}

		Ok(())
	}
}

#[cfg(feature = "validation")]
fn record_flag(seen: bool, present: bool, name: &str, field: &Ident) -> syn::Result<bool> {
	if seen && present {
		return Err(duplicate_account_constraint(field, name));
	}

	Ok(seen || present)
}

#[cfg(feature = "validation")]
fn record_option(seen: bool, value: Option<&Expr>, name: &str, field: &Ident) -> syn::Result<bool> {
	record_flag(seen, value.is_some(), name, field)
}

#[cfg(feature = "validation")]
fn duplicate_account_constraint(field: &Ident, name: &str) -> syn::Error {
	syn::Error::new_spanned(
		field,
		format!(
			"duplicate `{name}` validation on account field `{field}`; keep one declaration so \
			 validation order and its error are unambiguous"
		),
	)
}

#[cfg(feature = "validation")]
fn exclusive_error(field: &Ident, left: &str, right: &str) -> syn::Error {
	syn::Error::new_spanned(
		field,
		format!(
			"`{left}` and `{right}` are mutually exclusive on account field `{field}`; keep the \
			 rule that matches the accepted account set"
		),
	)
}

#[cfg(feature = "validation")]
fn validation_call(
	call: &proc_macro2::TokenStream,
	error: Option<&Expr>,
	crate_path: &syn::Path,
) -> proc_macro2::TokenStream {
	if let Some(error) = error {
		return quote! {
			if (#call).is_err() {
				return Err(::core::convert::Into::<#crate_path::ProgramError>::into(#error));
			}
		};
	}

	quote! {
		let _ = #call?;
	}
}

#[cfg(feature = "validation")]
fn for_account(
	field: &Ident,
	kind: AccountFieldKind,
	check: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	match kind {
		AccountFieldKind::Immutable | AccountFieldKind::Mutable => {
			quote! {
				{
					let __pina_account = &*self.#field;
					#check
				}
			}
		}
		AccountFieldKind::OptionalImmutable | AccountFieldKind::OptionalMutable => {
			quote! {
				if let Some(__pina_account) = self.#field.as_deref() {
					#check
				}
			}
		}
		AccountFieldKind::Nested => quote! {},
	}
}

#[cfg(feature = "validation")]
fn optional_address(field: &Ident, kind: AccountFieldKind) -> proc_macro2::TokenStream {
	match kind {
		AccountFieldKind::Immutable | AccountFieldKind::Mutable => {
			quote!(Some((&*self.#field).address()))
		}
		AccountFieldKind::OptionalImmutable | AccountFieldKind::OptionalMutable => {
			quote!(self.#field.as_deref().map(|account| account.address()))
		}
		AccountFieldKind::Nested => quote!(None),
	}
}

fn is_reference(ty: &Type) -> bool {
	matches!(ty, Type::Reference(_))
}

fn is_mut_reference(ty: &Type) -> bool {
	matches!(ty, Type::Reference(reference) if reference.mutability.is_some())
}

fn is_account_view(ty: &Type) -> bool {
	let Type::Path(type_path) = ty else {
		return false;
	};

	type_path
		.path
		.segments
		.last()
		.is_some_and(|segment| segment.ident == "AccountView")
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
		Type::Reference(reference)
			if is_account_view(&reference.elem) && reference.mutability.is_some() =>
		{
			Ok(Some(AccountFieldKind::OptionalMutable))
		}
		Type::Reference(reference) if is_account_view(&reference.elem) => {
			Ok(Some(AccountFieldKind::OptionalImmutable))
		}
		_ => {
			Err(syn::Error::new_spanned(
				ty,
				"only `Option<&AccountView>` and `Option<&mut AccountView>` fields are supported",
			))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn optional_fields_require_account_view_references() {
		for ty in [
			syn::parse_quote!(Option<&u64>),
			syn::parse_quote!(Option<&mut u64>),
			syn::parse_quote!(Option<&[AccountView]>),
		] {
			let error =
				account_field_kind(&ty).expect_err("non-AccountView references must be rejected");
			assert!(error.to_string().contains("Option<&AccountView>"));
		}

		assert_eq!(
			account_field_kind(&syn::parse_quote!(Option<&pina::AccountView>))
				.unwrap_or_else(|error| panic!("qualified AccountView reference: {error}")),
			AccountFieldKind::OptionalImmutable
		);
		assert_eq!(
			account_field_kind(&syn::parse_quote!(Option<&mut pina::AccountView>))
				.unwrap_or_else(|error| panic!("qualified mutable AccountView reference: {error}"),),
			AccountFieldKind::OptionalMutable
		);
	}

	/// An optional field may not precede a positional field: an absent optional
	/// consumes its program-address filler slot, so a dropped filler shifts
	/// every later binding by one slot.
	#[test]
	fn optional_fields_before_positional_fields_are_rejected() {
		let expansion = expand(quote! {
			#[derive(Accounts)]
			struct Shifted<'a> {
				authority: &'a AccountView,
				watcher: Option<&'a AccountView>,
				vault: &'a mut AccountView,
			}
		});
		let rendered = expansion.to_string();
		assert!(
			rendered.contains("cannot be followed by positional account"),
			"the derive must reject the shifted shape, got: {rendered}"
		);

		// A `remaining` slice after an optional is the supported shape: the
		// slice absorbs whatever follows, so no binding depends on filler
		// presence.
		let allowed = expand(quote! {
			#[derive(Accounts)]
			struct Trailing<'a> {
				authority: &'a AccountView,
				treasury: Option<&'a mut AccountView>,
				#[pina(remaining)]
				members: &'a [AccountView],
			}
		});
		assert!(
			!allowed
				.to_string()
				.contains("cannot be followed by positional account"),
			"a trailing remaining slice must stay allowed"
		);
	}

	#[test]
	fn unique_attribute_rejects_remaining_and_nested_fields() {
		let on_remaining = expand(quote! {
			#[derive(Accounts)]
			struct BadRemaining<'a> {
				authority: &'a AccountView,
				#[pina(remaining, unique)]
				rest: &'a [AccountView],
			}
		})
		.to_string();
		assert!(
			on_remaining.contains("`#[pina(unique)]` cannot be applied to a `remaining`"),
			"expected error for unique on remaining, got: {on_remaining}"
		);

		let on_nested = expand(quote! {
			#[derive(Accounts)]
			struct BadNested<'a> {
				#[pina(unique)]
				inner: NestedGroup<'a>,
			}
		})
		.to_string();
		assert!(
			on_nested.contains("`#[pina(unique)]` cannot be applied to a nested accounts struct"),
			"expected error for unique on nested, got: {on_nested}"
		);

		let valid = expand(quote! {
			#[derive(Accounts)]
			struct GoodUnique<'a> {
				#[pina(unique)]
				authority: &'a AccountView,
				#[pina(unique)]
				vault: &'a mut AccountView,
				#[pina(unique)]
				opt: Option<&'a AccountView>,
				#[pina(unique)]
				opt_mut: Option<&'a mut AccountView>,
			}
		})
		.to_string();
		assert!(valid.contains("__mask256_with_bit"));
		assert!(!valid.contains("compile_error"));

		let unknown_option = expand(quote! {
			#[derive(Accounts)]
			struct UnknownOpt<'a> {
				#[pina(not_a_real_option)]
				authority: &'a AccountView,
			}
		})
		.to_string();
		assert!(unknown_option.contains("`unique`"));
	}
}
