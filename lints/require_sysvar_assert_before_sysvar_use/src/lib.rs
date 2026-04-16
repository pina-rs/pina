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
	/// Warns when sysvar-like accounts are used without asserting their sysvar identity first.
	///
	/// ### Why is this bad?
	///
	/// Spoofed sysvar accounts can distort rent, clock, and instruction-data logic.
	///
	/// ### Example
	///
	/// ```ignore
	/// // See lints/readme.md for the preferred pattern.
	/// ```
	pub REQUIRE_SYSVAR_ASSERT_BEFORE_SYSVAR_USE,
	Deny,
	"sysvar access should be preceded by `assert_sysvar()` on the same account"
}

const TARGET_NEEDLES: &[&str] = &["process", "process_instruction", "instruction", "sysvar"];
const REQUIRED_METHODS: &[&str] = &["assert_sysvar"];
fn is_sysvar_receiver(name: &str) -> bool {
	name == "clock"
		|| name == "rent"
		|| name == "instructions"
		|| name.ends_with("_sysvar")
		|| name.ends_with("_instructions")
}

impl<'tcx> LateLintPass<'tcx> for RequireSysvarAssertBeforeSysvarUse {
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
			if call.method == "assert_sysvar" {
				continue;
			}

			let looks_like_sysvar_use = call.receiver.as_deref().is_some_and(is_sysvar_receiver)
				|| call.path.as_deref().is_some_and(|path| {
					path.contains("load_current_index")
						|| path.contains("load_instruction_at")
						|| path.contains("rent")
						|| path.contains("clock")
				});

			if !looks_like_sysvar_use {
				continue;
			}

			let has_guard = shared::has_prior_method_with_receiver_match(
				&facts.calls,
				index,
				REQUIRED_METHODS,
				&call.receiver,
			);
			if !has_guard {
				cx.lint(REQUIRE_SYSVAR_ASSERT_BEFORE_SYSVAR_USE, |diag| {
					diag.span(call.span);
					diag.primary_message(
						"sysvar access should be preceded by `assert_sysvar()` on the same account",
					);
					diag.help(
						"call `sysvar_account.assert_sysvar(&sysvar::ID)?` before reading time or \
						 rent information",
					);
				});
			}
		}
	}
}
