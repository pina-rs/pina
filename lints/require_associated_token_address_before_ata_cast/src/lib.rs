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
	/// Warns when associated token accounts are interpreted without checking the derived ATA address first.
	///
	/// ### Why is this bad?
	///
	/// ATA layouts are easy to spoof unless the address is checked against the expected owner, mint, and token program triple first.
	///
	/// ### Example
	///
	/// ```ignore
	/// // See lints/readme.md for the preferred pattern.
	/// ```
	pub REQUIRE_ASSOCIATED_TOKEN_ADDRESS_BEFORE_ATA_CAST,
	Deny,
	"ATA casts should be preceded by `assert_associated_token_address()`"
}

const TARGET_METHODS: &[&str] = &["as_associated_token_account"];
const TARGET_NEEDLES: &[&str] = &["process", "process_instruction", "instruction"];
const REQUIRED_METHODS: &[&str] = &["assert_associated_token_address"];

impl<'tcx> LateLintPass<'tcx> for RequireAssociatedTokenAddressBeforeAtaCast {
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
				cx.lint(REQUIRE_ASSOCIATED_TOKEN_ADDRESS_BEFORE_ATA_CAST, |diag| {
					diag.span(call.span);
					diag.primary_message(
						"ATA casts should be preceded by `assert_associated_token_address()`",
					);
					diag.help(
						"call `account.assert_associated_token_address(owner, mint, \
						 token_program)?` before `as_associated_token_account()`",
					);
				});
			}
		}
	}
}
