#![feature(rustc_private)]

#[path = "../../shared.rs"]
mod shared;

extern crate rustc_hir;
extern crate rustc_span;

use rustc_hir::intravisit::FnKind;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;

dylint_linting::declare_late_lint! {
	/// ### What it does
	///
	/// Requires `assert_canonical_bump()` before a PDA is accepted with
	/// `assert_seeds_with_bump()` in an instruction path.
	///
	/// ### Why is this bad?
	///
	/// Accepting an arbitrary valid bump can create multiple addresses for one
	/// logical seed namespace and break uniqueness assumptions.
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

			cx.lint(REQUIRE_CANONICAL_BUMP_BEFORE_PDA_WRITE, |diag| {
				diag.span(call.span);
				diag.primary_message(
					"explicit PDA bump used without first proving the canonical address",
				);
				diag.help(
					"call `account.assert_canonical_bump(seeds, program_id)?` before using an \
					 explicit bump, or use `assert_seeds()`",
				);
				diag.help(shared::CONTROL_FLOW_LIMITATION_HELP);
			});
		}
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn ui() {
		dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
	}
}
