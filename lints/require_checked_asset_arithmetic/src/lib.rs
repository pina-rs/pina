#![feature(rustc_private)]

#[path = "../../shared.rs"]
mod shared;

extern crate rustc_hir;
extern crate rustc_span;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::intravisit::FnKind;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;

dylint_linting::declare_late_lint! {
	/// ### What it does
	///
	/// Rejects raw, saturating, and wrapping arithmetic on values whose names
	/// indicate balances, amounts, prices, rewards, stakes, supply, or lamports.
	///
	/// ### Why is this bad?
	///
	/// Silent overflow, underflow, or saturation corrupts economic invariants.
	/// Asset arithmetic should fail explicitly with checked operations.
	pub REQUIRE_CHECKED_ASSET_ARITHMETIC,
	Deny,
	"asset arithmetic must use checked operations"
}

const ASSET_TERMS: &[&str] = &[
	"amount", "balance", "lamport", "price", "reward", "stake", "supply", "value",
];
const UNCHECKED_METHODS: &[&str] = &[
	"saturating_add",
	"saturating_sub",
	"saturating_mul",
	"saturating_div",
	"wrapping_add",
	"wrapping_sub",
	"wrapping_mul",
	"wrapping_div",
];

fn looks_like_asset(expr: &Expr<'_>) -> bool {
	shared::expression_identity(expr).is_some_and(|identity| {
		let identity = identity.to_ascii_lowercase();
		ASSET_TERMS.iter().any(|term| identity.contains(term))
	})
}

fn lint(cx: &LateContext<'_>, span: rustc_span::Span) {
	cx.lint(REQUIRE_CHECKED_ASSET_ARITHMETIC, |diag| {
		diag.span(span);
		diag.primary_message("asset arithmetic can overflow, underflow, or silently saturate");
		diag.help(
			"use `checked_add`, `checked_sub`, `checked_mul`, or `checked_div` and return an \
			 explicit program error on failure",
		);
	});
}

fn visit_expr(cx: &LateContext<'_>, expr: &Expr<'_>) {
	match &expr.kind {
		ExprKind::MethodCall(segment, receiver, args, _) => {
			visit_expr(cx, receiver);
			for argument in *args {
				visit_expr(cx, argument);
			}
			if UNCHECKED_METHODS.contains(&segment.ident.name.as_str())
				&& looks_like_asset(receiver)
			{
				lint(cx, expr.span);
			}
		}
		ExprKind::Binary(operation, left, right) => {
			visit_expr(cx, left);
			visit_expr(cx, right);
			if matches!(
				operation.node,
				rustc_hir::BinOpKind::Add
					| rustc_hir::BinOpKind::Sub
					| rustc_hir::BinOpKind::Mul
					| rustc_hir::BinOpKind::Div
			) && (looks_like_asset(left) || looks_like_asset(right))
			{
				lint(cx, expr.span);
			}
		}
		ExprKind::Call(callee, args) => {
			visit_expr(cx, callee);
			for argument in *args {
				visit_expr(cx, argument);
			}
		}
		ExprKind::Block(block, _) => {
			for statement in block.stmts {
				match &statement.kind {
					rustc_hir::StmtKind::Let(local) => {
						if let Some(initializer) = local.init {
							visit_expr(cx, initializer);
						}
					}
					rustc_hir::StmtKind::Expr(expr) | rustc_hir::StmtKind::Semi(expr) => {
						visit_expr(cx, expr);
					}
					_ => {}
				}
			}
			if let Some(expr) = block.expr {
				visit_expr(cx, expr);
			}
		}
		ExprKind::Match(scrutinee, arms, _) => {
			visit_expr(cx, scrutinee);
			for arm in *arms {
				visit_expr(cx, arm.body);
			}
		}
		ExprKind::If(condition, then, otherwise) => {
			visit_expr(cx, condition);
			visit_expr(cx, then);
			if let Some(otherwise) = otherwise {
				visit_expr(cx, otherwise);
			}
		}
		ExprKind::Loop(block, ..) => {
			for statement in block.stmts {
				if let rustc_hir::StmtKind::Expr(expr) | rustc_hir::StmtKind::Semi(expr) =
					statement.kind
				{
					visit_expr(cx, expr);
				}
			}
		}
		ExprKind::Unary(_, inner)
		| ExprKind::Use(inner, _)
		| ExprKind::Cast(inner, _)
		| ExprKind::Type(inner, _)
		| ExprKind::DropTemps(inner)
		| ExprKind::AddrOf(_, _, inner)
		| ExprKind::Field(inner, _)
		| ExprKind::Repeat(inner, _)
		| ExprKind::Yield(inner, _)
		| ExprKind::Become(inner)
		| ExprKind::UnsafeBinderCast(_, inner, _) => visit_expr(cx, inner),
		ExprKind::Assign(left, right, _) | ExprKind::AssignOp(_, left, right) => {
			visit_expr(cx, left);
			visit_expr(cx, right);
		}
		ExprKind::Index(base, index, _) => {
			visit_expr(cx, base);
			visit_expr(cx, index);
		}
		ExprKind::Let(let_expr) => visit_expr(cx, let_expr.init),
		ExprKind::Tup(expressions) | ExprKind::Array(expressions) => {
			for expression in *expressions {
				visit_expr(cx, expression);
			}
		}
		ExprKind::Struct(_, fields, tail) => {
			for field in *fields {
				visit_expr(cx, field.expr);
			}
			if let rustc_hir::StructTailExpr::Base(base) = tail {
				visit_expr(cx, base);
			}
		}
		_ => {}
	}
}

impl<'tcx> LateLintPass<'tcx> for RequireCheckedAssetArithmetic {
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

		visit_expr(cx, body.value);
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn ui() {
		dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
	}
}
