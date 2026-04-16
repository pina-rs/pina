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
	/// Warns when lamport mutation helpers are used without first proving program ownership.
	///
	/// ### Why is this bad?
	///
	/// Direct lamport transfers and close flows assume the sender account is owned by the executing program; skipping that check can mutate accounts that should be immutable.
	///
	/// ### Example
	///
	/// ```ignore
	/// // See lints/readme.md for the preferred pattern.
	/// ```
	pub REQUIRE_PROGRAM_OWNED_BEFORE_LAMPORT_MUTATION,
	Deny,
	"lamport mutation should be preceded by a program-owner check on the same account"
}

const TARGET_METHODS: &[&str] = &["send"];
const TARGET_NEEDLES: &[&str] = &["process", "process_instruction", "instruction"];
const REQUIRED_METHODS: &[&str] = &["assert_owner", "assert_owners", "assert_type"];

impl<'tcx> LateLintPass<'tcx> for RequireProgramOwnedBeforeLamportMutation {
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
				cx.lint(REQUIRE_PROGRAM_OWNED_BEFORE_LAMPORT_MUTATION, |diag| {
					diag.span(call.span);
					diag.primary_message(
						"lamport mutation should be preceded by a program-owner check on the same \
						 account",
					);
					diag.help(
						"add `account.assert_owner(&program_id)?` or \
						 `account.assert_owners(&[...])?` before mutating lamports",
					);
				});
			}
		}
	}
}
