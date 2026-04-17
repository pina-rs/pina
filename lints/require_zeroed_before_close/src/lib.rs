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
	/// Warns when accounts are closed without being zeroed first.
	///
	/// ### Why is this bad?
	///
	/// Closing an account without clearing its bytes first can leave stale data readable during the same transaction window.
	///
	/// ### Example
	///
	/// ```ignore
	/// // See lints/readme.md for the preferred pattern.
	/// ```
	pub REQUIRE_ZEROED_BEFORE_CLOSE,
	Deny,
	"account close should be preceded by `zeroed()` on the same account"
}

const TARGET_METHODS: &[&str] = &["close_with_recipient", "close"];
const TARGET_NEEDLES: &[&str] = &["process", "process_instruction", "instruction"];
const REQUIRED_METHODS: &[&str] = &["zeroed"];

impl<'tcx> LateLintPass<'tcx> for RequireZeroedBeforeClose {
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
			|| !shared::def_path_matches(&def_path, TARGET_NEEDLES)
		{
			return;
		}

		let facts = shared::collect_function_facts(body);
		for (index, call) in facts.calls.iter().enumerate() {
			if !TARGET_METHODS.contains(&call.method.as_str()) {
				continue;
			}

			let has_guard = shared::has_prior_method_with_receiver_match(
				&facts.calls,
				index,
				REQUIRED_METHODS,
				&call.receiver,
			);
			if !has_guard {
				cx.lint(REQUIRE_ZEROED_BEFORE_CLOSE, |diag| {
					diag.span(call.span);
					diag.primary_message(
						"account close should be preceded by `zeroed()` on the same account",
					);
					diag.help(
						"call `account.zeroed()?` before closing to reduce stale-data reuse risk",
					);
					diag.help(shared::CONTROL_FLOW_LIMITATION_HELP);
				});
			}
		}
	}
}
