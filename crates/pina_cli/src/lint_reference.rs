//! The lint reference printed by `pina lint --explain`.
//!
//! The same reference ships as a docs page, and both are generated from this
//! table so the CLI and the website cannot disagree about a lint's contract.
//!
//! Every entry names the lint's contract, why violating it is a vulnerability,
//! and the sanctioned way to bless an intentional exception. The blessing
//! guidance matters as much as the detection rules: a team that cannot bless a
//! deliberate pattern either disables the lint for the whole crate or leaves
//! `#[allow]` attributes behind with no explanation, and both lose the
//! signal the lint exists to provide.

/// One lint's reference entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LintExplanation {
	/// The lint's registered name.
	pub name: &'static str,

	/// The lint's default level.
	pub default_level: &'static str,

	/// What the lint requires.
	pub contract: &'static str,

	/// Why violating the contract is a vulnerability.
	pub rationale: &'static str,

	/// How to bless an intentional exception.
	pub blessing: &'static str,
}

/// Every lint in the catalog, in catalog order.
///
/// The names and levels are asserted against `lints.json` by a test, so a new
/// lint cannot ship without a reference entry.
pub const LINT_REFERENCE: &[LintExplanation] = &[
	LintExplanation {
		name: "deny_account_borrows_across_cpi",
		default_level: "deny",
		contract: "Drop every mutable account-data borrow before invoking another program.",
		rationale: "The invoked program may need the same account. Holding a `RefMut` across the \
		            CPI makes the invocation fail at runtime and hides the re-entrancy boundary \
		            the borrow was documenting.",
		blessing: "Call `drop(guard)` before the CPI, or narrow the borrow so it ends before the \
		           invocation. Prefer restructuring over an `#[allow]`: the attribute silences \
		           the check without ending the borrow, so the runtime failure returns.",
	},
	LintExplanation {
		name: "deny_colliding_account_discriminators",
		default_level: "deny",
		contract: "Every account type's `HasDiscriminator::VALUE` carries a numeric value no \
		           other account type in the program also claims at the same discriminator width.",
		rationale: "Pina discriminators are author-chosen integers, and rustc only rejects \
		            duplicates within one enum. Two account types behind different enums that \
		            agree on the value and the serialized width pass every typed loader check — \
		            owner, discriminator, exact size — so either account deserializes as the \
		            other: the sealevel-attacks type-cosplay class.",
		blessing: "There is no sound exception; two account types at one value is a latent \
		           vulnerability. Give the colliding variant a fresh value (wire values are part \
		           of the ABI, so ship it as a migration), or consolidate all accounts behind one \
		           discriminator enum, where rustc makes the collision impossible.",
	},
	LintExplanation {
		name: "deny_heap_allocations_in_onchain_instruction_handlers",
		default_level: "warn",
		contract: "Avoid heap allocation in instruction handlers.",
		rationale: "On-chain code is charged for every allocated byte and for the code that \
		            manages it. Borrowed slices, stack buffers, and fixed-size POD types keep \
		            both compute units and deployed size down.",
		blessing: "Warns by default and is heuristic: it matches allocation-prone method names in \
		           handler-like functions, so a non-allocating `Clone` implementation or a \
		           foreign API can trip it. Scope `#[allow]` to the individual item and say which \
		           call is known not to allocate.",
	},
	LintExplanation {
		name: "deny_unchecked_remaining_mut",
		default_level: "deny",
		contract: "Reach remaining accounts through `remaining_mut_distinct()` or the derive \
		           attribute rather than `AccountsCursor::remaining_mut()`.",
		rationale: "`remaining_mut()` validates writability but preserves duplicate addresses. \
		            Mutating through two aliases to one account applies a single logical update \
		            twice, which is the duplicate-mutable-account vulnerability class.",
		blessing: "An instruction that genuinely must accept a repeated address — a \
		           self-transfer, for example — should use `#[pina(remaining, distinct = \
		           false)]`, which documents the exception at the field that takes it. That is \
		           preferred over a raw `remaining_mut()` call, which the lint cannot distinguish \
		           from an oversight.",
	},
	LintExplanation {
		name: "deny_unused_account_borrow_guards",
		default_level: "warn",
		contract: "Read or discard an account borrow guard immediately instead of binding it to a \
		           local.",
		rationale: "The guard keeps the account-data borrow alive until the end of the enclosing \
		            scope. Binding and never reading it holds the borrow open for nothing and \
		            turns a later borrow of the same data into a runtime panic.",
		blessing: "Bind the guard as `_guard` if the borrow must outlive the statement, or call \
		           `drop(guard)` where it should end. Both state the intent; an `#[allow]` does \
		           not, and the borrow outlives the attribute either way.",
	},
	LintExplanation {
		name: "require_bounded_remaining_accounts",
		default_level: "deny",
		contract: "Bound every loop over remaining accounts with a constant `.take(MAX)` or a \
		           dominating constant-bound length check.",
		rationale: "The caller controls how many accounts arrive. Linear per-account work over an \
		            unbounded count turns into compute exhaustion, which is a denial of service \
		            against the instruction.",
		blessing: "Define the maximum as a `const MAX: usize` and apply it with `.take(MAX)`, or \
		           reject oversized input first with a constant-bound length check. A \
		           caller-derived bound does not satisfy the contract: the lint requires a \
		           constant so the worst-case cost is auditable from the source.",
	},
	LintExplanation {
		name: "require_canonical_bump_before_pda_write",
		default_level: "deny",
		contract: "Prove a PDA bump is canonical with `assert_canonical_bump()` before accepting \
		           a PDA through `assert_seeds_with_bump()`. `assert_stored_bump()` is the \
		           generated counterpart of `assert_seeds()`: it reuses a bump the handler parsed \
		           from the same account, and this lint requires that provenance.",
		rationale: "A program-derived address has one canonical bump. Accepting any valid bump \
		            lets one seed namespace resolve to several addresses, breaking the uniqueness \
		            the seeds were chosen to provide. `assert_stored_bump()` names the one \
		            legitimate source for an explicit bump — the account's own stored field, read \
		            in this instruction — so the provenance is checked rather than assumed.",
		blessing: "`CreateProgramAccount` and `CreateProgramAccountWithBump` validate \
		           canonicality internally and need no assertion. Where several addresses per \
		           namespace are genuinely intended, use `CreateProgramAccountWithUncheckedBump`, \
		           which names the decision. `assert_stored_bump()` passes only when its bump \
		           argument resolves to a parse of the same account; a bump from instruction data \
		           or a different account fails. Reach for `#[allow]` only on a validation-only \
		           path that accepts non-canonical bumps by design, and name that invariant in \
		           the comment.",
	},
	LintExplanation {
		name: "require_canonical_instruction_dispatch_for_idl",
		default_level: "warn",
		contract: "Match directly on the parsed instruction enum in the entrypoint.",
		rationale: "IDL extraction starts from the entrypoint. An explicit `match` over the \
		            instruction enum is what lets the extractor resolve every accounts struct, so \
		            hidden dispatch means a program whose IDL is incomplete or wrong.",
		blessing: "Restructure the dispatch. If the indirection is required, scope an `#[allow]` \
		           to the entrypoint function and note which construct the extractor cannot \
		           follow.",
	},
	LintExplanation {
		name: "require_checked_asset_arithmetic",
		default_level: "deny",
		contract: "Use checked arithmetic for values that carry an economic quantity: balances, \
		           amounts, prices, rewards, stakes, supply, or lamports.",
		rationale: "Silent overflow, underflow, or saturation corrupts an economic invariant \
		            without failing. The corruption is often permanent and can be worth real \
		            tokens, so the arithmetic must fail loudly.",
		blessing: "Use the checked operation and map the error into the program's error enum. An \
		           `#[allow]` is appropriate only where the invariant is proven by the \
		           surrounding code, and the comment should state the bound that makes it safe.",
	},
	LintExplanation {
		name: "require_consistent_token_program",
		default_level: "deny",
		contract: "Use one token-program identity for parsing, ATA derivation, and dynamic token \
		           CPI within one instruction.",
		rationale: "Mixing identities can validate an account under one token program and then \
		            invoke another. The ownership and address assumptions that justified the \
		            validation no longer hold for the program that acts.",
		blessing: "Decide the token program once and thread that single value through: read it \
		           from the `TokenAccountRef` or `TokenMintRef` you already resolved, whose \
		           `from_account_view` validated that the account belongs to `ID` or \
		           `crate::token_2022::ID`. Where a program supports both in one instruction, \
		           branch on the identity first and keep each branch internally consistent.",
	},
	LintExplanation {
		name: "require_explicit_discriminators_and_seed_namespaces",
		default_level: "warn",
		contract: "Give seed-based code an explicit byte-string namespace and make discriminator \
		           markers visible.",
		rationale: "Explicit namespaces keep seed derivation auditable and let the IDL extractor \
		            follow the program's account layout. Without them, two account roles can \
		            share a namespace and collide.",
		blessing: "Declare the namespace as a named `const` byte string, for example `const \
		           SEED_CONFIG: &[u8] = b\"config\";`. The lint checks that a namespace is \
		           visible, not that all namespaces differ; compare them across account roles \
		           yourself.",
	},
	LintExplanation {
		name: "require_explicit_token_2022_extension_policy",
		default_level: "deny",
		contract: "State which Token-2022 extensions an instruction accepts before it reads a \
		           Token-2022-capable mint's fields.",
		rationale: "Extensions change transfer and authority semantics — transfer fees, permanent \
		            delegates, transfer hooks. Reading only the legacy base fields silently \
		            treats those semantics as irrelevant, and accounting computed from them is \
		            wrong.",
		blessing: "Assert the policy explicitly on the mint: `assert_extensions_allowed(&[...])` \
		           for the extensions the program handles, or `assert_no_extensions()` when it \
		           handles none. Do that rather than allowing the lint, because the policy is the \
		           thing the lint is asking for.",
	},
	LintExplanation {
		name: "require_guarded_full_balance_drain",
		default_level: "warn",
		contract: "Gate an instruction that can sweep an account's entire balance behind a pause, \
		           circuit breaker, or withdrawal cap.",
		rationale: "An ungated full-balance drain is the shape real key-compromise exploits use. \
		            Once the sweep authority leaks, nothing on-chain slows the drain; a pause \
		            switch plus a per-window cap bounds the blast radius.",
		blessing: "Add the guard, or express the operation as a close when that is the intent, \
		           since a close states where the remaining lamports go. The guard must behave \
		           like one: its name states the pause or cap check (or it is a local wrapper \
		           returning `Result` that enforces such a guard before any early success \
		           return), its receiver or an argument is derived from the handler's parameters, \
		           and it stops the handler on the failing value with `?`, `unwrap`, or a branch \
		           that returns `Err` or panics; the polarity of `is_err`, `is_ok`, and \
		           `assert_eq!` is checked. A discarded result, a branch that returns `Ok`, a \
		           zero-argument or literal-only call, a closure, a local callee that can only \
		           return a literal success, and a generic wrapper whose concrete impl does not \
		           enforce the guard do not count. Where a drain is intended and bounded \
		           elsewhere, scope `#[allow]` to the handler and name the compensating control.",
	},
	LintExplanation {
		name: "require_idl_root_to_define_one_program_id",
		default_level: "warn",
		contract: "Define exactly one program ID at the crate root.",
		rationale: "IDL extraction starts from the crate root and expects a single declaration. \
		            Several IDs make the resolution ambiguous, and none means the extractor has \
		            no anchor.",
		blessing: "Keep one `declare_id!` at the root and move test or auxiliary IDs behind \
		           `#[cfg(test)]`. A crate that exports a program ID for another crate to consume \
		           should be a library, not the IDL root; scope `#[allow]` to that item if the \
		           layout must stay.",
	},
	LintExplanation {
		name: "require_post_cpi_balance_reload",
		default_level: "deny",
		contract: "Read a custody destination both before and after a token transfer CPI and \
		           account from the observed delta.",
		rationale: "Token-2022 transfer fees can make the amount received differ from the amount \
		            requested. Accounting from the requested amount rather than the observed \
		            balance delta credits the protocol with tokens it never received.",
		blessing: "Reload the destination with `amount()` after the CPI and compute the delta. \
		           This is the fix the lint asks for, so there is no exception to bless: a \
		           transfer whose fee is known to be zero still has a correct delta, and reading \
		           it costs one load.",
	},
	LintExplanation {
		name: "require_program_check_before_cpi",
		default_level: "deny",
		contract: "Validate a dynamic CPI target with `assert_address()`, `assert_addresses()`, \
		           or `assert_program()` against a compile-time program ID before invoking it.",
		rationale: "A dynamic program argument controls the CPI target. Without verifying that \
		            exact argument, an attacker substitutes a malicious program. An instruction \
		            argument is not a trusted expected ID — comparing two attacker-controlled \
		            values proves consistency, not authenticity — and success-side adapters such \
		            as `map()` can replace the validated binding before execution continues.",
		blessing: "Call `assert_address()`, `assert_addresses()`, or `assert_program()` against a \
		           compile-time ID on every continuing path before the invocation and do not \
		           discard the result. There is no sound way to bless an unverified dynamic CPI \
		           target: the check is the entire security property. Use a hardcoded program ID \
		           type when the target is in fact fixed.",
	},
	LintExplanation {
		name: "require_reason_for_duplicate_remaining_accounts",
		default_level: "deny",
		contract: "Document why a field opts out of distinctness with `#[pina(remaining, distinct \
		           = false)]`.",
		rationale: "Opting out of the duplicate-address check reintroduces the duplicate \
		            mutable-account vulnerability. The doc comment forces the author to state the \
		            reason, which is the only thing distinguishing a deliberate exception from a \
		            mistake.",
		blessing: "Write the doc comment. This lint _is_ the blessing mechanism: it asks for an \
		           explanation rather than forbidding the pattern, so an `#[allow]` would remove \
		           the only recorded justification.",
	},
	LintExplanation {
		name: "require_sysvar_assert_before_sysvar_use",
		default_level: "deny",
		contract: "Call `assert_sysvar()` on an account before reading it as a sysvar.",
		rationale: "A sysvar account is a specific address. Reading an account that merely has a \
		            sysvar-shaped layout without checking its address lets an attacker substitute \
		            data of their choosing for the clock, rent, or another sysvar.",
		blessing: "Assert the sysvar kind on the account. The assertion derives the expected \
		           address from the canonical sysvar ID, so it is strictly stronger than \
		           comparing an ID supplied by the caller; prefer it over an `#[allow]` even \
		           where the surrounding code looks sufficient.",
	},
	LintExplanation {
		name: "require_type_assert_before_zero_copy_cast",
		default_level: "deny",
		contract: "Convert account data through a guard-backed Pina conversion instead of a raw \
		           zero-copy cast.",
		rationale: "A raw cast reinterprets account bytes as a struct without proving the account \
		            is the expected type or that the data is large enough and aligned. The result \
		            is type cosplay: attacker-controlled bytes read as trusted fields.",
		blessing: "Use the guard-backed conversion — `assert_type()` on the account, or a typed \
		           loader such as `TokenAccountRef::from_account_view()` — which checks the \
		           account type and length and keeps the borrow alive. Do not bless a raw cast on \
		           account data: the check it skips is what makes reading the fields sound.",
	},
	LintExplanation {
		name: "require_writable_before_account_resize",
		default_level: "deny",
		contract: "Call `assert_writable()` on an account before resizing it.",
		rationale: "Writing to an account the transaction did not mark writable fails at runtime, \
		            and the resize is a write. The check also documents that the instruction \
		            intends to change the account's size, which a reviewer and the client both \
		            need to know.",
		blessing: "Assert writability on the account before the resize. A mutable fixed field \
		           parsed through `AccountsCursor::next_mut` already validated writability, so no \
		           exception is needed there; reach for `#[allow]` only on a path whose \
		           writability is established by a construct the lint cannot follow, and name it.",
	},
	LintExplanation {
		name: "require_zeroed_before_close",
		default_level: "deny",
		contract: "Call `zeroed()` on an account before closing it.",
		rationale: "A closed account's lamports are gone but its data survives until the account \
		            is reused. A later instruction that reads before writing sees the previous \
		            contents, so stale data can be reinterpreted as valid state.",
		blessing: "Zero the account before closing. Prefer Pina's `close_account_zeroed()`, which \
		           zeroes and closes as one operation and cannot be reordered. There is no sound \
		           reason to close a Pina account without zeroing it.",
	},
];

/// Look up one lint's reference entry by name.
///
/// Accepts the bare name or the `pina::`-prefixed spelling, because rustc
/// diagnostic output and `#[allow]` attributes both use the latter while
/// `pina.toml` uses the former.
#[must_use]
pub fn explain(name: &str) -> Option<&'static LintExplanation> {
	let name = name.strip_prefix("pina::").unwrap_or(name);
	LINT_REFERENCE
		.iter()
		.find(|explanation| explanation.name == name)
}

/// Return every lint name in catalog order.
#[must_use]
pub fn names() -> Vec<&'static str> {
	LINT_REFERENCE
		.iter()
		.map(|explanation| explanation.name)
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_reference_entry_is_complete() {
		for explanation in LINT_REFERENCE {
			assert!(
				!explanation.name.is_empty(),
				"a lint reference entry must be named"
			);
			assert!(
				matches!(explanation.default_level, "allow" | "warn" | "deny"),
				"{} has an invalid default level",
				explanation.name
			);
			assert!(
				explanation.contract.len() > 20,
				"{} needs a real contract statement",
				explanation.name
			);
			assert!(
				explanation.rationale.len() > 20,
				"{} needs a real rationale",
				explanation.name
			);
			assert!(
				explanation.blessing.len() > 20,
				"{} needs documented blessing guidance",
				explanation.name
			);
		}
	}

	#[test]
	fn reference_names_are_unique_and_sorted() {
		let mut names = names();
		let sorted = {
			let mut sorted = names.clone();
			sorted.sort_unstable();
			sorted
		};
		assert_eq!(names, sorted, "the reference follows catalog order");

		names.dedup();
		assert_eq!(
			names.len(),
			LINT_REFERENCE.len(),
			"lint names must be unique"
		);
	}

	#[test]
	fn explain_accepts_both_spellings() {
		assert!(explain("require_zeroed_before_close").is_some());
		assert!(explain("pina::require_zeroed_before_close").is_some());
		assert_eq!(
			explain("pina::require_zeroed_before_close"),
			explain("require_zeroed_before_close"),
		);
		assert_eq!(explain("not_a_lint"), None);
	}

	#[test]
	fn explain_preserves_a_name_that_only_looks_prefixed() {
		// `pina::` is stripped once; a lint genuinely named that way would still
		// resolve, and an unknown name stays unknown rather than matching by
		// accident.
		assert_eq!(explain("pina::pina::require_zeroed_before_close"), None);
	}

	#[test]
	fn every_blessing_names_a_code_identifier_or_refuses_to_bless() {
		// "Allow it" is not guidance. Every entry must either name a real API,
		// attribute, or type in backticks, or state plainly that the pattern
		// cannot be blessed soundly.
		const REFUSALS: &[&str] = &[
			"no sound",
			"no exception",
			"do not bless",
			"Write the doc comment",
		];

		for explanation in LINT_REFERENCE {
			let blessing = explanation.blessing;
			let names_code = blessing.contains('`');
			let refuses = REFUSALS.iter().any(|refusal| blessing.contains(refusal));
			assert!(
				names_code || refuses,
				"{} must name a concrete API in backticks or refuse to bless the pattern: \
				 {blessing}",
				explanation.name,
			);
		}
	}

	#[test]
	fn reference_matches_the_embedded_catalog() {
		// `lints.json` is the single source of truth shared with `pina_lints`,
		// which asserts its registered lints against that file. Pinning the
		// reference to the same file means a new lint cannot ship with
		// `--explain` silently missing it.
		let catalog = crate::lint_catalog::LintCatalog::global();

		assert_eq!(
			names(),
			catalog.names(),
			"every catalog lint needs a reference entry, in catalog order"
		);

		for explanation in LINT_REFERENCE {
			assert_eq!(
				Some(explanation.default_level),
				catalog.level(explanation.name),
				"{} has a default level that disagrees with the catalog",
				explanation.name,
			);
		}
	}
}
