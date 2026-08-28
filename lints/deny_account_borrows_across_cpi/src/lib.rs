#![feature(rustc_private)]

extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::HirId;
use rustc_hir::intravisit::FnKind;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;

dylint_linting::declare_late_lint! {
	/// ### What it does
	///
	/// Rejects CPI invocation while a local mutable account-data borrow remains
	/// alive.
	///
	/// ### Why is this bad?
	///
	/// The invoked program may need the same account data. Retaining a `RefMut`
	/// across CPI can make the invocation fail and obscures re-entrancy boundaries.
	pub DENY_ACCOUNT_BORROWS_ACROSS_CPI,
	Deny,
	"mutable account-data borrows must be dropped before CPI"
}

const BORROW_METHODS: &[&str] = &["try_borrow_mut", "as_account_mut"];
const CPI_METHODS: &[&str] = &[
	"invoke",
	"invoke_signed",
	"invoke_with_program",
	"invoke_signed_with_program",
];

fn contains_mutable_borrow(expr: &Expr<'_>) -> bool {
	match &expr.kind {
		ExprKind::MethodCall(segment, receiver, args, _) => {
			BORROW_METHODS.contains(&segment.ident.name.as_str())
				|| contains_mutable_borrow(receiver)
				|| args
					.iter()
					.any(|argument| contains_mutable_borrow(argument))
		}
		ExprKind::Match(scrutinee, ..)
		| ExprKind::DropTemps(scrutinee)
		| ExprKind::Use(scrutinee, _)
		| ExprKind::Type(scrutinee, _)
		| ExprKind::UnsafeBinderCast(_, scrutinee, _) => contains_mutable_borrow(scrutinee),
		ExprKind::Call(callee, args) => {
			contains_mutable_borrow(callee)
				|| args
					.iter()
					.any(|argument| contains_mutable_borrow(argument))
		}
		_ => false,
	}
}

fn local_binding(expr: &Expr<'_>) -> Option<HirId> {
	let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = &expr.kind else {
		return None;
	};
	let rustc_hir::def::Res::Local(binding) = path.res else {
		return None;
	};

	Some(binding)
}

struct Analyzer<'cx, 'tcx> {
	cx: &'cx LateContext<'tcx>,
}

impl<'tcx> Analyzer<'_, 'tcx> {
	fn visit_block(
		&self,
		block: &'tcx rustc_hir::Block<'tcx>,
		active: &mut HashMap<HirId, rustc_span::Span>,
	) {
		let mut block_bindings = Vec::new();

		for statement in block.stmts {
			match &statement.kind {
				rustc_hir::StmtKind::Let(local) => {
					if let Some(initializer) = local.init {
						self.visit_expr(initializer, active);
						if contains_mutable_borrow(initializer)
							&& let rustc_hir::PatKind::Binding(_, binding, ..) = local.pat.kind
						{
							active.insert(binding, initializer.span);
							block_bindings.push(binding);
						}
					}
				}
				rustc_hir::StmtKind::Expr(expr) | rustc_hir::StmtKind::Semi(expr) => {
					self.visit_expr(expr, active);
				}
				_ => {}
			}
		}

		if let Some(expr) = block.expr {
			self.visit_expr(expr, active);
		}

		for binding in block_bindings {
			active.remove(&binding);
		}
	}

	fn visit_expr(&self, expr: &'tcx Expr<'tcx>, active: &mut HashMap<HirId, rustc_span::Span>) {
		match &expr.kind {
			ExprKind::MethodCall(segment, receiver, args, _) => {
				self.visit_expr(receiver, active);
				for argument in *args {
					self.visit_expr(argument, active);
				}

				let method = segment.ident.name.as_str();
				if CPI_METHODS.contains(&method) && !active.is_empty() {
					self.cx.lint(DENY_ACCOUNT_BORROWS_ACROSS_CPI, |diag| {
						diag.span(expr.span);
						diag.primary_message(
							"CPI invoked while a mutable account-data borrow is still alive",
						);
						diag.help(
							"copy the required values, then call `drop(guard)` or end the \
							 borrow's scope before invoking another program",
						);
					});
				}
			}
			ExprKind::Call(callee, args) => {
				self.visit_expr(callee, active);
				for argument in *args {
					self.visit_expr(argument, active);
				}

				if let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = &callee.kind
					&& path
						.segments
						.last()
						.is_some_and(|segment| segment.ident.name.as_str() == "drop")
					&& let Some(binding) = args.first().and_then(|argument| local_binding(argument))
				{
					active.remove(&binding);
				}
			}
			ExprKind::Block(block, _) => self.visit_block(block, active),
			ExprKind::Match(scrutinee, arms, _) => {
				self.visit_expr(scrutinee, active);
				for arm in *arms {
					let mut branch = active.clone();
					self.visit_expr(arm.body, &mut branch);
				}
			}
			ExprKind::If(condition, then, otherwise) => {
				self.visit_expr(condition, active);
				let mut branch = active.clone();
				self.visit_expr(then, &mut branch);
				if let Some(otherwise) = otherwise {
					let mut branch = active.clone();
					self.visit_expr(otherwise, &mut branch);
				}
			}
			ExprKind::Loop(block, ..) => self.visit_block(block, active),
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
			| ExprKind::UnsafeBinderCast(_, inner, _) => self.visit_expr(inner, active),
			ExprKind::Binary(_, left, right)
			| ExprKind::Assign(left, right, _)
			| ExprKind::AssignOp(_, left, right) => {
				self.visit_expr(left, active);
				self.visit_expr(right, active);
			}
			ExprKind::Index(base, index, _) => {
				self.visit_expr(base, active);
				self.visit_expr(index, active);
			}
			ExprKind::Let(let_expr) => self.visit_expr(let_expr.init, active),
			ExprKind::Tup(expressions) | ExprKind::Array(expressions) => {
				for expression in *expressions {
					self.visit_expr(expression, active);
				}
			}
			ExprKind::Struct(_, fields, tail) => {
				for field in *fields {
					self.visit_expr(field.expr, active);
				}
				if let rustc_hir::StructTailExpr::Base(base) = tail {
					self.visit_expr(base, active);
				}
			}
			ExprKind::Ret(Some(inner)) | ExprKind::Break(_, Some(inner)) => {
				self.visit_expr(inner, active);
			}
			_ => {}
		}
	}
}

impl<'tcx> LateLintPass<'tcx> for DenyAccountBorrowsAcrossCpi {
	fn check_fn(
		&mut self,
		cx: &LateContext<'tcx>,
		_: FnKind<'tcx>,
		_: &'tcx rustc_hir::FnDecl<'tcx>,
		body: &'tcx rustc_hir::Body<'tcx>,
		_: rustc_span::Span,
		_: rustc_hir::def_id::LocalDefId,
	) {
		Analyzer { cx }.visit_expr(body.value, &mut HashMap::new());
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn ui() {
		dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
	}
}
