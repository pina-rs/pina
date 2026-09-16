//! Expansion for `#[instruction_dispatch]`.
//!
//! Turns the instruction discriminator enum into the program's
//! `process_instruction` entrypoint, so consumers stop hand-writing the
//! discriminator → accounts → `process` match, the account-count constant, and
//! the reserved `Migrate` prelude.

use darling::FromMeta;
use darling::ast::NestedMeta;
use proc_macro2::Literal;
use quote::quote;
use quote::quote_spanned;
use syn::Ident;
use syn::ItemEnum;
use syn::Path;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;

use crate::args::CapacityTestArg;
use crate::args::DispatchVariantArgs;
use crate::args::InstructionDispatchArgs;

/// Suffix appended to a variant name to find its accounts struct.
const ACCOUNTS_SUFFIX: &str = "Accounts";

/// One resolved variant → accounts-struct route.
struct Route {
	/// The enum variant this route dispatches.
	variant: Ident,
	/// The accounts struct parsed for the variant.
	accounts: Path,
	/// Trailing segment of `accounts`, used in diagnostics and capacity tests.
	accounts_name: Ident,
}

pub(crate) fn expand(
	args: proc_macro2::TokenStream,
	input: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	let nested_metas = match NestedMeta::parse_meta_list(args) {
		Ok(value) => value,
		Err(error) => return error.into_compile_error(),
	};

	let args = match InstructionDispatchArgs::from_list(&nested_metas) {
		Ok(value) => value,
		Err(error) => {
			let reason = crate::validation::darling_reason(&error);

			return syn::Error::new(
				error.span(),
				format!(
					"could not parse the `#[instruction_dispatch(...)]` input: {reason} Supported \
					 arguments are `crate = path`, `migrations(Account, ...)`, \
					 `migrations_max_lamports = EXPR`, `capacity_test`, `maximum_accounts = EXPR`, \
					 and `program_id = EXPR`"
				),
			)
			.to_compile_error();
		}
	};

	let mut item_enum: ItemEnum = match syn::parse2(input) {
		Ok(value) => value,
		Err(error) => return error.to_compile_error(),
	};

	let crate_path = &args.crate_path;
	let enum_name = item_enum.ident.clone();
	let program_id = args
		.program_id
		.clone()
		.unwrap_or_else(|| syn::parse_quote!(ID));

	// Resolve every route before emitting anything: a diagnostic naming the
	// offending variant is worth more than a partially expanded program.
	let mut routes = Vec::with_capacity(item_enum.variants.len());
	for variant in &mut item_enum.variants {
		let variant_name = variant.ident.clone();
		let mut declared_accounts = None;

		for attribute in &variant.attrs {
			if !attribute.path().is_ident("dispatch") {
				continue;
			}
			if declared_accounts.is_some() {
				return syn::Error::new_spanned(
					attribute,
					format!("duplicate `#[dispatch(...)]` on variant `{variant_name}`"),
				)
				.to_compile_error();
			}

			let metas = match attribute.parse_args_with(
				Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
			) {
				Ok(value) => value,
				Err(error) => return error.to_compile_error(),
			};
			let parsed = match DispatchVariantArgs::from_list(
				&metas
					.into_iter()
					.map(NestedMeta::Meta)
					.collect::<Vec<_>>(),
			) {
				Ok(value) => value,
				Err(error) => {
					let reason = crate::validation::darling_reason(&error);

					return syn::Error::new_spanned(
						attribute,
						format!(
							"could not parse `#[dispatch(...)]` on variant `{variant_name}`: \
							 {reason} The only supported argument is `accounts = \
							 AccountsStruct`"
						),
					)
					.to_compile_error();
				}
			};
			declared_accounts = parsed.accounts;
		}

		let (accounts, accounts_name) = if let Some(accounts) = declared_accounts {
			let Some(segment) = accounts.segments.last() else {
				return syn::Error::new_spanned(&accounts, "`accounts` path cannot be empty")
					.to_compile_error();
			};
			let name = segment.ident.clone();

			(accounts, name)
		} else {
			// Carrying the variant's span makes a missing accounts struct report
			// `cannot find type \`FooAccounts\`` at the variant that needs it.
			let name = Ident::new(
				&format!("{variant_name}{ACCOUNTS_SUFFIX}"),
				variant_name.span(),
			);
			let path: Path = syn::parse_quote!(#name);

			(path, name)
		};

		routes.push(Route {
			variant: variant_name,
			accounts,
			accounts_name,
		});
	}

	if routes.is_empty() {
		return syn::Error::new_spanned(
			&enum_name,
			"`#[instruction_dispatch]` requires at least one instruction variant",
		)
		.to_compile_error();
	}

	// The per-variant attributes belong to this macro's grammar, not to the
	// enum's own API, so they must not survive into the emitted enum.
	for variant in &mut item_enum.variants {
		variant
			.attrs
			.retain(|attribute| !attribute.path().is_ident("dispatch"));
	}

	let migrations = match resolve_migrations(&args, &enum_name) {
		Ok(value) => value,
		Err(error) => return error.to_compile_error(),
	};

	// The generated arms and bounds both name the accounts type. Building those
	// tokens with the variant's span keeps a missing accounts struct pointing at
	// the variant the caller wrote, instead of at the attribute.
	let dispatch_arms = routes.iter().map(|route| {
		let Route {
			variant,
			accounts,
			..
		} = route;

		quote_spanned! {variant.span()=>
			#enum_name::#variant => {
				<#accounts as ::core::convert::TryFrom<(
					& #crate_path::Address,
					&mut [#crate_path::AccountView],
				)>>::try_from((program_id, accounts))?.process(data)
			}
		}
	});

	let route_bound = |route: &Route| {
		let accounts = &route.accounts;

		quote_spanned! {accounts.span()=>
			{
				const fn __pina_account_bound<'a, T>() -> usize
				where
					T: #crate_path::ParseAccounts<'a>,
				{
					<T as #crate_path::ParseAccounts<'a>>::ACCOUNT_BOUND
				}
				__pina_account_bound::<'static, #accounts>()
			}
		}
	};
	let bound_values = routes.iter().map(route_bound).collect::<Vec<_>>();
	let bound_count = Literal::usize_unsuffixed(bound_values.len());

	let maximum_accounts = if let Some(expression) = &args.maximum_accounts {
		quote!(#expression)
	} else {
		quote!(#crate_path::pinocchio::MAX_TX_ACCOUNTS)
	};

	let capacity_test = args
		.capacity_test
		.unwrap_or(CapacityTestArg::Enabled)
		.is_enabled()
		.then(|| {
			let assertions = routes.iter().map(|route| {
				let name = route.accounts_name.to_string();
				let bound = route_bound(route);

				quote! {
					const _: () = assert!(
						#bound <= MAX_INSTRUCTION_ACCOUNTS,
						concat!(
							"MAX_INSTRUCTION_ACCOUNTS must cover `",
							#name,
							"`; the constant is derived from every declared instruction's \
							 ACCOUNT_BOUND, so a smaller value means a route was missed",
						),
					);
				}
			});

			quote! {
				#[cfg(test)]
				const _: () = {
					// The entrypoint's account array is what the cap protects, so the
					// assertions below compare declared slots against the same
					// element type the runtime writes into it.
					const ELEMENT: usize = ::core::mem::size_of::<#crate_path::AccountView>();
					const _: () = assert!(ELEMENT > 0, "AccountView must occupy stack");
					const _: () = assert!(
						MAX_INSTRUCTION_ACCOUNTS <= #crate_path::pinocchio::MAX_TX_ACCOUNTS,
						"MAX_INSTRUCTION_ACCOUNTS must not exceed the entrypoint's account array",
					);

					#(#assertions)*
				};
			}
		});

	let (migrate_helper, migrate_prelude) = match migrations {
		Some((helper, prelude)) => (Some(helper), Some(prelude)),
		None => (None, None),
	};

	quote! {
		#item_enum

		#migrate_helper

		/// Upper bound on the accounts this program reads in one instruction.
		///
		/// Derived from every instruction's declared `ACCOUNT_BOUND` and
		/// saturated at the entrypoint's account array, so a variant that
		/// declares an unbounded trailing slice cannot inflate the cap.
		pub const MAX_INSTRUCTION_ACCOUNTS: usize = {
			const fn maximum(values: [usize; #bound_count]) -> usize {
				let mut index = 0;
				let mut highest = 0;
				while index < values.len() {
					if values[index] > highest {
						highest = values[index];
					}
					index += 1;
				}
				highest
			}
			const fn clamp(value: usize, limit: usize) -> usize {
				if value > limit { limit } else { value }
			}

			clamp(maximum([#(#bound_values),*]), #maximum_accounts)
		};

		#capacity_test

		#[inline(always)]
		pub fn process_instruction(
			program_id: & #crate_path::Address,
			accounts: &mut [#crate_path::AccountView],
			data: &[u8],
		) -> #crate_path::ProgramResult {
			#migrate_prelude

			let instruction: #enum_name =
				#crate_path::parse_instruction(program_id, & #program_id, data)?;

			match instruction {
				#(#dispatch_arms),*
			}
		}
	}
}

/// Resolve the optional reserved-`Migrate` prelude.
///
/// Returns the `process_migrate` helper and the `is_migrate_instruction`
/// guard, or `None` when the program is not migration-aware.
fn resolve_migrations(
	args: &InstructionDispatchArgs,
	enum_name: &Ident,
) -> syn::Result<Option<(proc_macro2::TokenStream, proc_macro2::TokenStream)>> {
	let crate_path = &args.crate_path;
	let Some(ladder) = &args.migrations else {
		if args.migrations_max_lamports.is_some() {
			return Err(syn::Error::new_spanned(
				enum_name,
				"`migrations_max_lamports` requires `migrations(Account, ...)`",
			));
		}

		return Ok(None);
	};

	if ladder.is_empty() {
		return Err(syn::Error::new_spanned(
			enum_name,
			"`migrations(...)` must name at least one migratable contract",
		));
	}

	let Some(max_lamports) = &args.migrations_max_lamports else {
		return Err(syn::Error::new_spanned(
			enum_name,
			"`migrations(...)` requires `migrations_max_lamports = EXPR`: the budget is \
			 program policy, and a default would silently misprice rent transfers",
		));
	};

	// Every listed contract must carry a checked-in history, because the
	// generated ladder calls `MigratableAccount` and a missing manifest entry
	// surfaces later as an unsatisfied trait bound instead of the remedy.
	crate::migration::verify_migration_contracts(enum_name, ladder)?;

	let steps = ladder.iter().enumerate().map(|(position, account)| {
		// Slots 0 and 1 are the payer and the system program.
		let index = Literal::usize_unsuffixed(position + 2);

		quote! {
			migrate.run_optional::<#account>(#index)?;
		}
	});

	// The helper is emitted outside the entrypoint module so a caller that wires
	// its own `nostd_entrypoint!` can still reach it.
	let helper = quote! {
		/// Reserved framework `Migrate` instruction.
		///
		/// Slots are `[payer, systemProgram, ...migratable]`. A slot holding the
		/// program address, or an index past the end of the slice, is treated as
		/// omitted, so a client sends only the accounts it needs and every slot
		/// draws from one shared lamport budget.
		///
		/// # Errors
		///
		/// Inherits `MigrateContext`'s layout validation and each slot's
		/// migration failures.
		pub fn process_migrate(
			program_id: & #crate_path::Address,
			accounts: &mut [#crate_path::AccountView],
		) -> #crate_path::ProgramResult {
			let mut migrate = #crate_path::MigrateContext::new(program_id, accounts, #max_lamports)?;
			#(#steps)*

			Ok(())
		}
	};

	let prelude = quote! {
		if #crate_path::is_migrate_instruction(data) {
			return process_migrate(program_id, accounts);
		}
	};

	Ok(Some((helper, prelude)))
}
