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
use syn::Expr;
use syn::Ident;
use syn::ItemEnum;
use syn::Path;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;

use crate::args::CapacityTestArg;
use crate::args::DispatchVariantArgs;
use crate::args::InlineArg;
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
					 `migrations_max_lamports = EXPR`, `capacity_test`, `maximum_accounts = \
					 EXPR`, and `program_id = EXPR`"
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

			let metas = match attribute
				.parse_args_with(Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)
			{
				Ok(value) => value,
				Err(error) => return error.to_compile_error(),
			};
			let parsed = match DispatchVariantArgs::from_list(
				&metas.into_iter().map(NestedMeta::Meta).collect::<Vec<_>>(),
			) {
				Ok(value) => value,
				Err(error) => {
					let reason = crate::validation::darling_reason(&error);

					return syn::Error::new_spanned(
						attribute,
						format!(
							"could not parse `#[dispatch(...)]` on variant `{variant_name}`: \
							 {reason} The only supported argument is `accounts = AccountsStruct`"
						),
					)
					.to_compile_error();
				}
			};
			declared_accounts = parsed.accounts;
		}

		let (accounts, accounts_name) = if let Some(accounts) = declared_accounts {
			// Darling parses the value as a `syn::Path`, which always carries at
			// least one segment.
			let name = accounts
				.segments
				.last()
				.expect("darling parses `accounts` as a non-empty path")
				.ident
				.clone();

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
			variant, accounts, ..
		} = route;

		quote_spanned! {variant.span()=>
			#enum_name::#variant => {
				// Trait-qualified so programs with explicit imports compile
				// without relying on `ProcessAccountInfos` being in scope.
				let __pina_accounts = <#accounts as ::core::convert::TryFrom<(
					& #crate_path::Address,
					&mut [#crate_path::AccountView],
				)>>::try_from((program_id, accounts))?;

				<#accounts as #crate_path::ProcessAccountInfos>::process(__pina_accounts, data)
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

	// The hand-written dispatchers this replaces are all inlined, but the two
	// spellings are not equivalent at the codegen level: a program measured with
	// `#[inline]` grew by 240 bytes once the generated dispatcher was inlined
	// unconditionally. Callers keep the spelling their program was measured with.
	let inline_attribute: syn::Attribute = match args.inline.unwrap_or(InlineArg::Always) {
		InlineArg::Always => syn::parse_quote!(#[inline(always)]),
		InlineArg::Hint => syn::parse_quote!(#[inline]),
	};

	let capacity_test = args
		.capacity_test
		.unwrap_or(CapacityTestArg::Enabled)
		.is_enabled()
		.then(|| {
			let assertions = routes.iter().map(|route| {
				let name = route.accounts_name.to_string();
				let bound = route_bound(route);
				let accounts = &route.accounts;
				// The documented hand-written sentinel, resolved through the
				// concrete accounts type so the reference is unambiguous.
				let sentinel = quote! {
					{
						const fn __pina_unbounded<'a, T>() -> usize
						where
							T: #crate_path::ParseAccounts<'a>,
						{
							<T as #crate_path::ParseAccounts<'a>>::UNBOUNDED
						}
						__pina_unbounded::<'static, #accounts>()
					}
				};

				quote! {
					const _: () = assert!(
						#bound <= MAX_INSTRUCTION_ACCOUNTS || #bound == #sentinel,
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
		///
		/// This is the count a program declares, not a security boundary. Passing
		/// it to `nostd_entrypoint!` would size the runtime's account array below
		/// the transaction maximum, and the loader *skips* any account beyond that
		/// array instead of failing, so `finish_exact` would no longer reject an
		/// instruction that supplies too many accounts. Keep the entrypoint at its
		/// default maximum and use this constant as the declaration and test
		/// contract it is.
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

		#inline_attribute
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
			"`migrations(...)` requires `migrations_max_lamports = EXPR`: the budget is program \
			 policy, and a default would silently misprice rent transfers",
		));
	};

	// Every listed contract must carry a checked-in history, because the
	// generated ladder calls `MigratableAccount` and a missing manifest entry
	// surfaces later as an unsatisfied trait bound instead of the remedy.
	crate::migration::verify_migration_contracts(enum_name, ladder)?;

	Ok(Some(migrate_emission(
		crate_path,
		ladder,
		max_lamports,
		&args
			.program_id
			.clone()
			.unwrap_or_else(|| syn::parse_quote!(ID)),
	)))
}

/// Emit the reserved-`Migrate` helper and the dispatch prelude for a validated
/// ladder.
///
/// Split from [`resolve_migrations`] so the emitted shape is unit-testable
/// without a manifest on disk.
fn migrate_emission(
	crate_path: &Path,
	ladder: &[Path],
	max_lamports: &Expr,
	program_id: &Expr,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
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
		fn process_migrate(
			program_id: & #crate_path::Address,
			accounts: &mut [#crate_path::AccountView],
		) -> #crate_path::ProgramResult {
			let mut migrate = #crate_path::MigrateContext::new(program_id, accounts, #max_lamports)?;
			#(#steps)*

			Ok(())
		}
	};

	// The reserved instruction bypasses `parse_instruction`, so the configured
	// id is checked here: the loader would otherwise let a mismatched program
	// id run migrations that ordinary instructions reject. The comparison is
	// gated to the reserved path, leaving other instructions' cost unchanged.
	let prelude = quote! {
		if #crate_path::is_migrate_instruction(data) {
			if program_id != &#program_id {
				return Err(#crate_path::ProgramError::IncorrectProgramId);
			}

			return process_migrate(program_id, accounts);
		}
	};

	(helper, prelude)
}

#[cfg(test)]
mod tests {
	use proc_macro2::TokenStream;

	use super::*;
	use crate::args::CapacityTestArg;

	fn expand_with(args: TokenStream, input: TokenStream) -> String {
		squeezed(&expand(args, input).to_string())
	}

	fn dispatch_args(tokens: TokenStream) -> InstructionDispatchArgs {
		let nested = NestedMeta::parse_meta_list(tokens)
			.unwrap_or_else(|error| panic!("test args: {error}"));
		InstructionDispatchArgs::from_list(&nested)
			.unwrap_or_else(|error| panic!("test dispatch args: {error}"))
	}

	fn ident(name: &str) -> Ident {
		Ident::new(name, proc_macro2::Span::call_site())
	}

	fn ladder_of(name: &str) -> Path {
		syn::parse_str(name).unwrap_or_else(|error| panic!("test ladder path: {error}"))
	}

	/// Collapse whitespace so assertions match a token stream's normalized
	/// spacing rather than its pretty-printed form.
	fn squeezed(text: &str) -> String {
		text.split_whitespace().collect()
	}

	#[test]
	fn happy_path_emits_dispatch_constant_and_capacity_block() {
		let input: TokenStream = quote! {
			#[discriminator]
			pub enum CounterInstruction {
				Initialize = 0,
				Increment = 1,
			}
		};
		let expanded = expand_with(quote!(), input);

		assert!(expanded.contains("pubconstMAX_INSTRUCTION_ACCOUNTS:usize"));
		assert!(expanded.contains("pubfnprocess_instruction"));
		assert!(expanded.contains("CounterInstruction::Initialize"));
		// Variant `Foo` routes to `FooAccounts` by convention.
		assert!(expanded.contains("InitializeAccounts"));
		assert!(expanded.contains("IncrementAccounts"));
		assert!(
			expanded.contains("#[cfg(test)]"),
			"capacity block is test-gated"
		);
		assert!(expanded.contains("MAX_INSTRUCTION_ACCOUNTSmustnotexceed"));
		// The dispatcher is inlined by default.
		assert!(expanded.contains("#[inline(always)]"));
	}

	#[test]
	fn accounts_override_routes_the_named_struct() {
		let input: TokenStream = quote! {
			pub enum Mixed {
				Default = 0,
				#[dispatch(accounts = CustomAccounts)]
				Overridden = 1,
			}
		};
		let expanded = expand_with(quote!(), input);

		assert!(expanded.contains("DefaultAccounts"));
		assert!(expanded.contains("CustomAccounts"));
		// The override attribute is consumed, not re-emitted.
		assert!(!expanded.contains("#[dispatch("));
	}

	#[test]
	fn capacity_test_can_be_suppressed() {
		let input: TokenStream = quote! {
			pub enum CounterInstruction {
				Initialize = 0,
			}
		};
		let expanded = expand_with(quote!(capacity_test = false), input);

		assert!(expanded.contains("pubconstMAX_INSTRUCTION_ACCOUNTS:usize"));
		assert!(!expanded.contains("#[cfg(test)]"));
	}

	#[test]
	fn inline_hint_is_honoured() {
		let input: TokenStream = quote! {
			pub enum CounterInstruction {
				Initialize = 0,
			}
		};
		let expanded = expand_with(quote!(inline = "hint"), input);

		assert!(expanded.contains("#[inline]"));
		assert!(!expanded.contains("#[inline(always)]"));
	}

	#[test]
	fn empty_enum_is_rejected() {
		let input: TokenStream = quote! {
			pub enum Empty {}
		};
		let expanded = expand_with(quote!(), input);

		assert!(expanded.contains("requiresatleastoneinstructionvariant"));
	}

	#[test]
	fn duplicate_variant_attribute_is_rejected() {
		let input: TokenStream = quote! {
			pub enum Mixed {
				#[dispatch(accounts = OneAccounts)]
				#[dispatch(accounts = TwoAccounts)]
				Run = 0,
			}
		};
		let expanded = expand_with(quote!(), input);

		assert!(expanded.contains("duplicate`#[dispatch(...)]`onvariant`Run`"));
	}

	#[test]
	fn unknown_outer_argument_is_rejected() {
		let input: TokenStream = quote! {
			pub enum CounterInstruction {
				Initialize = 0,
			}
		};
		let expanded = expand_with(quote!(disptach = true), input);

		assert!(expanded.contains("couldnotparsethe`#[instruction_dispatch(...)]`input"));
	}

	#[test]
	fn migrations_budget_without_a_ladder_is_rejected() {
		let args = dispatch_args(quote!(migrations_max_lamports = 20_000));
		let error = resolve_migrations(&args, &ident("Instruction")).unwrap_err();

		assert!(
			error
				.to_string()
				.contains("requires `migrations(Account, ...)`")
		);
	}

	#[test]
	fn empty_ladder_is_rejected() {
		let args = dispatch_args(quote!(migrations(), migrations_max_lamports = 20_000));
		let error = resolve_migrations(&args, &ident("Instruction")).unwrap_err();

		assert!(
			error
				.to_string()
				.contains("at least one migratable contract")
		);
	}

	#[test]
	fn ladder_without_a_budget_is_rejected() {
		let args = dispatch_args(quote!(migrations(State)));
		let error = resolve_migrations(&args, &ident("Instruction")).unwrap_err();

		let message = error.to_string();
		assert!(
			message.contains("migrations_max_lamports"),
			"message: {message}"
		);
		assert!(message.contains("program policy"), "message: {message}");
	}

	#[test]
	fn migration_emission_wires_the_declared_slot_order() {
		let crate_path: Path = syn::parse_quote!(::pina);
		let budget: Expr = syn::parse_quote!(BUDGET);
		let ladder = [
			ladder_of("State"),
			ladder_of("ManualState"),
			ladder_of("CompactState"),
			ladder_of("State"),
		];
		let program_id: Expr = syn::parse_quote!(ID);
		let (helper, prelude) = migrate_emission(&crate_path, &ladder, &budget, &program_id);
		let helper = squeezed(&helper.to_string());
		let prelude = squeezed(&prelude.to_string());

		assert!(helper.contains("fnprocess_migrate"));
		assert!(helper.contains("MigrateContext::new(program_id,accounts,BUDGET)"));
		// Slots start at 2: the payer and the system program precede them.
		assert!(helper.contains("run_optional::<State>(2)"));
		assert!(helper.contains("run_optional::<ManualState>(3)"));
		assert!(helper.contains("run_optional::<CompactState>(4)"));
		assert!(helper.contains("run_optional::<State>(5)"));
		assert!(prelude.contains("is_migrate_instruction(data)"));
	}

	#[test]
	fn ladder_resolved_under_this_crate_reports_the_missing_manifest() {
		// The unit-test environment has no checked-in manifest, so the resolved
		// path reports the same remedy a program without one would see.
		let args = dispatch_args(quote!(migrations(State), migrations_max_lamports = 20_000));
		let error = resolve_migrations(&args, &ident("Instruction")).unwrap_err();

		let message = error.to_string();
		assert!(
			message.contains("pina migrations make"),
			"message: {message}"
		);
	}

	#[test]
	fn capacity_flag_round_trips() {
		let enabled = CapacityTestArg::from_word().unwrap_or_else(|error| panic!("word: {error}"));
		assert!(enabled.is_enabled());

		let disabled =
			CapacityTestArg::from_bool(false).unwrap_or_else(|error| panic!("bool: {error}"));
		assert!(!disabled.is_enabled());
	}
}
