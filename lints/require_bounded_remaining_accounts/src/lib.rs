#![feature(rustc_private)]

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
	/// Rejects loops over remaining accounts unless the iterator includes an
	/// explicit `.take(MAX)` bound.
	///
	/// ### Why is this bad?
	///
	/// Caller-controlled account counts can turn linear per-account work into
	/// compute exhaustion. A visible protocol bound makes the cost auditable.
	pub REQUIRE_BOUNDED_REMAINING_ACCOUNTS,
	Deny,
	"remaining-account loops require an explicit maximum"
}

fn visit_expr(cx: &LateContext<'_>, expr: &Expr<'_>) {
	match &expr.kind {
		ExprKind::Loop(block, ..) => {
			let snippet = cx
				.sess()
				.source_map()
				.span_to_snippet(expr.span)
				.unwrap_or_default();
			if snippet.to_ascii_lowercase().contains("remaining") && !snippet.contains(".take(") {
				cx.lint(REQUIRE_BOUNDED_REMAINING_ACCOUNTS, |diag| {
					diag.span(expr.span);
					diag.primary_message(
						"remaining accounts are processed without an explicit bound",
					);
					diag.help(
						"define a protocol maximum and iterate with \
						 `remaining.iter().take(MAX_REMAINING_ACCOUNTS)`",
					);
				});
			}

			for statement in block.stmts {
				if let rustc_hir::StmtKind::Expr(expr) | rustc_hir::StmtKind::Semi(expr) =
					statement.kind
				{
					visit_expr(cx, expr);
				}
			}
		}
		ExprKind::MethodCall(_, receiver, args, _) => {
			visit_expr(cx, receiver);
			for argument in *args {
				visit_expr(cx, argument);
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
		ExprKind::Binary(_, left, right)
		| ExprKind::Assign(left, right, _)
		| ExprKind::AssignOp(_, left, right) => {
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

impl<'tcx> LateLintPass<'tcx> for RequireBoundedRemainingAccounts {
	fn check_fn(
		&mut self,
		cx: &LateContext<'tcx>,
		_: FnKind<'tcx>,
		_: &'tcx rustc_hir::FnDecl<'tcx>,
		body: &'tcx rustc_hir::Body<'tcx>,
		_: rustc_span::Span,
		_: rustc_hir::def_id::LocalDefId,
	) {
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
