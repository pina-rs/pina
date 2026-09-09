extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::HirId;
use rustc_hir::Pat;
use rustc_hir::intravisit::FnKind;
use rustc_hir::intravisit::Visitor;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;

crate::declare_late_lint! {
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

const CPI_METHODS: &[&str] = &[
	"invoke",
	"invoke_signed",
	"invoke_with_program",
	"invoke_signed_with_program",
];

fn method_def_path(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
	cx.typeck_results()
		.type_dependent_def_id(expr.hir_id)
		.map(|def_id| cx.tcx.def_path_str(def_id))
}

fn is_cpi_invocation(cx: &LateContext<'_>, expr: &Expr<'_>, method: &str) -> bool {
	if !CPI_METHODS.contains(&method) {
		return false;
	}

	method_def_path(cx, expr).is_some_and(|path| {
		let path = path.to_ascii_lowercase();
		path.starts_with("pina::")
			|| path.starts_with("pinocchio::")
			|| path.starts_with("pinocchio_")
			|| path.split("::").any(|segment| {
				segment == "cpi"
					|| segment == "instructions"
					|| segment.ends_with("instruction")
					|| segment.starts_with("cpi")
			})
	})
}

fn is_mutable_account_borrow_guard(cx: &LateContext<'_>, ty: rustc_middle::ty::Ty<'_>) -> bool {
	let Some(definition) = ty.peel_refs().ty_adt_def() else {
		return false;
	};
	let definition = definition.did();

	cx.tcx.crate_name(definition.krate).as_str() == "solana_account_view"
		&& cx.tcx.item_name(definition).as_str() == "RefMut"
}

struct GuardPatternCollector<'cx, 'tcx, 'bindings> {
	cx: &'cx LateContext<'tcx>,
	bindings: &'bindings mut Vec<HirId>,
}

impl<'tcx> Visitor<'tcx> for GuardPatternCollector<'_, 'tcx, '_> {
	fn visit_pat(&mut self, pattern: &'tcx Pat<'tcx>) {
		if let rustc_hir::PatKind::Binding(_, binding, ..) = pattern.kind
			&& is_mutable_account_borrow_guard(self.cx, self.cx.typeck_results().pat_ty(pattern))
		{
			self.bindings.push(binding);
		}

		rustc_hir::intravisit::walk_pat(self, pattern);
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

fn is_drop_callee(cx: &LateContext<'_>, callee: &Expr<'_>) -> bool {
	let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = &callee.kind else {
		return false;
	};

	match path.res {
		rustc_hir::def::Res::Def(_, definition) => {
			matches!(
				cx.tcx.def_path_str(definition).as_str(),
				"std::mem::drop" | "core::mem::drop"
			)
		}
		_ => false,
	}
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
						let mut bindings = Vec::new();
						GuardPatternCollector {
							cx: self.cx,
							bindings: &mut bindings,
						}
						.visit_pat(local.pat);
						for binding in bindings {
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
				if is_cpi_invocation(self.cx, expr, method) && !active.is_empty() {
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

				if is_drop_callee(self.cx, callee)
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
					if let Some(guard) = arm.guard {
						self.visit_expr(guard, &mut branch);
					}
					self.visit_expr(arm.body, &mut branch);
				}
			}
			ExprKind::Closure(closure) => {
				let mut closure_active = active.clone();
				self.visit_expr(
					self.cx.tcx.hir_body(closure.body).value,
					&mut closure_active,
				);
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
