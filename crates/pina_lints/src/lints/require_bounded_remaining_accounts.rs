extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashSet;

use rustc_hir::BinOpKind;
use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::def::DefKind;
use rustc_hir::def::Res;
use rustc_hir::intravisit::FnKind;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;

use crate::shared;

crate::declare_late_lint! {
	/// ### What it does
	///
	/// Rejects loops over remaining accounts unless the iterator has an explicit
	/// `.take(MAX)` bound or a dominating constant-bound length check rejects
	/// oversized input first.
	///
	/// ### Why is this bad?
	///
	/// Caller-controlled account counts can turn linear per-account work into
	/// compute exhaustion. A visible protocol bound makes the cost auditable.
	pub REQUIRE_BOUNDED_REMAINING_ACCOUNTS,
	Deny,
	"remaining-account loops require an explicit maximum"
}

fn is_constant_bound(expr: &Expr<'_>) -> bool {
	match &expr.kind {
		ExprKind::Lit(_) => true,
		ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
			matches!(path.res, Res::Def(DefKind::Const | DefKind::AssocConst, _))
		}
		ExprKind::Unary(_, inner)
		| ExprKind::Cast(inner, _)
		| ExprKind::DropTemps(inner)
		| ExprKind::AddrOf(_, _, inner) => is_constant_bound(inner),
		_ => false,
	}
}

fn remaining_len_identity(expr: &Expr<'_>) -> Option<String> {
	let ExprKind::MethodCall(segment, receiver, arguments, _) = &expr.kind else {
		return None;
	};
	if segment.ident.name.as_str() != "len" || !arguments.is_empty() {
		return None;
	}

	let identity = shared::expression_identity(receiver)?;
	identity
		.to_ascii_lowercase()
		.contains("remaining")
		.then_some(identity)
}

fn bounded_identity(condition: &Expr<'_>) -> Option<String> {
	let ExprKind::Binary(operation, left, right) = &condition.kind else {
		return None;
	};

	match operation.node {
		BinOpKind::Gt | BinOpKind::Ge if is_constant_bound(right) => remaining_len_identity(left),
		BinOpKind::Lt | BinOpKind::Le if is_constant_bound(left) => remaining_len_identity(right),
		_ => None,
	}
}

fn expression_returns(expr: &Expr<'_>) -> bool {
	match &expr.kind {
		ExprKind::Ret(_) => true,
		ExprKind::Block(block, _) => {
			block.expr.is_some_and(expression_returns)
				|| block.stmts.last().is_some_and(|statement| {
					match &statement.kind {
						rustc_hir::StmtKind::Expr(expr) | rustc_hir::StmtKind::Semi(expr) => {
							expression_returns(expr)
						}
						_ => false,
					}
				})
		}
		ExprKind::DropTemps(inner) => expression_returns(inner),
		_ => false,
	}
}

fn header_contains_identity(header: &str, identity: &str) -> bool {
	header.match_indices(identity).any(|(start, matched)| {
		let before = header[..start].chars().next_back();
		let after = header[start + matched.len()..].chars().next();
		let is_boundary = |character: Option<char>| {
			character.is_none_or(|character| !character.is_ascii_alphanumeric() && character != '_')
		};

		is_boundary(before) && is_boundary(after)
	})
}

struct Analyzer<'cx, 'tcx> {
	cx: &'cx LateContext<'tcx>,
}

impl<'tcx> Analyzer<'_, 'tcx> {
	fn visit_block(&self, block: &'tcx rustc_hir::Block<'tcx>, bounded: &mut HashSet<String>) {
		for statement in block.stmts {
			match &statement.kind {
				rustc_hir::StmtKind::Let(local) => {
					if let Some(initializer) = local.init {
						self.visit_expr(initializer, bounded);
					}
				}
				rustc_hir::StmtKind::Expr(expr) | rustc_hir::StmtKind::Semi(expr) => {
					self.visit_expr(expr, bounded);
				}
				_ => {}
			}
		}
		if let Some(expr) = block.expr {
			self.visit_expr(expr, bounded);
		}
	}

	fn visit_expr(&self, expr: &'tcx Expr<'tcx>, bounded: &mut HashSet<String>) {
		match &expr.kind {
			ExprKind::Loop(block, ..) => {
				let snippet = self
					.cx
					.sess()
					.source_map()
					.span_to_snippet(expr.span)
					.unwrap_or_default();
				let loop_header = snippet
					.split_once('{')
					.map_or(snippet.as_str(), |(header, _)| header);
				let mentions_remaining = loop_header.to_ascii_lowercase().contains("remaining");
				let has_validated_bound = bounded
					.iter()
					.any(|identity| header_contains_identity(loop_header, identity));
				if mentions_remaining && !loop_header.contains(".take(") && !has_validated_bound {
					self.cx.lint(REQUIRE_BOUNDED_REMAINING_ACCOUNTS, |diag| {
						diag.span(expr.span);
						diag.primary_message(
							"remaining accounts are processed without an explicit bound",
						);
						diag.help(
							"reject `remaining.len() > MAX_REMAINING_ACCOUNTS` before the loop, \
							 or iterate with `remaining.iter().take(MAX_REMAINING_ACCOUNTS)`",
						);
					});
				}

				let mut nested = bounded.clone();
				self.visit_block(block, &mut nested);
			}
			ExprKind::If(condition, then, otherwise) => {
				self.visit_expr(condition, bounded);
				let mut branch = bounded.clone();
				self.visit_expr(then, &mut branch);
				if let Some(otherwise) = otherwise {
					let mut branch = bounded.clone();
					self.visit_expr(otherwise, &mut branch);
				} else if expression_returns(then)
					&& let Some(identity) = bounded_identity(condition)
				{
					bounded.insert(identity);
				}
			}
			ExprKind::MethodCall(_, receiver, arguments, _) => {
				self.visit_expr(receiver, bounded);
				for argument in *arguments {
					self.visit_expr(argument, bounded);
				}
			}
			ExprKind::Call(callee, arguments) => {
				self.visit_expr(callee, bounded);
				for argument in *arguments {
					self.visit_expr(argument, bounded);
				}
			}
			ExprKind::Block(block, _) => {
				let mut nested = bounded.clone();
				self.visit_block(block, &mut nested);
			}
			ExprKind::Match(scrutinee, arms, _) => {
				self.visit_expr(scrutinee, bounded);
				for arm in *arms {
					let mut branch = bounded.clone();
					self.visit_expr(arm.body, &mut branch);
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
			| ExprKind::UnsafeBinderCast(_, inner, _) => self.visit_expr(inner, bounded),
			ExprKind::Binary(_, left, right)
			| ExprKind::Assign(left, right, _)
			| ExprKind::AssignOp(_, left, right) => {
				self.visit_expr(left, bounded);
				self.visit_expr(right, bounded);
			}
			ExprKind::Index(base, index, _) => {
				self.visit_expr(base, bounded);
				self.visit_expr(index, bounded);
			}
			ExprKind::Let(let_expr) => self.visit_expr(let_expr.init, bounded),
			ExprKind::Tup(expressions) | ExprKind::Array(expressions) => {
				for expression in *expressions {
					self.visit_expr(expression, bounded);
				}
			}
			ExprKind::Struct(_, fields, tail) => {
				for field in *fields {
					self.visit_expr(field.expr, bounded);
				}
				if let rustc_hir::StructTailExpr::Base(base) = tail {
					self.visit_expr(base, bounded);
				}
			}
			ExprKind::Ret(Some(inner)) | ExprKind::Break(_, Some(inner)) => {
				self.visit_expr(inner, bounded);
			}
			_ => {}
		}
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
		Analyzer { cx }.visit_expr(body.value, &mut HashSet::new());
	}
}
