extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashSet;

use rustc_hir::BinOpKind;
use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::LangItem;
use rustc_hir::LoopSource;
use rustc_hir::MatchSource;
use rustc_hir::Node;
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

fn is_iterator_method(cx: &LateContext<'_>, expr: &Expr<'_>, expected: &str) -> bool {
	cx.typeck_results()
		.type_dependent_def_id(expr.hir_id)
		.is_some_and(|method| {
			cx.tcx.item_name(method).as_str() == expected
				&& cx
					.tcx
					.trait_of_assoc(method)
					.is_some_and(|trait_id| cx.tcx.is_lang_item(trait_id, LangItem::Iterator))
		})
}

fn is_array_iteration_method(cx: &LateContext<'_>, expr: &Expr<'_>, expected: &str) -> bool {
	cx.typeck_results()
		.type_dependent_def_id(expr.hir_id)
		.is_some_and(|method| {
			if expected == "iter" {
				cx.tcx.crate_name(method.krate).as_str() == "core"
					&& cx.tcx.item_name(method).as_str() == "iter"
			} else {
				debug_assert_eq!(expected, "into_iter");
				cx.tcx.is_lang_item(method, LangItem::IntoIterIntoIter)
			}
		})
}

fn expression_has_static_bound(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
	match &expr.kind {
		ExprKind::MethodCall(segment, receiver, arguments, _) => {
			let method = segment.ident.name.as_str();
			if method == "take" {
				return arguments.len() == 1
					&& is_constant_bound(&arguments[0])
					&& is_iterator_method(cx, expr, "take");
			}
			if method == "chain" {
				return arguments.len() == 1
					&& is_iterator_method(cx, expr, "chain")
					&& expression_has_static_bound(cx, receiver)
					&& expression_has_static_bound(cx, &arguments[0]);
			}

			matches!(method, "iter" | "into_iter")
				&& arguments.is_empty()
				&& is_array_iteration_method(cx, expr, method)
				&& matches!(receiver.kind, ExprKind::Array(_) | ExprKind::Repeat(_, _))
		}
		_ => false,
	}
}

// The UI tests exercise this compiler-generated HIR adapter end to end, but
// LLVM maps its structural pattern fields to synthetic, unreachable regions.
#[coverage(off)]
fn for_loop_has_constant_take(cx: &LateContext<'_>, loop_expr: &Expr<'_>) -> bool {
	cx.tcx
		.hir_parent_iter(loop_expr.hir_id)
		.find_map(|(_, node)| {
			let Node::Expr(Expr {
				kind:
					ExprKind::Match(
						Expr {
							kind: ExprKind::Call(_, [iterator]),
							..
						},
						_,
						MatchSource::ForLoopDesugar,
					),
				..
			}) = node
			else {
				return None;
			};

			Some(iterator)
		})
		.is_some_and(|iterator| expression_has_static_bound(cx, iterator))
}

fn len_identity(expr: &Expr<'_>) -> Option<String> {
	let ExprKind::MethodCall(segment, receiver, arguments, _) = &expr.kind else {
		return None;
	};
	if segment.ident.name.as_str() != "len" || !arguments.is_empty() {
		return None;
	}

	shared::expression_identity(receiver)
}

fn bounded_identity(condition: &Expr<'_>) -> Option<String> {
	let ExprKind::Binary(operation, left, right) = &condition.kind else {
		return None;
	};

	match operation.node {
		BinOpKind::Gt | BinOpKind::Ge if is_constant_bound(right) => len_identity(left),
		BinOpKind::Lt | BinOpKind::Le if is_constant_bound(left) => len_identity(right),
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

#[derive(Clone, Default)]
struct AnalysisState {
	bounded: HashSet<String>,
	remaining: HashSet<String>,
}

fn same_or_descendant(candidate: &str, identity: &str) -> bool {
	candidate == identity
		|| candidate
			.strip_prefix(identity)
			.is_some_and(|suffix| suffix.starts_with(['.', '[']))
}

fn intersect_states(states: &[AnalysisState]) -> AnalysisState {
	let Some(first) = states.first() else {
		return AnalysisState::default();
	};
	let mut bounded = first.bounded.clone();
	let mut remaining = HashSet::new();

	for state in states {
		bounded.retain(|identity| state.bounded.contains(identity));
		remaining.extend(state.remaining.iter().cloned());
	}

	AnalysisState { bounded, remaining }
}

struct Analyzer<'cx, 'tcx> {
	cx: &'cx LateContext<'tcx>,
}

impl<'tcx> Analyzer<'_, 'tcx> {
	fn is_remaining_identity(&self, identity: &str, state: &AnalysisState) -> bool {
		state.remaining.contains(identity) || identity.to_ascii_lowercase().contains("remaining")
	}

	fn expression_identity(&self, expression: &Expr<'_>) -> Option<String> {
		shared::expression_identity(expression)
	}

	fn expression_is_remaining(&self, expression: &Expr<'_>, state: &AnalysisState) -> bool {
		self.expression_identity(expression)
			.is_some_and(|identity| self.is_remaining_identity(&identity, state))
	}

	fn invalidate(&self, state: &mut AnalysisState, identity: &str) {
		state
			.bounded
			.retain(|candidate| !same_or_descendant(candidate, identity));
	}

	fn method_mutably_borrows_receiver(&self, expression: &Expr<'_>) -> bool {
		let Some(definition) = self
			.cx
			.typeck_results()
			.type_dependent_def_id(expression.hir_id)
		else {
			return false;
		};

		self.cx
			.tcx
			.fn_sig(definition)
			.instantiate_identity()
			.skip_binder()
			.inputs()
			.first()
			.and_then(|receiver| receiver.ref_mutability())
			== Some(rustc_hir::Mutability::Mut)
	}

	fn invalidate_mutable_reference_argument(
		&self,
		state: &mut AnalysisState,
		argument: &Expr<'_>,
	) {
		if self
			.cx
			.typeck_results()
			.expr_ty_adjusted(argument)
			.ref_mutability()
			== Some(rustc_hir::Mutability::Mut)
			&& let Some(identity) = self.expression_identity(argument)
		{
			self.invalidate(state, &identity);
		}
	}

	fn loop_mentions_remaining(&self, snippet: &str, state: &AnalysisState) -> bool {
		let header = snippet
			.split_once('{')
			.map_or(snippet, |(header, _)| header);

		state
			.remaining
			.iter()
			.any(|identity| header_contains_identity(header, identity))
			|| header.to_ascii_lowercase().contains("remaining")
	}

	fn visit_block(&self, block: &'tcx rustc_hir::Block<'tcx>, state: &mut AnalysisState) {
		for statement in block.stmts {
			match &statement.kind {
				rustc_hir::StmtKind::Let(local) => {
					if let Some(initializer) = local.init {
						let inherits_remaining = self.expression_is_remaining(initializer, state);
						let inherits_bound = self
							.expression_identity(initializer)
							.is_some_and(|identity| state.bounded.contains(&identity))
							|| expression_has_static_bound(self.cx, initializer);
						self.visit_expr(initializer, state);

						if let rustc_hir::PatKind::Binding(_, _, identifier, None) = local.pat.kind
						{
							let identity = identifier.as_str().to_owned();
							if inherits_remaining {
								state.remaining.insert(identity.clone());
							}
							if inherits_bound {
								state.bounded.insert(identity);
							}
						}
					}
				}
				rustc_hir::StmtKind::Expr(expr) | rustc_hir::StmtKind::Semi(expr) => {
					self.visit_expr(expr, state);
				}
				_ => {}
			}
		}
		if let Some(expr) = block.expr {
			self.visit_expr(expr, state);
		}
	}

	fn visit_expr(&self, expr: &'tcx Expr<'tcx>, state: &mut AnalysisState) {
		match &expr.kind {
			ExprKind::Loop(block, _, source, _) => {
				let snippet = self
					.cx
					.sess()
					.source_map()
					.span_to_snippet(expr.span)
					.unwrap_or_default();
				let loop_header = snippet
					.split_once('{')
					.map_or(snippet.as_str(), |(header, _)| header);
				let mentions_remaining = self.loop_mentions_remaining(&snippet, state);
				let has_validated_bound = state
					.bounded
					.iter()
					.any(|identity| header_contains_identity(loop_header, identity));
				let has_constant_take = matches!(source, LoopSource::ForLoop)
					&& for_loop_has_constant_take(self.cx, expr);

				if mentions_remaining && !has_constant_take && !has_validated_bound {
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

				let entry = state.clone();
				let mut body_state = entry.clone();
				self.visit_block(block, &mut body_state);
				*state = intersect_states(&[entry, body_state]);
			}
			ExprKind::If(condition, then, otherwise) => {
				self.visit_expr(condition, state);
				let base = state.clone();
				let mut then_state = base.clone();
				self.visit_expr(then, &mut then_state);

				if let Some(otherwise) = otherwise {
					let mut branches = Vec::with_capacity(2);
					if !expression_returns(then) {
						branches.push(then_state);
					}
					let mut otherwise_state = base;
					self.visit_expr(otherwise, &mut otherwise_state);
					if !expression_returns(otherwise) {
						branches.push(otherwise_state);
					}
					if !branches.is_empty() {
						*state = intersect_states(&branches);
					}
				} else if expression_returns(then)
					&& let Some(identity) = bounded_identity(condition)
					&& self.is_remaining_identity(&identity, &base)
				{
					*state = base;
					state.bounded.insert(identity);
				} else {
					*state = intersect_states(&[base, then_state]);
				}
			}
			ExprKind::MethodCall(_, receiver, arguments, _) => {
				self.visit_expr(receiver, state);
				for argument in *arguments {
					self.visit_expr(argument, state);
					self.invalidate_mutable_reference_argument(state, argument);
				}
				if self.method_mutably_borrows_receiver(expr)
					&& let Some(identity) = self.expression_identity(receiver)
				{
					self.invalidate(state, &identity);
				}
			}
			ExprKind::Call(callee, arguments) => {
				self.visit_expr(callee, state);
				for argument in *arguments {
					self.visit_expr(argument, state);
					self.invalidate_mutable_reference_argument(state, argument);
				}
			}
			ExprKind::Block(block, _) => self.visit_block(block, state),
			ExprKind::Match(scrutinee, arms, _) => {
				self.visit_expr(scrutinee, state);
				let base = state.clone();
				let mut branches = Vec::with_capacity(arms.len());
				for arm in *arms {
					let mut branch = base.clone();
					if let Some(guard) = arm.guard {
						self.visit_expr(guard, &mut branch);
					}
					self.visit_expr(arm.body, &mut branch);
					if !expression_returns(arm.body) {
						branches.push(branch);
					}
				}
				if !branches.is_empty() {
					*state = intersect_states(&branches);
				}
			}
			ExprKind::Closure(closure) => {
				let entry = state.clone();
				let mut body_state = entry.clone();
				let body = self.cx.tcx.hir_body(closure.body);
				self.visit_expr(body.value, &mut body_state);
				*state = intersect_states(&[entry, body_state]);
			}
			ExprKind::AddrOf(_, rustc_hir::Mutability::Mut, inner) => {
				self.visit_expr(inner, state);
				if let Some(identity) = self.expression_identity(inner) {
					self.invalidate(state, &identity);
				}
			}
			ExprKind::Unary(_, inner)
			| ExprKind::Use(inner, _)
			| ExprKind::Cast(inner, _)
			| ExprKind::Type(inner, _)
			| ExprKind::DropTemps(inner)
			| ExprKind::AddrOf(_, rustc_hir::Mutability::Not, inner)
			| ExprKind::Field(inner, _)
			| ExprKind::Repeat(inner, _)
			| ExprKind::Yield(inner, _)
			| ExprKind::Become(inner)
			| ExprKind::UnsafeBinderCast(_, inner, _) => self.visit_expr(inner, state),
			ExprKind::Binary(operation, left, right) => {
				self.visit_expr(left, state);
				if matches!(operation.node, BinOpKind::And | BinOpKind::Or) {
					let base = state.clone();
					let mut right_state = base.clone();
					self.visit_expr(right, &mut right_state);
					*state = intersect_states(&[base, right_state]);
				} else {
					self.visit_expr(right, state);
				}
			}
			ExprKind::Assign(left, right, _) => {
				let right_is_remaining = self.expression_is_remaining(right, state);
				let right_is_bounded = self
					.expression_identity(right)
					.is_some_and(|identity| state.bounded.contains(&identity))
					|| expression_has_static_bound(self.cx, right);
				self.visit_expr(left, state);
				self.visit_expr(right, state);
				if let Some(identity) = self.expression_identity(left) {
					self.invalidate(state, &identity);
					if right_is_remaining {
						state.remaining.insert(identity.clone());
					}
					if right_is_bounded {
						state.bounded.insert(identity);
					}
				}
			}
			ExprKind::AssignOp(_, left, right) => {
				self.visit_expr(left, state);
				self.visit_expr(right, state);
				if let Some(identity) = self.expression_identity(left) {
					self.invalidate(state, &identity);
				}
			}
			ExprKind::Index(base, index, _) => {
				self.visit_expr(base, state);
				self.visit_expr(index, state);
			}
			ExprKind::Let(let_expr) => self.visit_expr(let_expr.init, state),
			ExprKind::Tup(expressions) | ExprKind::Array(expressions) => {
				for expression in *expressions {
					self.visit_expr(expression, state);
				}
			}
			ExprKind::Struct(_, fields, tail) => {
				for field in *fields {
					self.visit_expr(field.expr, state);
				}
				if let rustc_hir::StructTailExpr::Base(base) = tail {
					self.visit_expr(base, state);
				}
			}
			ExprKind::Ret(Some(inner)) | ExprKind::Break(_, Some(inner)) => {
				self.visit_expr(inner, state);
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
		let mut state = AnalysisState::default();
		for parameter in body.params {
			if let rustc_hir::PatKind::Binding(_, _, identifier, None) = parameter.pat.kind {
				let identity = identifier.as_str().to_owned();
				if identity.to_ascii_lowercase().contains("remaining") {
					state.remaining.insert(identity);
				}
			}
		}

		Analyzer { cx }.visit_expr(body.value, &mut state);
	}
}
