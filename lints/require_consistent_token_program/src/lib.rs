#![feature(rustc_private)]

#[path = "../../shared.rs"]
mod shared;

extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashSet;

use rustc_hir::intravisit::FnKind;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;

dylint_linting::declare_late_lint! {
	/// ### What it does
	///
	/// Requires token parsing, ATA derivation, and dynamic token CPI calls in one
	/// instruction path to use one token-program identity.
	///
	/// ### Why is this bad?
	///
	/// Mixing program identities can validate an account under one token program
	/// and invoke another, invalidating ownership and address assumptions.
	pub REQUIRE_CONSISTENT_TOKEN_PROGRAM,
	Deny,
	"token operations in one instruction must share one program identity"
}

fn program_argument(call: &shared::CallInfo) -> Option<&str> {
	let index = match call.method.as_str() {
		"as_token_mint_for_program" | "as_token_account_for_program" => 0,
		"as_associated_token_account"
		| "as_associated_token_account_checked"
		| "assert_associated_token_address" => 2,
		"invoke_with_program" => 0,
		"invoke_signed_with_program" => 1,
		_ => return None,
	};

	call.args.get(index).and_then(Option::as_deref)
}

impl<'tcx> LateLintPass<'tcx> for RequireConsistentTokenProgram {
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

		let facts = shared::collect_function_facts(body);
		let mut expected = None;
		let mut previous_usage = None;
		let mut reported_reassignments = HashSet::new();
		for call in &facts.calls {
			let Some(argument) = program_argument(call) else {
				continue;
			};
			let identity = argument;
			let Some(first) = expected else {
				expected = Some(identity);
				previous_usage = Some(call);
				continue;
			};

			if identity == first {
				let was_reassigned = previous_usage.is_some_and(|previous: &shared::CallInfo| {
					facts.assignments.iter().any(|assignment| {
						assignment.identity == identity
							&& previous.span.hi() <= assignment.span.lo()
							&& assignment.span.hi() <= call.span.lo()
					})
				});
				if was_reassigned && reported_reassignments.insert(identity.to_string()) {
					cx.lint(REQUIRE_CONSISTENT_TOKEN_PROGRAM, |diag| {
						diag.span(call.span);
						diag.primary_message(format!(
							"token-program value `{identity}` was reassigned between token \
							 operations"
						));
						diag.help(
							"validate the token-program account once, copy its address into an \
							 immutable binding, and reuse that binding for every token operation",
						);
					});
				}
				previous_usage = Some(call);
				continue;
			}

			cx.lint(REQUIRE_CONSISTENT_TOKEN_PROGRAM, |diag| {
				diag.span(call.span);
				diag.primary_message(format!(
					"token operation uses `{identity}` after the instruction established `{first}`"
				));
				diag.help(
					"validate the token-program account once, copy its address, and pass that \
					 same value to token parsing, ATA checks, and CPI",
				);
			});
			previous_usage = Some(call);
		}
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn ui() {
		dylint_testing::ui_test(
			env!("CARGO_PKG_NAME"),
			concat!(env!("CARGO_MANIFEST_DIR"), "/ui"),
		);
	}
}
