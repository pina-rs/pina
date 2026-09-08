extern crate rustc_hir;
extern crate rustc_span;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;

crate::declare_late_lint! {
	/// ### What it does
	///
	/// Rejects direct calls to Pina's `AccountsCursor::remaining_mut()`.
	///
	/// ### Why is this bad?
	///
	/// The unchecked method validates writability but preserves duplicate
	/// addresses. Mutating both aliases can apply one logical update twice.
	pub DENY_UNCHECKED_REMAINING_MUT,
	Deny,
	"direct mutable remaining-account access permits duplicate writable aliases"
}

const ACCOUNTS_CURSOR_PATH: &str = "pina::traits::AccountsCursor";

fn is_pina_remaining_mut(cx: &LateContext<'_>, expr: &Expr<'_>, receiver: &Expr<'_>) -> bool {
	let Some(method) = cx.typeck_results().type_dependent_def_id(expr.hir_id) else {
		return false;
	};
	let receiver_type = cx.typeck_results().expr_ty(receiver).peel_refs();
	let Some(receiver_definition) = receiver_type.ty_adt_def() else {
		return false;
	};

	cx.tcx.crate_name(method.krate).as_str() == "pina"
		&& cx.tcx.item_name(method).as_str() == "remaining_mut"
		&& cx.tcx.def_path_str(receiver_definition.did()) == ACCOUNTS_CURSOR_PATH
}

impl<'tcx> LateLintPass<'tcx> for DenyUncheckedRemainingMut {
	fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
		let ExprKind::MethodCall(segment, receiver, arguments, _) = &expr.kind else {
			return;
		};

		if expr.span.from_expansion()
			|| segment.ident.name.as_str() != "remaining_mut"
			|| !arguments.is_empty()
			|| !is_pina_remaining_mut(cx, expr, receiver)
		{
			return;
		}

		cx.lint(DENY_UNCHECKED_REMAINING_MUT, |diag| {
			diag.span(expr.span);
			diag.primary_message(
				"direct mutable remaining-account access permits duplicate writable aliases",
			);
			diag.help("use `remaining_mut_distinct()` to reject duplicate addresses");
			diag.help(
				"if duplicates are intentional, contain the call in a reviewed helper and allow \
				 this lint there with a reason",
			);
		});
	}
}
