extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_hir::Body;
use rustc_hir::ByRef;
use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::HirId;
use rustc_hir::MatchSource;
use rustc_hir::Mutability;
use rustc_hir::Pat;
use rustc_hir::PatKind;
use rustc_hir::StmtKind;
use rustc_hir::UnOp;
use rustc_hir::intravisit::FnKind;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_middle::ty;
use rustc_middle::ty::Ty;
use rustc_middle::ty::adjustment::Adjust;
use rustc_middle::ty::adjustment::AutoBorrow;
use rustc_middle::ty::adjustment::AutoBorrowMutability;
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::diagnostics;

crate::declare_late_lint! {
	/// ### What it does
	///
	/// Rejects taking a mutable borrow of an `AccountView` that was copied or
	/// cloned out of a shared `&AccountView`, `&[AccountView]`, or
	/// `Option<&AccountView>` reference.
	///
	/// ### Why is this bad?
	///
	/// `pinocchio::AccountView` implements `Copy`, so copying `*self.readonly`
	/// into a mutable local or temporary bypasses Rust's `&AccountView`
	/// immutability guarantee and Pina's `#[derive(Accounts)]` mutable-aliasing
	/// checks.
	pub DENY_READONLY_ACCOUNT_COPY_ESCALATION,
	Deny,
	"cannot escalate a readonly `&AccountView` to mutable by copying `AccountView`"
}

fn is_account_view_ty(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
	let Some(definition) = ty.ty_adt_def() else {
		return false;
	};
	let did = definition.did();

	matches!(
		cx.tcx.crate_name(did.krate).as_str(),
		"solana_account_view" | "pinocchio" | "pina"
	) && cx.tcx.item_name(did).as_str() == "AccountView"
}

fn is_shared_ref_to_account_view(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
	matches!(
		ty.kind(),
		ty::Ref(_, inner, Mutability::Not) if is_account_view_ty(cx, *inner)
	)
}

fn is_shared_ref_to_account_view_slice_or_array(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
	let ty::Ref(_, inner, Mutability::Not) = ty.kind() else {
		return false;
	};

	match inner.kind() {
		ty::Slice(element) | ty::Array(element, _) => is_account_view_ty(cx, *element),
		_ => false,
	}
}

fn is_option_of_shared_account_view_ref(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
	let peeled = ty.peel_refs();
	let Some(definition) = peeled.ty_adt_def() else {
		return false;
	};
	if cx.tcx.item_name(definition.did()).as_str() != "Option" {
		return false;
	}
	let ty::Adt(_, args) = peeled.kind() else {
		return false;
	};

	args.types()
		.next()
		.is_some_and(|inner| is_shared_ref_to_account_view(cx, inner))
}

fn ty_contains_owned_account_view(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
	if is_account_view_ty(cx, ty) {
		return true;
	}
	let ty::Adt(definition, args) = ty.kind() else {
		return false;
	};
	let name = cx.tcx.item_name(definition.did());
	if matches!(name.as_str(), "Option" | "Result") {
		return args
			.types()
			.next()
			.is_some_and(|inner| is_account_view_ty(cx, inner));
	}

	false
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

fn is_clone_callee(cx: &LateContext<'_>, callee: &Expr<'_>) -> bool {
	let ExprKind::Path(path) = &callee.kind else {
		return false;
	};
	let rustc_hir::def::Res::Def(_, definition) = cx.qpath_res(path, callee.hir_id) else {
		return false;
	};

	matches!(
		cx.tcx.def_path_str(definition).as_str(),
		"core::clone::Clone::clone" | "std::clone::Clone::clone"
	)
}

fn is_try_branch_callee(cx: &LateContext<'_>, callee: &Expr<'_>) -> bool {
	let ExprKind::Path(path) = &callee.kind else {
		return false;
	};
	let rustc_hir::def::Res::Def(_, definition) = cx.qpath_res(path, callee.hir_id) else {
		return false;
	};
	let def_path = cx.tcx.def_path_str(definition);

	cx.tcx.crate_name(definition.krate).as_str() == "core"
		&& def_path.ends_with("::ops::Try::branch")
}

fn receiver_has_mut_autoref(cx: &LateContext<'_>, receiver: &Expr<'_>) -> bool {
	cx.typeck_results()
		.expr_adjustments(receiver)
		.iter()
		.any(|adjustment| {
			matches!(
				adjustment.kind,
				Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Mut { .. }))
			)
		})
}

fn emit_escalation(cx: &LateContext<'_>, use_span: Span, copy_span: Span) {
	diagnostics::emit(cx, DENY_READONLY_ACCOUNT_COPY_ESCALATION, |diag| {
		diag.span(use_span.source_callsite());
		diag.primary_message(
			"mutably borrowing an `AccountView` copied from a shared reference bypasses readonly \
			 and mutable-aliasing checks",
		);
		let copy_callsite = copy_span.source_callsite();
		if copy_callsite != use_span.source_callsite() {
			diag.span_note(
				copy_callsite,
				"readonly `AccountView` was copied or cloned here",
			);
		}
		diag.help(
			"declare the account slot as `&'a mut AccountView` in `#[derive(Accounts)]` (or accept \
			 `&mut AccountView` directly) so Pina enforces writability and duplicate-mutable \
			 checks before execution",
		);
	});
}

struct Analyzer<'cx, 'tcx> {
	cx: &'cx LateContext<'tcx>,
}

impl<'tcx> Analyzer<'_, 'tcx> {
	fn readonly_copy_source(
		&self,
		expr: &'tcx Expr<'tcx>,
		tainted: &HashMap<HirId, Span>,
	) -> Option<Span> {
		let expr_ty = self.cx.typeck_results().expr_ty(expr);

		match &expr.kind {
			ExprKind::Unary(UnOp::Deref, inner) => {
				let inner_ty = self.cx.typeck_results().expr_ty(inner);
				if is_account_view_ty(self.cx, expr_ty)
					&& is_shared_ref_to_account_view(self.cx, inner_ty)
				{
					return Some(expr.span.source_callsite());
				}
				None
			}
			ExprKind::Index(base, ..) => {
				let base_ty = self.cx.typeck_results().expr_ty(base);
				if is_account_view_ty(self.cx, expr_ty)
					&& is_shared_ref_to_account_view_slice_or_array(self.cx, base_ty)
				{
					return Some(expr.span.source_callsite());
				}
				None
			}
			ExprKind::Path(_) => {
				local_binding(expr).and_then(|binding| tainted.get(&binding).copied())
			}
			ExprKind::MethodCall(segment, receiver, args, _) => {
				let method = segment.ident.name.as_str();
				let receiver_ty = self.cx.typeck_results().expr_ty(receiver);

				if method == "clone" && args.is_empty() && is_account_view_ty(self.cx, expr_ty) {
					if let Some(span) = self.readonly_copy_source(receiver, tainted) {
						return Some(span);
					}
					if is_shared_ref_to_account_view(self.cx, receiver_ty) {
						return Some(expr.span.source_callsite());
					}
				}

				if matches!(method, "copied" | "cloned")
					&& args.is_empty()
					&& is_option_of_shared_account_view_ref(self.cx, receiver_ty)
				{
					return Some(expr.span.source_callsite());
				}

				if ty_contains_owned_account_view(self.cx, expr_ty) {
					return self.readonly_copy_source(receiver, tainted);
				}

				None
			}
			ExprKind::Call(callee, args) => {
				if args.len() == 1 && is_clone_callee(self.cx, callee) {
					let arg = &args[0];
					let arg_ty = self.cx.typeck_results().expr_ty(arg);
					if is_account_view_ty(self.cx, expr_ty) {
						if let Some(span) = self.readonly_copy_source(arg, tainted) {
							return Some(span);
						}
						if is_shared_ref_to_account_view(self.cx, arg_ty) {
							return Some(expr.span.source_callsite());
						}
					}
				}
				if args.len() == 1 && is_try_branch_callee(self.cx, callee) {
					return self.readonly_copy_source(&args[0], tainted);
				}
				None
			}
			ExprKind::Match(scrutinee, _, MatchSource::TryDesugar(_)) => {
				self.readonly_copy_source(scrutinee, tainted)
			}
			ExprKind::Block(block, _) => {
				block
					.expr
					.and_then(|tail| self.readonly_copy_source(tail, tainted))
			}
			ExprKind::If(_, then_branch, Some(else_branch)) => {
				self.readonly_copy_source(then_branch, tainted)
					.or_else(|| self.readonly_copy_source(else_branch, tainted))
			}
			ExprKind::DropTemps(inner) | ExprKind::Use(inner, _) | ExprKind::Type(inner, _) => {
				self.readonly_copy_source(inner, tainted)
			}
			_ => None,
		}
	}

	fn bind_pattern(
		&self,
		pat: &'tcx Pat<'tcx>,
		source: Option<Span>,
		init_expr: Option<&'tcx Expr<'tcx>>,
		tainted: &mut HashMap<HirId, Span>,
	) {
		match &pat.kind {
			PatKind::Binding(mode, binding, _, subpat) => {
				if let Some(copy_span) = source {
					let pat_ty = self.cx.typeck_results().pat_ty(pat);
					if matches!(mode.0, ByRef::Yes(_, Mutability::Mut))
						&& is_account_view_ty(self.cx, pat_ty.peel_refs())
					{
						emit_escalation(self.cx, pat.span, copy_span);
					} else if ty_contains_owned_account_view(self.cx, pat_ty) {
						tainted.insert(*binding, copy_span);
					}
				} else {
					tainted.remove(binding);
				}
				if let Some(sub) = subpat {
					self.bind_pattern(sub, source, init_expr, tainted);
				}
			}
			PatKind::Tuple(elements, _) => {
				if let Some(Expr {
					kind: ExprKind::Tup(exprs),
					..
				}) = init_expr
				{
					for (sub_pat, sub_expr) in elements.iter().zip(*exprs) {
						let sub_source = self.readonly_copy_source(sub_expr, tainted);
						self.bind_pattern(sub_pat, sub_source, Some(sub_expr), tainted);
					}
				} else {
					for sub_pat in *elements {
						self.bind_pattern(sub_pat, source, None, tainted);
					}
				}
			}
			PatKind::TupleStruct(_, elements, _) => {
				for sub_pat in *elements {
					self.bind_pattern(sub_pat, source, None, tainted);
				}
			}
			PatKind::Struct(_, fields, _) => {
				for field in *fields {
					self.bind_pattern(field.pat, source, None, tainted);
				}
			}
			PatKind::Ref(inner, ..) | PatKind::Deref(inner) => {
				self.bind_pattern(inner, source, None, tainted);
			}
			PatKind::Or(alternatives) => {
				for alt in *alternatives {
					self.bind_pattern(alt, source, init_expr, tainted);
				}
			}
			_ => {}
		}
	}

	fn visit_block(&self, block: &'tcx rustc_hir::Block<'tcx>, tainted: &mut HashMap<HirId, Span>) {
		for stmt in block.stmts {
			match &stmt.kind {
				StmtKind::Let(local) => {
					let source = local.init.and_then(|init| {
						self.visit_expr(init, tainted);
						self.readonly_copy_source(init, tainted)
					});
					self.bind_pattern(local.pat, source, local.init, tainted);
					if let Some(els) = local.els {
						let mut else_tainted = tainted.clone();
						self.visit_block(els, &mut else_tainted);
					}
				}
				StmtKind::Expr(expr) | StmtKind::Semi(expr) => {
					self.visit_expr(expr, tainted);
				}
				StmtKind::Item(_) => {}
			}
		}

		if let Some(expr) = block.expr {
			self.visit_expr(expr, tainted);
		}
	}

	fn visit_expr(&self, expr: &'tcx Expr<'tcx>, tainted: &mut HashMap<HirId, Span>) {
		match &expr.kind {
			ExprKind::AddrOf(_, Mutability::Mut, inner) => {
				self.visit_expr(inner, tainted);
				let inner_ty = self.cx.typeck_results().expr_ty(inner);
				if is_account_view_ty(self.cx, inner_ty)
					&& let Some(copy_span) = self.readonly_copy_source(inner, tainted)
				{
					emit_escalation(self.cx, expr.span, copy_span);
				}
			}
			ExprKind::MethodCall(_, receiver, args, _) => {
				self.visit_expr(receiver, tainted);
				let receiver_ty = self.cx.typeck_results().expr_ty(receiver);
				if is_account_view_ty(self.cx, receiver_ty)
					&& receiver_has_mut_autoref(self.cx, receiver)
					&& let Some(copy_span) = self.readonly_copy_source(receiver, tainted)
				{
					emit_escalation(self.cx, expr.span, copy_span);
				}
				for arg in *args {
					self.visit_expr(arg, tainted);
				}
			}
			ExprKind::Assign(lhs, rhs, _) => {
				self.visit_expr(rhs, tainted);
				let source = self.readonly_copy_source(rhs, tainted);
				if let Some(binding) = local_binding(lhs) {
					if let Some(copy_span) = source {
						tainted.insert(binding, copy_span);
					} else {
						tainted.remove(&binding);
					}
				} else {
					self.visit_expr(lhs, tainted);
				}
			}
			ExprKind::Block(block, _) => {
				self.visit_block(block, tainted);
			}
			ExprKind::If(cond, then_expr, else_expr) => {
				let before = tainted.clone();
				self.visit_expr(cond, tainted);
				let mut then_tainted = tainted.clone();
				self.visit_expr(then_expr, &mut then_tainted);
				let mut merged = then_tainted;
				if let Some(else_expr) = else_expr {
					let mut else_tainted = before;
					self.visit_expr(else_expr, &mut else_tainted);
					for (binding, span) in else_tainted {
						merged.entry(binding).or_insert(span);
					}
				} else {
					for (binding, span) in before {
						merged.entry(binding).or_insert(span);
					}
				}
				*tainted = merged;
			}
			ExprKind::Let(let_expr) => {
				self.visit_expr(let_expr.init, tainted);
				let source = self.readonly_copy_source(let_expr.init, tainted);
				self.bind_pattern(let_expr.pat, source, Some(let_expr.init), tainted);
			}
			ExprKind::Match(scrutinee, arms, _) => {
				self.visit_expr(scrutinee, tainted);
				let scrutinee_source = self.readonly_copy_source(scrutinee, tainted);
				for arm in *arms {
					let mut arm_tainted = tainted.clone();
					self.bind_pattern(arm.pat, scrutinee_source, Some(scrutinee), &mut arm_tainted);
					if let Some(guard) = arm.guard {
						self.visit_expr(guard, &mut arm_tainted);
					}
					self.visit_expr(arm.body, &mut arm_tainted);
				}
			}
			ExprKind::Loop(block, ..) => {
				let mut loop_tainted = tainted.clone();
				self.visit_block(block, &mut loop_tainted);
			}
			ExprKind::Closure(closure) => {
				let body = self.cx.tcx.hir_body(closure.body);
				let mut closure_tainted = tainted.clone();
				self.visit_expr(body.value, &mut closure_tainted);
			}
			ExprKind::Call(callee, args) => {
				self.visit_expr(callee, tainted);
				for arg in *args {
					self.visit_expr(arg, tainted);
				}
			}
			ExprKind::Binary(_, lhs, rhs) | ExprKind::AssignOp(_, lhs, rhs) => {
				self.visit_expr(lhs, tainted);
				self.visit_expr(rhs, tainted);
			}
			ExprKind::Unary(_, inner)
			| ExprKind::Cast(inner, _)
			| ExprKind::Type(inner, _)
			| ExprKind::DropTemps(inner)
			| ExprKind::Use(inner, _)
			| ExprKind::Field(inner, _)
			| ExprKind::AddrOf(_, Mutability::Not, inner)
			| ExprKind::Repeat(inner, _)
			| ExprKind::Yield(inner, _) => {
				self.visit_expr(inner, tainted);
			}
			ExprKind::Index(base, index, _) => {
				self.visit_expr(base, tainted);
				self.visit_expr(index, tainted);
			}
			ExprKind::Array(elements) | ExprKind::Tup(elements) => {
				for element in *elements {
					self.visit_expr(element, tainted);
				}
			}
			ExprKind::Struct(_, fields, base) => {
				for field in *fields {
					self.visit_expr(field.expr, tainted);
				}
				if let rustc_hir::StructTailExpr::Base(base_expr) = base {
					self.visit_expr(base_expr, tainted);
				}
			}
			ExprKind::Ret(Some(inner)) | ExprKind::Break(_, Some(inner)) => {
				self.visit_expr(inner, tainted);
			}
			_ => {}
		}
	}
}

impl<'tcx> LateLintPass<'tcx> for DenyReadonlyAccountCopyEscalation {
	fn check_fn(
		&mut self,
		cx: &LateContext<'tcx>,
		_: FnKind<'tcx>,
		_: &'tcx rustc_hir::FnDecl<'tcx>,
		body: &'tcx Body<'tcx>,
		_: Span,
		_: LocalDefId,
	) {
		let analyzer = Analyzer { cx };
		let mut tainted = HashMap::new();
		analyzer.visit_expr(body.value, &mut tainted);
	}
}
