//! Entrypoint generation for `#[discriminator(entrypoint)]`.
//!
//! Turns the instruction discriminator enum into the program's entrypoint
//! without introducing a free function that could collide with other items:
//! every generated item is an associated item on the annotated enum, so the
//! entrypoint is always named `Enum::process_instruction`.

use darling::FromMeta;
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
use crate::args::DiscriminatorArgs;
use crate::args::DispatchVariantArgs;
use crate::args::InlineArg;
use crate::args::Primitive;

/// Suffix appended to a variant name to find its accounts struct.
const ACCOUNTS_SUFFIX: &str = "Accounts";

/// Name of the crate-level marker that keeps the entrypoint unique.
///
/// The marker is a `#[macro_export]`-style item in the crate root namespace, so
/// two opt-ins collide even when they live in different modules. A plain
/// module-level `const` would only collide within one module and would let a
/// program declare two entrypoints.
const UNIQUENESS_MARKER: &str = "__pina_entrypoint_must_be_unique_per_program";

/// Account slots the reserved `Migrate` route reads before its migratable
/// accounts: the payer and the system program.
const MIGRATE_LEADING_SLOTS: usize = 2;

/// One resolved variant → accounts-struct route.
struct Route {
	/// The enum variant this route dispatches.
	variant: Ident,
	/// The accounts struct parsed for the variant.
	accounts: Path,
	/// Trailing segment of `accounts`, used in diagnostics and capacity tests.
	accounts_name: Ident,
}

/// Resolve the routed variants, or report the first unusable one.
///
/// Every route is resolved before anything is emitted: a diagnostic naming the
/// offending variant is worth more than a partially expanded program.
fn resolve_routes(item_enum: &ItemEnum) -> syn::Result<Vec<Route>> {
	let mut routes = Vec::with_capacity(item_enum.variants.len());

	for variant in &item_enum.variants {
		let variant_name = variant.ident.clone();
		let mut declared_accounts = None;

		for attribute in &variant.attrs {
			if !attribute.path().is_ident("dispatch") {
				continue;
			}
			if declared_accounts.is_some() {
				return Err(syn::Error::new_spanned(
					attribute,
					format!("duplicate `#[dispatch(...)]` on variant `{variant_name}`"),
				));
			}

			let metas = attribute
				.parse_args_with(Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)?;
			let parsed = DispatchVariantArgs::from_list(
				&metas
					.into_iter()
					.map(darling::ast::NestedMeta::Meta)
					.collect::<Vec<_>>(),
			)
			.map_err(|error| {
				let reason = crate::validation::darling_reason(&error);

				syn::Error::new_spanned(
					attribute,
					format!(
						"could not parse `#[dispatch(...)]` on variant `{variant_name}`: {reason} \
						 The only supported argument is `accounts = AccountsStruct`"
					),
				)
			})?;
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
		return Err(syn::Error::new_spanned(
			&item_enum.ident,
			"an `entrypoint` discriminator requires at least one instruction variant",
		));
	}

	Ok(routes)
}

/// One dispatch arm: parse the variant's accounts and run its process.
///
/// A variant whose instruction the manifest records with a version envelope
/// is routed through that instruction's `process_versioned`, which normalizes
/// a historical payload first. The discriminator comparison is a constant, so
/// only the matching route survives compilation.
fn dispatch_arm(
	crate_path: &Path,
	enum_name: &Ident,
	route: &Route,
	enveloped: &[(u64, Ident)],
) -> proc_macro2::TokenStream {
	let Route {
		variant, accounts, ..
	} = route;
	let versioned = enveloped.iter().map(|(value, instruction)| {
		let value = Literal::u64_unsuffixed(*value);
		quote_spanned! {variant.span()=>
			if #enum_name::#variant as u64 == #value {
				return #instruction::process_versioned(__pina_accounts, data);
			}
		}
	});

	quote_spanned! {variant.span()=>
		#enum_name::#variant => {
			// Trait-qualified so programs with explicit imports compile
			// without relying on `ProcessAccountInfos` being in scope.
			let __pina_accounts = <#accounts as ::core::convert::TryFrom<(
				& #crate_path::Address,
				&mut [#crate_path::AccountView],
			)>>::try_from((program_id, accounts))?;

			#(#versioned)*
			<#accounts as #crate_path::ProcessAccountInfos>::process(__pina_accounts, data)
		}
	}
}

/// Everything the entrypoint expansion emits besides the enum itself.
#[derive(Debug)]
pub(crate) struct EntrypointExpansion {
	/// A crate-root marker keeping the entrypoint unique per program.
	pub(crate) uniqueness_marker: proc_macro2::TokenStream,
	/// The associated items placed inside `impl Enum { ... }`.
	pub(crate) implementation: proc_macro2::TokenStream,
}

/// Build the entrypoint for an `#[discriminator(entrypoint)]` enum.
pub(crate) fn expand(
	args: &DiscriminatorArgs,
	item_enum: &mut ItemEnum,
) -> syn::Result<EntrypointExpansion> {
	let crate_path = &args.crate_path;
	let enum_name = item_enum.ident.clone();
	let program_id = args
		.program_id
		.clone()
		.unwrap_or_else(|| syn::parse_quote!(ID));

	let routes = resolve_routes(item_enum)?;

	// The per-variant attributes belong to this macro's grammar, not to the
	// enum's own API, so they must not survive into the emitted enum.
	for variant in &mut item_enum.variants {
		variant
			.attrs
			.retain(|attribute| !attribute.path().is_ident("dispatch"));
	}

	let migration_ladder = resolve_migrations(args, &enum_name)?;
	let enveloped = crate::migration::manifest_enveloped_instructions(&enum_name)?;

	// The generated arms and bounds both name the accounts type. Building those
	// tokens with the variant's span keeps a missing accounts struct pointing at
	// the variant the caller wrote, instead of at the attribute.
	let dispatch_arms = routes
		.iter()
		.map(|route| dispatch_arm(crate_path, &enum_name, route, &enveloped));

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

	// The entrypoint array must hold every account a route could accept, which
	// is the parser's limit, not its declared bound: a `remaining` slice counts
	// one declared slot but consumes every account left.
	let route_limit = |route: &Route| {
		let accounts = &route.accounts;

		quote_spanned! {accounts.span()=>
			{
				const fn __pina_account_limit<'a, T>() -> usize
				where
					T: #crate_path::ParseAccounts<'a>,
				{
					<T as #crate_path::ParseAccounts<'a>>::ACCOUNT_LIMIT
				}
				__pina_account_limit::<'static, #accounts>()
			}
		}
	};
	let limit_values = routes.iter().map(route_limit).collect::<Vec<_>>();

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
				let limit = route_limit(route);

				quote! {
					const _: () = assert!(
						#bound <= #enum_name::MAX_INSTRUCTION_ACCOUNTS
							|| #bound == ::core::primitive::usize::MAX,
						concat!(
							"MAX_INSTRUCTION_ACCOUNTS must cover `",
							#name,
							"`; the constant is derived from every declared instruction's \
							 ACCOUNT_BOUND, so a smaller value means a route was missed",
						),
					);
					const _: () = assert!(
						#limit < #enum_name::ENTRYPOINT_ACCOUNT_CAPACITY
							|| #enum_name::ENTRYPOINT_ACCOUNT_CAPACITY
								== #crate_path::pinocchio::MAX_TX_ACCOUNTS,
						concat!(
							"ENTRYPOINT_ACCOUNT_CAPACITY must leave a spare slot past every account `",
							#name,
							"` can accept, or `finish_exact` cannot see an extra trailing account",
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
						#enum_name::MAX_INSTRUCTION_ACCOUNTS
							<= #crate_path::pinocchio::MAX_TX_ACCOUNTS,
						"MAX_INSTRUCTION_ACCOUNTS must not exceed the entrypoint's account array",
					);
					const _: () = assert!(
						#enum_name::ENTRYPOINT_ACCOUNT_CAPACITY
							<= #crate_path::pinocchio::MAX_TX_ACCOUNTS,
						"ENTRYPOINT_ACCOUNT_CAPACITY must not exceed the transaction's account limit",
					);

					#(#assertions)*
				};
			}
		});

	let migrate_helper = migration_ladder.as_ref().map(|route| &route.helper);
	let migrate_prelude = migration_ladder.as_ref().map(|route| &route.prelude);
	let migrate_slots = migration_ladder.as_ref().map(|route| route.account_slots);
	let capacity_values = capacity_values(&limit_values, migrate_slots);
	let capacity_count = Literal::usize_unsuffixed(capacity_values.len());

	// `macro_export` lifts the name into the crate root regardless of the module
	// the enum lives in, so a second opt-in anywhere in the crate collides on it.
	let entrypoint_docs = format!(
		"Dispatches one instruction to its accounts struct.\n\nPass this to `nostd_entrypoint!` \
		 as `nostd_entrypoint!({enum_name}::process_instruction, \
		 {enum_name}::ENTRYPOINT_ACCOUNT_CAPACITY)`. Program-specific behavior beyond routing \
		 belongs in each accounts struct's `ProcessAccountInfos::process`."
	);

	let uniqueness_marker = {
		let marker = Ident::new(UNIQUENESS_MARKER, enum_name.span());

		quote! {
			#[doc(hidden)]
			#[macro_export]
			macro_rules! #marker {
				() => {};
			}
		}
	};

	let implementation = quote! {
		impl #enum_name {
			/// Upper bound on the accounts this program reads in one instruction.
			///
			/// Derived from every instruction's declared `ACCOUNT_BOUND` and
			/// saturated at the entrypoint's account array, so a variant that
			/// declares an unbounded trailing slice cannot inflate the cap.
			///
			/// This is the count a program declares, not a security boundary. Passing
			/// it to `nostd_entrypoint!` would size the runtime's account array to
			/// exactly the widest instruction, and the loader *skips* any account
			/// beyond that array instead of failing, so `finish_exact` would no longer
			/// see — or reject — an extra trailing account. Pass
			/// [`Self::ENTRYPOINT_ACCOUNT_CAPACITY`] to bound the array safely.
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

			/// The account-array size to pass to `nostd_entrypoint!` as its second
			/// argument: one slot more than any route can accept, or the transaction
			/// maximum when a route accepts any number of accounts.
			///
			/// Each route contributes its accounts struct's `ACCOUNT_LIMIT`, not its
			/// declared `ACCOUNT_BOUND`: a `#[pina(remaining)]` slice, directly or in
			/// a nested account group, and a hand-written parser that declares no
			/// limit all make it the transaction maximum, so such a route sees every
			/// account it is sent.
			///
			/// Pinocchio's deserializer walks accounts five at a time, so an array
			/// of five or fewer slots drops that loop and shrinks the deployed
			/// program. A larger bounded array keeps the loop and adds one that
			/// skips accounts past the array, so it grows the program; measure
			/// before passing a capacity above five.
			///
			/// The loader skips accounts beyond the entrypoint's array rather than
			/// rejecting them, and the spare slot is what keeps a smaller array
			/// safe: every instruction whose accounts struct ends with
			/// `finish_exact` still sees the first extra account and rejects it
			/// with `TooManyAccountKeys`, exactly as it would with the full array.
			/// Accounts past the spare slot are never
			/// materialized, so the one observable difference is precedence: a
			/// writable account whose duplicate sits past the spare slot fails with
			/// `TooManyAccountKeys` instead of `DuplicateMutableAccount`.
			///
			/// A program with a hand-written router, or one that reads accounts
			/// outside its routed accounts structs, must size its array itself.
			pub const ENTRYPOINT_ACCOUNT_CAPACITY: usize = {
				const fn maximum(values: [usize; #capacity_count]) -> usize {
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

				let highest = maximum([#(#capacity_values),*]);
				if highest >= #crate_path::pinocchio::MAX_TX_ACCOUNTS {
					#crate_path::pinocchio::MAX_TX_ACCOUNTS
				} else {
					highest + 1
				}
			};

			#[doc = #entrypoint_docs]
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

			#migrate_helper
		}

		#capacity_test
	};

	Ok(EntrypointExpansion {
		uniqueness_marker,
		implementation,
	})
}

/// The account counts `ENTRYPOINT_ACCOUNT_CAPACITY` must exceed: every
/// route's `ACCOUNT_LIMIT`, then the reserved `Migrate` route's slots when
/// the program has one.
///
/// The reserved route reads its slots straight from the account slice, treats
/// a missing slot as omitted, and fails on an account past its last slot, so its
/// slot count is a limit like any route's.
fn capacity_values(
	limit_values: &[proc_macro2::TokenStream],
	migrate_slots: Option<usize>,
) -> Vec<proc_macro2::TokenStream> {
	limit_values
		.iter()
		.cloned()
		.chain(migrate_slots.map(|slots| {
			let slots = Literal::usize_unsuffixed(slots);
			quote!(#slots)
		}))
		.collect()
}

/// The reserved `Migrate` route of a program with migratable accounts.
struct MigrateRoute {
	/// The `process_migrate` associated function.
	helper: proc_macro2::TokenStream,
	/// The width-matched `is_migrate_instruction` guard run before dispatch.
	prelude: proc_macro2::TokenStream,
	/// Account slots the route reads: the leading slots plus one per ladder
	/// entry.
	account_slots: usize,
}

/// Resolve the optional reserved-`Migrate` routing.
///
/// The ladder is derived from the checked-in manifest: every enveloped account
/// contract becomes an optional reserved-instruction slot, in the manifest's
/// identity-sorted order — the same order generated clients compose. Returns
/// `None` when the program has no manifest or no migratable accounts.
fn resolve_migrations(
	args: &DiscriminatorArgs,
	enum_name: &Ident,
) -> syn::Result<Option<MigrateRoute>> {
	let crate_path = &args.crate_path;
	let declared_ladder = &args.migrations;
	// The budget is optional: with no declared ceiling the reserved route
	// enforces none, because a transfer is already bounded by the rent deficit
	// of a growth the runtime caps. Declaring one only tightens that.
	let max_lamports = &args.migrations_max_lamports;

	// An explicit list overrides the derived ladder: it is the only way to
	// expose several accounts of the same contract in one sweep, because the
	// manifest records contracts, not account instances. Without a list, the
	// ladder is derived from the manifest — one slot per enveloped account
	// contract, in the identity-sorted order generated clients compose.
	let ladder: Vec<proc_macro2::TokenStream> = match declared_ladder {
		Some(ladder) => {
			if ladder.is_empty() {
				return Err(syn::Error::new_spanned(
					enum_name,
					"`migrations(...)` must name at least one migratable contract",
				));
			}
			// A named contract without a checked-in history would fail later as
			// an unsatisfied trait bound; report the remedy here instead.
			crate::migration::verify_migration_contracts(enum_name, ladder)?;
			ladder
				.iter()
				.map(::quote::ToTokens::to_token_stream)
				.collect()
		}
		None => {
			crate::migration::manifest_account_ladder(enum_name)?
				.into_iter()
				.map(|ident| ::quote::ToTokens::to_token_stream(&ident))
				.collect()
		}
	};

	if ladder.is_empty() {
		return Ok(None);
	}

	Ok(Some(migrate_emission(
		crate_path,
		args.primitive,
		&ladder,
		max_lamports.as_ref(),
		&args
			.program_id
			.clone()
			.unwrap_or_else(|| syn::parse_quote!(ID)),
	)))
}

/// Emit the reserved-`Migrate` helper and the dispatch prelude for a validated
/// ladder, and count the account slots the route reads.
///
/// `max_lamports` tightens the route when present; `None` emits a call with no
/// declared ceiling.
///
/// Split from [`resolve_migrations`] so the emitted shape is unit-testable
/// without a manifest on disk.
fn migrate_emission(
	crate_path: &Path,
	primitive: Primitive,
	ladder: &[proc_macro2::TokenStream],
	max_lamports: Option<&Expr>,
	program_id: &Expr,
) -> MigrateRoute {
	// `None` states the absence of a declared ceiling rather than inventing
	// one; the executor then enforces none.
	let budget = max_lamports.map_or_else(
		|| quote!(::core::option::Option::None),
		|expr| quote!(::core::option::Option::Some(#expr)),
	);
	let steps = ladder.iter().enumerate().map(|(position, account)| {
		let index = Literal::usize_unsuffixed(position + MIGRATE_LEADING_SLOTS);

		quote! {
			migrate.run_optional::<#account>(#index)?;
		}
	});

	// The reserved value is the all-ones value of the instruction
	// discriminator's own width, so the guard must test the same width. A
	// one-byte check against a `u16` enum's `0xffff` never matches, which would
	// make the reserved path unreachable.
	let is_migrate_instruction = match primitive {
		Primitive::U8 => quote!(is_migrate_instruction),
		Primitive::U16 => quote!(is_migrate_instruction_u16),
		Primitive::U32 => quote!(is_migrate_instruction_u32),
		Primitive::U64 => quote!(is_migrate_instruction_u64),
	};

	let slots = Literal::usize_unsuffixed(MIGRATE_LEADING_SLOTS + ladder.len());

	let helper = quote! {
		/// Routes the reserved framework `Migrate` instruction.
		///
		/// Slots are `[payer, systemProgram, ...migratable]`, in the order declared by
		/// `migrations(...)`. A slot holding the program address, or an index past the
		/// end of the slice, is treated as omitted, so a client sends only the accounts
		/// it needs and every slot draws from one shared lamport budget. An account
		/// past the last slot is never read or validated, and it fails the
		/// instruction, so the slot count is the most accounts the route accepts and
		/// `ENTRYPOINT_ACCOUNT_CAPACITY` can bound the array by it.
		///
		/// # Errors
		///
		/// Returns `IncorrectProgramId` when `program_id` is not this program and
		/// `TooManyAccountKeys` when an account follows the last slot, then inherits
		/// `MigrateContext`'s layout validation and each slot's migration failures.
		pub fn process_migrate(
			program_id: & #crate_path::Address,
			accounts: &mut [#crate_path::AccountView],
		) -> #crate_path::ProgramResult {
			// The route calls the `account-resize` executor, so assert the
			// feature is on. The constant is named after the remedy, which
			// makes an unresolved symbol say what to enable instead of
			// reporting a bare `MigrateContext` lookup failure.
			const _: () = #crate_path::ACCOUNT_RESIZE_FEATURE_REQUIRED_FOR_MIGRATE_ROUTE;

			// The reserved path bypasses `parse_instruction`, so the configured
			// id is checked here: a mismatched program id must fail before any
			// account is migrated, exactly as ordinary instructions reject it.
			if program_id != &#program_id {
				return Err(#crate_path::ProgramError::IncorrectProgramId);
			}
			// The steps only see the declared slots. An account past them fails the
			// instruction after the steps, which the runtime rolls back; returning
			// before them measured up to 1.6 KB larger, because LLVM restructures
			// the inlined steps around that early exit.
			let accounts_len = accounts.len();
			let accounts = &mut accounts[..accounts_len.min(#slots)];

			let mut migrate = #crate_path::MigrateContext::new(program_id, accounts, #budget)?;
			#(#steps)*

			if accounts_len > #slots {
				return Err(#crate_path::PinaProgramError::TooManyAccountKeys.into());
			}

			Ok(())
		}
	};

	// The reserved instruction is detected before `parse_instruction`, which
	// would reject it: `#[discriminator]` reserves the all-ones value. The
	// guard matches the enum's own width, so a wider program's `0xffff` (or
	// `0xffff_ffff`, and so on) is recognized rather than only `0xff`.
	let prelude = quote! {
		if #crate_path::#is_migrate_instruction(data) {
			return Self::process_migrate(program_id, accounts);
		}
	};

	MigrateRoute {
		helper,
		prelude,
		account_slots: MIGRATE_LEADING_SLOTS + ladder.len(),
	}
}

#[cfg(test)]
mod tests {
	use darling::ast::NestedMeta;
	use proc_macro2::TokenStream;
	use quote::ToTokens;

	use super::*;
	use crate::args::DiscriminatorArgs;

	fn args(tokens: TokenStream) -> DiscriminatorArgs {
		let nested = NestedMeta::parse_meta_list(tokens)
			.unwrap_or_else(|error| panic!("test args: {error}"));
		DiscriminatorArgs::from_list(&nested)
			.unwrap_or_else(|error| panic!("test discriminator args: {error}"))
	}

	/// Collapse whitespace so assertions match a token stream's normalized
	/// spacing rather than its pretty-printed form.
	fn squeezed(tokens: &str) -> String {
		tokens.split_whitespace().collect()
	}

	fn enum_of(tokens: TokenStream) -> ItemEnum {
		syn::parse2(tokens).unwrap_or_else(|error| panic!("test enum: {error}"))
	}

	fn expand_with(args_tokens: TokenStream, enum_tokens: TokenStream) -> String {
		let args = args(args_tokens);
		let mut item_enum = enum_of(enum_tokens);
		let expansion = expand(&args, &mut item_enum)
			.unwrap_or_else(|error| panic!("entrypoint expansion: {error}"));

		squeezed(&expansion.implementation.to_string())
	}

	#[test]
	fn entrypoint_is_an_associated_item_on_the_enum() {
		let expanded = expand_with(
			quote!(entrypoint),
			quote! {
				#[discriminator]
				pub enum CounterInstruction {
					Initialize = 0,
					Increment = 1,
				}
			},
		);

		// Everything is generated inside one inherent impl.
		assert!(expanded.contains("implCounterInstruction{"));
		assert!(expanded.contains("pubconstMAX_INSTRUCTION_ACCOUNTS:usize"));
		assert!(expanded.contains("pubfnprocess_instruction("));
		// No free function is emitted, so nothing can collide with the caller's items.
		assert!(!expanded.contains("}pubfnprocess_instruction"));
		assert!(expanded.contains("CounterInstruction::Initialize"));
		assert!(expanded.contains("InitializeAccounts"));
		assert!(expanded.contains("IncrementAccounts"));
		assert!(expanded.contains("#[inline(always)]"));
	}

	/// A variant whose instruction the manifest records with an envelope is
	/// normalized through `process_versioned`; every other route is unchanged.
	#[test]
	fn enveloped_instructions_dispatch_through_their_versioned_process() {
		let route = Route {
			variant: syn::parse_quote!(Update),
			accounts: syn::parse_quote!(UpdateAccounts),
			accounts_name: syn::parse_quote!(UpdateAccounts),
		};
		let crate_path: Path = syn::parse_quote!(::pina);
		let enum_name: Ident = syn::parse_quote!(MigrationInstruction);
		let enveloped = [(0, syn::parse_quote!(UpdateInstruction))];

		let routed =
			squeezed(&dispatch_arm(&crate_path, &enum_name, &route, &enveloped).to_string());
		assert!(
			routed.contains("ifMigrationInstruction::Updateasu64==0{"),
			"routed: {routed}"
		);
		assert!(
			routed.contains("returnUpdateInstruction::process_versioned(__pina_accounts,data);"),
			"routed: {routed}"
		);
		// The ordinary process remains the fallthrough for other variants.
		assert!(routed.contains("ProcessAccountInfos>::process(__pina_accounts,data)"));

		let plain = squeezed(&dispatch_arm(&crate_path, &enum_name, &route, &[]).to_string());
		assert!(!plain.contains("process_versioned"), "plain: {plain}");
	}

	#[test]
	fn dispatch_override_routes_the_named_struct() {
		let expanded = expand_with(
			quote!(entrypoint),
			quote! {
				pub enum Mixed {
					Default = 0,
					#[dispatch(accounts = CustomAccounts)]
					Overridden = 1,
				}
			},
		);

		assert!(expanded.contains("DefaultAccounts"));
		assert!(expanded.contains("CustomAccounts"));
	}

	#[test]
	fn the_enum_keeps_its_dispatch_attributes_stripped() {
		let args = args(quote!(entrypoint));
		let mut item_enum = enum_of(quote! {
			pub enum Mixed {
				#[dispatch(accounts = CustomAccounts)]
				Run = 0,
			}
		});
		expand(&args, &mut item_enum).unwrap_or_else(|error| panic!("expansion: {error}"));

		let rendered = item_enum.to_token_stream().to_string();

		assert!(!rendered.contains("#[dispatch("), "rendered: {rendered}");
	}

	#[test]
	fn capacity_test_can_be_suppressed() {
		let expanded = expand_with(
			quote!(entrypoint, capacity_test = false),
			quote! {
				pub enum CounterInstruction {
					Initialize = 0,
				}
			},
		);

		assert!(expanded.contains("pubconstMAX_INSTRUCTION_ACCOUNTS:usize"));
		assert!(!expanded.contains("#[cfg(test)]"));
	}

	#[test]
	fn inline_hint_is_honoured() {
		let expanded = expand_with(
			quote!(entrypoint, inline = "hint"),
			quote! {
				pub enum CounterInstruction {
					Initialize = 0,
				}
			},
		);

		assert!(expanded.contains("#[inline]"));
		assert!(!expanded.contains("#[inline(always)]"));
	}

	#[test]
	fn empty_enum_is_rejected() {
		let args = args(quote!(entrypoint));
		let mut item_enum = enum_of(quote!(
			pub enum Empty {}
		));
		let error = expand(&args, &mut item_enum).unwrap_err();

		assert!(
			error
				.to_string()
				.contains("at least one instruction variant")
		);
	}

	#[test]
	fn duplicate_variant_attribute_is_rejected() {
		let args = args(quote!(entrypoint));
		let mut item_enum = enum_of(quote! {
			pub enum Mixed {
				#[dispatch(accounts = OneAccounts)]
				#[dispatch(accounts = TwoAccounts)]
				Run = 0,
			}
		});
		let error = expand(&args, &mut item_enum).unwrap_err();

		assert!(
			error
				.to_string()
				.contains("duplicate `#[dispatch(...)]` on variant `Run`")
		);
	}

	#[test]
	fn budget_without_explicit_ladder_compiles_without_routing() {
		// No manifest in the unit-test environment, so the derived ladder is
		// empty and the migration endpoint is simply not generated.
		let args = args(quote!(entrypoint, migrations_max_lamports = 20_000));
		let mut item_enum = enum_of(quote!(
			pub enum Instruction {
				Run = 0,
			}
		));
		let expanded = expand(&args, &mut item_enum).unwrap();
		let implementation = squeezed(&expanded.implementation.to_string());

		assert!(!implementation.contains("process_migrate"));
		assert!(!implementation.contains("is_migrate_instruction"));
	}

	#[test]
	fn empty_ladder_is_rejected() {
		let args = args(quote!(
			entrypoint,
			migrations(),
			migrations_max_lamports = 20_000
		));
		let mut item_enum = enum_of(quote!(
			pub enum Instruction {
				Run = 0,
			}
		));
		let error = expand(&args, &mut item_enum).unwrap_err();

		assert!(
			error
				.to_string()
				.contains("at least one migratable contract")
		);
	}

	#[test]
	fn an_explicit_ladder_without_a_budget_still_requires_a_manifest() {
		// Dropping the budget made it optional, not the history: naming
		// contracts still demands a checked-in snapshot. The unit-test
		// environment sits in a crate with no manifest anywhere above it, so
		// `discover_program_dir` finds nothing and the ladder reports the
		// unlocatable-manifest error rather than the missing-file remedy a
		// real program directory would produce.
		let args = args(quote!(entrypoint, migrations(State)));
		let mut item_enum = enum_of(quote!(
			pub enum Instruction {
				Run = 0,
			}
		));
		let error = expand(&args, &mut item_enum).unwrap_err();

		assert!(
			error
				.to_string()
				.contains("could not locate Cargo manifest for migration-aware dispatch"),
			"unexpected message: {error}"
		);
	}

	#[test]
	fn migration_emission_wires_the_declared_slot_order() {
		let crate_path: Path = syn::parse_quote!(::pina);
		let budget: Expr = syn::parse_quote!(BUDGET);
		let program_id: Expr = syn::parse_quote!(ID);
		let ladder = [
			ladder_of("State"),
			ladder_of("ManualState"),
			ladder_of("CompactState"),
			ladder_of("State"),
		]
		.map(|path| ::quote::ToTokens::to_token_stream(&path));
		let MigrateRoute {
			helper,
			prelude,
			account_slots,
		} = migrate_emission(
			&crate_path,
			Primitive::U8,
			&ladder,
			Some(&budget),
			&program_id,
		);
		// The payer and system program lead the four migratable slots.
		assert_eq!(account_slots, 6);
		let helper = squeezed(&helper.to_string());
		let prelude = squeezed(&prelude.to_string());

		assert!(helper.contains("pubfnprocess_migrate("));
		// The route depends on the `account-resize` executor, and the constant
		// it references is named after the feature so a missing one is
		// self-describing.
		assert!(
			helper.contains("ACCOUNT_RESIZE_FEATURE_REQUIRED_FOR_MIGRATE_ROUTE"),
			"the route must declare its `account-resize` dependency: {helper}"
		);
		// A declared budget is passed through as `Some`.
		assert!(helper.contains(
			"MigrateContext::new(program_id,accounts,::core::option::Option::Some(BUDGET))"
		));
		// Slots start at 2: the payer and the system program precede them.
		assert!(helper.contains("run_optional::<State>(2)"));
		assert!(helper.contains("run_optional::<ManualState>(3)"));
		assert!(helper.contains("run_optional::<CompactState>(4)"));
		assert!(helper.contains("run_optional::<State>(5)"));
		// An account past the last slot is rejected, so the slot count bounds
		// the entrypoint array.
		assert!(
			helper.contains("letaccounts=&mutaccounts[..accounts_len.min(6)];"),
			"helper: {helper}"
		);
		assert!(helper.contains("ifaccounts_len>6{"), "helper: {helper}");
		assert!(helper.contains("PinaProgramError::TooManyAccountKeys"));
		// The reserved path validates the program id before migrating anything.
		assert!(helper.contains("ProgramError::IncorrectProgramId"));
		// The prelude routes through the associated helper.
		assert!(prelude.contains("is_migrate_instruction(data)"));
		assert!(prelude.contains("Self::process_migrate(program_id,accounts)"));
	}

	#[test]
	fn migration_emission_matches_the_guard_to_the_discriminator_width() {
		// The reserved value is the all-ones value of the enum's own width, so a
		// one-byte guard on a `u16` program never matches and the reserved path
		// is unreachable. Each width must select the helper that tests it.
		let crate_path: Path = syn::parse_quote!(::pina);
		let budget: Expr = syn::parse_quote!(BUDGET);
		let program_id: Expr = syn::parse_quote!(ID);
		let ladder = [ladder_of("State")].map(|path| ::quote::ToTokens::to_token_stream(&path));

		for (primitive, expected) in [
			(Primitive::U8, "::pina::is_migrate_instruction(data)"),
			(Primitive::U16, "::pina::is_migrate_instruction_u16(data)"),
			(Primitive::U32, "::pina::is_migrate_instruction_u32(data)"),
			(Primitive::U64, "::pina::is_migrate_instruction_u64(data)"),
		] {
			let MigrateRoute { prelude, .. } =
				migrate_emission(&crate_path, primitive, &ladder, Some(&budget), &program_id);
			let prelude = squeezed(&prelude.to_string());

			assert!(
				prelude.contains(expected),
				"`{primitive:?}` must guard with `{expected}`; got: {prelude}"
			);
			// Exact spelling only: the wider helper names contain the one-byte
			// name as a prefix, so a substring check would pass on the wrong one.
			if !matches!(primitive, Primitive::U8) {
				assert!(
					!prelude.contains("is_migrate_instruction("),
					"`{primitive:?}` must use its own width's helper: {prelude}"
				);
			}
		}
	}

	#[test]
	fn migration_emission_without_a_budget_declares_no_ceiling() {
		let crate_path: Path = syn::parse_quote!(::pina);
		let program_id: Expr = syn::parse_quote!(ID);
		let ladder = [ladder_of("State")].map(|path| ::quote::ToTokens::to_token_stream(&path));
		let MigrateRoute { helper, .. } =
			migrate_emission(&crate_path, Primitive::U8, &ladder, None, &program_id);
		let helper = squeezed(&helper.to_string());

		// No ceiling is stated as `None` rather than a sentinel maximum, and the
		// ladder still routes.
		assert!(
			helper
				.contains("MigrateContext::new(program_id,accounts,::core::option::Option::None)"),
			"helper: {helper}"
		);
		assert!(helper.contains("run_optional::<State>(2)"));
	}

	#[test]
	fn entrypoint_documentation_names_the_enum() {
		// `Self::` is not valid where `nostd_entrypoint!` is invoked at module
		// scope, so the generated guidance must spell out the enum.
		let expanded = expand_with(
			quote!(entrypoint),
			quote!(
				pub enum CounterInstruction {
					Run = 0,
				}
			),
		);

		assert!(
			expanded.contains(
				"nostd_entrypoint!(CounterInstruction::process_instruction,\
				 CounterInstruction::ENTRYPOINT_ACCOUNT_CAPACITY)"
			),
			"the doc must name the enum; got: {expanded}"
		);
		assert!(!expanded.contains("nostd_entrypoint!(Self::process_instruction"));
	}

	#[test]
	fn entrypoint_account_capacity_is_generated_with_its_spare_slot_guard() {
		let expanded = expand_with(
			quote!(entrypoint),
			quote!(
				pub enum CounterInstruction {
					Run = 0,
				}
			),
		);

		assert!(
			expanded.contains("pubconstENTRYPOINT_ACCOUNT_CAPACITY:usize"),
			"expanded: {expanded}"
		);
		assert!(
			expanded.contains("ENTRYPOINT_ACCOUNT_CAPACITYmustleaveaspareslotpast"),
			"the capacity test must guard the spare slot; got: {expanded}"
		);
		assert!(
			expanded.contains("highest+1"),
			"the capacity must add the spare slot; got: {expanded}"
		);
	}

	/// The capacity folds each route's limit, not its declared bound, which
	/// counts a `remaining` slice as a single slot.
	#[test]
	fn entrypoint_account_capacity_folds_route_limits() {
		let expanded = expand_with(
			quote!(entrypoint),
			quote!(
				pub enum CounterInstruction {
					Run = 0,
				}
			),
		);
		let capacity = expanded
			.split("pubconstENTRYPOINT_ACCOUNT_CAPACITY:usize")
			.nth(1)
			.and_then(|rest| rest.split("pubfnprocess_instruction").next())
			.expect("the capacity must be generated");

		assert!(capacity.contains("ACCOUNT_LIMIT"), "capacity: {capacity}");
		assert!(!capacity.contains("ACCOUNT_BOUND"), "capacity: {capacity}");
	}

	#[test]
	fn capacity_values_append_the_migrate_route_slots() {
		let bounds = vec![quote!(3), quote!(2)];

		let without = capacity_values(&bounds, None)
			.iter()
			.map(ToString::to_string)
			.collect::<Vec<_>>();
		assert_eq!(without, ["3", "2"]);

		let with = capacity_values(&bounds, Some(MIGRATE_LEADING_SLOTS + 4))
			.iter()
			.map(ToString::to_string)
			.collect::<Vec<_>>();
		assert_eq!(with, ["3", "2", "6"]);
	}

	#[test]
	fn ladder_resolved_under_this_crate_reports_the_missing_manifest() {
		// The unit-test environment has no manifest in this crate or any
		// ancestor, so `discover_program_dir` finds nothing and the ladder
		// reports the unlocatable-manifest error — the same failure a program
		// whose manifest was deleted entirely would see.
		let args = args(quote!(
			entrypoint,
			migrations(State),
			migrations_max_lamports = 20_000
		));
		let mut item_enum = enum_of(quote!(
			pub enum Instruction {
				Run = 0,
			}
		));
		let error = expand(&args, &mut item_enum).unwrap_err();

		let message = error.to_string();
		assert!(
			message.contains("could not locate Cargo manifest for migration-aware dispatch"),
			"message: {message}"
		);
	}

	#[test]
	fn the_uniqueness_marker_lifts_to_the_crate_root() {
		// `macro_export` puts the name in the crate root namespace, so two
		// opt-ins collide even when they live in different modules.
		let args = args(quote!(entrypoint));
		let mut item_enum = enum_of(quote!(
			pub enum Instruction {
				Run = 0,
			}
		));
		let expansion =
			expand(&args, &mut item_enum).unwrap_or_else(|error| panic!("expansion: {error}"));
		let marker = squeezed(&expansion.uniqueness_marker.to_string());

		assert!(marker.contains("#[macro_export]"), "marker: {marker}");
		assert!(marker.contains("__pina_entrypoint_must_be_unique_per_program"));
	}

	#[test]
	fn capacity_flag_round_trips() {
		let enabled = CapacityTestArg::from_word().unwrap_or_else(|error| panic!("word: {error}"));
		assert!(enabled.is_enabled());

		let disabled =
			CapacityTestArg::from_bool(false).unwrap_or_else(|error| panic!("bool: {error}"));
		assert!(!disabled.is_enabled());
	}

	fn ladder_of(name: &str) -> Path {
		syn::parse_str(name).unwrap_or_else(|error| panic!("test ladder path: {error}"))
	}
}
