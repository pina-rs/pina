extern crate rustc_hir;
extern crate rustc_span;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::def::DefKind;
use rustc_hir::def::Res;
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

const REMAINING_MUT_PATH: &str = "pina::traits::AccountsCursor::remaining_mut";
const ACCOUNTS_DERIVE_PATH: &str = "pina_macros::Accounts";

fn is_pina_remaining_mut(cx: &LateContext<'_>, definition: rustc_hir::def_id::DefId) -> bool {
	cx.tcx.def_path_str(definition) == REMAINING_MUT_PATH
}

fn path_definition(
	cx: &LateContext<'_>,
	expression: &Expr<'_>,
) -> Option<rustc_hir::def_id::DefId> {
	let ExprKind::Path(path) = &expression.kind else {
		return None;
	};
	let Res::Def(DefKind::AssocFn, definition) = cx.qpath_res(path, expression.hir_id) else {
		return None;
	};

	Some(definition)
}

fn method_definition(
	cx: &LateContext<'_>,
	expression: &Expr<'_>,
) -> Option<rustc_hir::def_id::DefId> {
	cx.typeck_results().type_dependent_def_id(expression.hir_id)
}

fn is_direct_call_callee(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
	matches!(
		cx.tcx.parent_hir_node(expression.hir_id),
		rustc_hir::Node::Expr(Expr {
			kind: ExprKind::Call(callee, _),
			..
		}) if callee.hir_id == expression.hir_id
	)
}

fn is_accounts_derive_expansion(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
	if !expression.span.from_expansion() {
		return false;
	}

	expression
		.span
		.ctxt()
		.outer_expn_data()
		.macro_def_id
		.is_some_and(|definition| cx.tcx.def_path_str(definition) == ACCOUNTS_DERIVE_PATH)
}

impl<'tcx> LateLintPass<'tcx> for DenyUncheckedRemainingMut {
	fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
		if is_accounts_derive_expansion(cx, expr) {
			return;
		}

		let is_unchecked_call = match &expr.kind {
			ExprKind::MethodCall(_, _, arguments, _) => {
				arguments.is_empty()
					&& method_definition(cx, expr)
						.is_some_and(|definition| is_pina_remaining_mut(cx, definition))
			}
			ExprKind::Call(callee, _) => {
				path_definition(cx, callee)
					.is_some_and(|definition| is_pina_remaining_mut(cx, definition))
			}
			ExprKind::Path(_) => {
				!is_direct_call_callee(cx, expr)
					&& path_definition(cx, expr)
						.is_some_and(|definition| is_pina_remaining_mut(cx, definition))
			}
			_ => false,
		};

		if !is_unchecked_call {
			return;
		}

		cx.lint(DENY_UNCHECKED_REMAINING_MUT, |diag| {
			diag.span(expr.span.source_callsite());
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
