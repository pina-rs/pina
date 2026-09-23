extern crate rustc_hir;
extern crate rustc_span;

use rustc_hir::intravisit::FnKind;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;

use crate::diagnostics;
use crate::shared;

crate::declare_late_lint! {
	/// ### What it does
	///
	/// Requires `assert_canonical_bump()` before validation-only code accepts a
	/// PDA with `assert_seeds_with_bump()` in an instruction path.
	///
	/// ### Why is this bad?
	///
	/// Accepting an arbitrary valid bump can create multiple addresses for one
	/// logical seed namespace and break uniqueness assumptions.
	/// `CreateProgramAccountWithBump` and `CreateProgramAccount` validate
	/// canonicality internally and do not require either assertion before
	/// invocation. `CreateProgramAccountWithUncheckedBump` deliberately does
	/// not: it checks that the supplied bump derives the account's address, so
	/// use it only where several addresses per seed namespace are acceptable.
	///
	/// `assert_stored_bump()` is the generated counterpart of
	/// `assert_seeds()`: it accepts a bump the handler already parsed from the
	/// same account, avoiding the re-parse `assert_seeds()` performs. The
	/// bump argument must resolve to that account's parsed state; a bump from
	/// instruction data or a different account fails this lint, because the
	/// stored-bump name would then be a claim the code does not make.
	pub REQUIRE_CANONICAL_BUMP_BEFORE_PDA_WRITE,
	Deny,
	"explicit PDA bumps must be proven canonical before use"
}

impl<'tcx> LateLintPass<'tcx> for RequireCanonicalBumpBeforePdaWrite {
	fn check_fn(
		&mut self,
		cx: &LateContext<'tcx>,
		_: FnKind<'tcx>,
		_: &'tcx rustc_hir::FnDecl<'tcx>,
		body: &'tcx rustc_hir::Body<'tcx>,
		_: rustc_span::Span,
		def_id: rustc_hir::def_id::LocalDefId,
	) {
		let def_path = cx.tcx.def_path_str(def_id.to_def_id());
		if shared::should_skip_def_path(&def_path)
			|| !shared::def_path_matches(&def_path, &["process", "instruction"])
		{
			return;
		}

		let facts = shared::collect_function_facts(cx, body);
		for (index, call) in facts.calls.iter().enumerate() {
			if call.method == "assert_stored_bump" {
				// The generated method names its own contract: `stored_bump`
				// must be a value the handler parsed from this same account.
				// Enforce the name rather than trusting it: the bump argument
				// must resolve (through aliases) to a field read of the
				// account, and that read must follow a parse of it.
				if stored_bump_provenance_ok(&facts, call) {
					continue;
				}

				diagnostics::emit(cx, REQUIRE_CANONICAL_BUMP_BEFORE_PDA_WRITE, |diag| {
					diag.span(call.span);
					diag.primary_message(
						"assert_stored_bump was given a bump that was not parsed from the account",
					);
					diag.help(
						"read the bump from the account's own state — for example capture it in \
						 the same `as_account`/`with_compact_account` parse that produced the \
						 other fields being validated — or use `assert_seeds()`, which performs \
						 that parse itself",
					);
					diag.help(shared::CONTROL_FLOW_LIMITATION_HELP);
				});
				continue;
			}

			if call.method != "assert_seeds_with_bump" {
				continue;
			}

			let has_canonical_check = shared::has_prior_method_with_receiver_match(
				&facts.calls,
				index,
				&["assert_canonical_bump", "assert_seeds"],
				&call.receiver,
			);
			if has_canonical_check {
				continue;
			}

			diagnostics::emit(cx, REQUIRE_CANONICAL_BUMP_BEFORE_PDA_WRITE, |diag| {
				diag.span(call.span);
				diag.primary_message(
					"explicit PDA bump used without first proving the canonical address",
				);
				diag.help(
					"call `account.assert_canonical_bump(seeds, program_id)?` before using an \
					 explicit bump in validation-only code, use `assert_seeds()`, or let \
					 `CreateProgramAccountWithBump` validate the canonical bump; \
					 `CreateProgramAccountWithUncheckedBump` skips that check on purpose",
				);
				diag.help(shared::CONTROL_FLOW_LIMITATION_HELP);
			});
		}
	}
}

/// Resolve the `stored_bump` argument's provenance through alias chains.
///
/// The argument is acceptable only when its canonical identity is a field
/// read whose base resolves to the account being validated — the shape
/// `account.<field>` that a parse of that account produces. Method-call
/// chains collapse to their receiver in the identity, so
/// `account.as_account()?.bump` records as `account.bump`. A field read of
/// anything else — `args.bump`, another account, a constant — fails.
fn stored_bump_provenance_ok(facts: &shared::FunctionFacts, call: &shared::CallInfo) -> bool {
	// `assert_stored_bump(account, stored_bump, .., program_id)` — the bump is
	// the second positional argument, the account the first.
	let Some(account_identity) = call.args.first().and_then(Option::as_deref) else {
		return false;
	};

	// Walk the bump argument's alias chain. Accept when the current identity
	// is a field read (`<base>.<field>`) whose base names the account, or
	// whose base binding aliases back to the account — the scoped-parse
	// shape `let state = account.as_account()?; ... state.bump`, where
	// `state`'s alias identity collapses `account.as_account()?` to
	// `account`.
	let mut identity = call.args.get(1).and_then(Option::as_deref);
	let mut binding = call.arg_bindings.get(1).copied().flatten();
	let mut visited = std::collections::HashSet::new();
	loop {
		if let Some(value) = identity {
			// The chain bottomed out at the account expression itself — the
			// scoped parse `let state = <account>.as_account()?` records the
			// alias identity as the account, dotted or not.
			if value == account_identity {
				return true;
			}
			if let Some((base, rest)) = value.split_once('.') {
				// A field read of the account itself: `account.bump`.
				if !rest.contains('.') && base == account_identity {
					return true;
				}
			}
		}

		let Some(current) = binding else {
			return false;
		};
		if !visited.insert(current) {
			return false;
		};
		let Some(alias) = facts.aliases.get(&current) else {
			return false;
		};
		identity = Some(alias.identity.as_str());
		binding = alias.binding;
	}
}
