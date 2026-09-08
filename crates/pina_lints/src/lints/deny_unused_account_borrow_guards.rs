extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashSet;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::StmtKind;
use rustc_hir::intravisit::Visitor;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;

crate::declare_late_lint! {
	/// ### What it does
	///
	/// Flags account borrow guards bound to locals that are never read.
	///
	/// ### Why is this bad?
	///
	/// The guard keeps the account data borrow alive from its binding to the
	/// end of the enclosing scope. Assertion-style validation that binds the
	/// guard and never reads it holds that borrow open for nothing, obscures
	/// the borrow boundary, and can turn a later borrow of the same account
	/// data into a runtime panic.
	pub DENY_UNUSED_ACCOUNT_BORROW_GUARDS,
	Warn,
	"account borrow guards bound to locals must be read or discarded immediately"
}

/// Definition-path fragments that confirm a guard-named method call resolves
/// to Pina's account-view or token-view APIs.
const GUARD_PATH_FRAGMENTS: &[&str] = &[
	"accountview::try_borrow",
	"asaccount::as_account",
	"astokenaccount::as_token",
	"astokenaccount::as_associated_token_account",
];

fn method_def_path(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
	cx.typeck_results()
		.type_dependent_def_id(expr.hir_id)
		.map(|def_id| cx.tcx.def_path_str(def_id))
}

fn is_guard_method_name(method: &str) -> bool {
	matches!(
		method,
		"try_borrow" | "try_borrow_mut" | "as_account" | "as_account_mut"
	) || method.starts_with("as_token_")
		|| method.starts_with("as_associated_token_account")
}

fn is_guard_method(cx: &LateContext<'_>, expr: &Expr<'_>, method: &str) -> bool {
	if !is_guard_method_name(method) {
		return false;
	}

	method_def_path(cx, expr).is_some_and(|path| {
		let path = path.to_ascii_lowercase();
		GUARD_PATH_FRAGMENTS
			.iter()
			.any(|fragment| path.contains(fragment))
	})
}

fn is_generated_pda_guard(callee: &Expr<'_>) -> bool {
	let ExprKind::Path(path) = &callee.kind else {
		return false;
	};

	let method = match path {
		rustc_hir::QPath::Resolved(_, path) => path.segments.last().map(|segment| segment.ident),
		rustc_hir::QPath::TypeRelative(_, segment) => Some(segment.ident),
	};

	method.is_some_and(|method| matches!(method.name.as_str(), "load_pda" | "load_pda_mut"))
}

fn contains_guard_construction(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
	match &expr.kind {
		ExprKind::MethodCall(segment, receiver, args, _) => {
			is_guard_method(cx, expr, segment.ident.name.as_str())
				|| contains_guard_construction(cx, receiver)
				|| args
					.iter()
					.any(|argument| contains_guard_construction(cx, argument))
		}
		ExprKind::Match(scrutinee, ..)
		| ExprKind::DropTemps(scrutinee)
		| ExprKind::Use(scrutinee, _)
		| ExprKind::Type(scrutinee, _)
		| ExprKind::UnsafeBinderCast(_, scrutinee, _) => contains_guard_construction(cx, scrutinee),
		ExprKind::Block(block, _) => {
			block
				.expr
				.is_some_and(|tail| contains_guard_construction(cx, tail))
		}
		ExprKind::Call(callee, args) => {
			is_generated_pda_guard(callee)
				|| contains_guard_construction(cx, callee)
				|| args
					.iter()
					.any(|argument| contains_guard_construction(cx, argument))
		}
		_ => false,
	}
}

fn is_drop_callee(expr: &Expr<'_>) -> bool {
	let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = &expr.kind else {
		return false;
	};

	path.segments
		.last()
		.is_some_and(|segment| segment.ident.name.as_str() == "drop")
}

fn local_binding(expr: &Expr<'_>) -> Option<rustc_hir::hir_id::HirId> {
	let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = &expr.kind else {
		return None;
	};
	let rustc_hir::def::Res::Local(binding) = path.res else {
		return None;
	};

	Some(binding)
}

/// One guard binding discovered in the function body.
struct GuardBinding {
	hir_id: rustc_hir::hir_id::HirId,
	name: String,
	span: rustc_span::Span,
}

/// Collects guard bindings and local references in one pass over the body.
///
/// Reads are any resolved local path outside a direct `drop(local)` call, so
/// guards read through method chains, field access, `&` borrows, closures,
/// and block tails all count as used.
struct Analyzer<'cx, 'tcx> {
	cx: &'cx LateContext<'tcx>,
	guards: Vec<GuardBinding>,
	referenced: HashSet<rustc_hir::hir_id::HirId>,
}

impl<'cx, 'tcx> Analyzer<'cx, 'tcx> {
	fn visit_arg_of_drop(&mut self, argument: &'tcx Expr<'tcx>) {
		// A bare local passed to `drop` releases the guard without reading it,
		// so it is not a use. Anything else still counts as a read.
		if local_binding(argument).is_none() {
			self.visit_expr(argument);
		}
	}
}

impl<'tcx> Visitor<'tcx> for Analyzer<'_, 'tcx> {
	fn visit_stmt(&mut self, statement: &'tcx rustc_hir::Stmt<'tcx>) {
		if let StmtKind::Let(local) = statement.kind
			&& let Some(initializer) = local.init
			&& contains_guard_construction(self.cx, initializer)
			&& let rustc_hir::PatKind::Binding(_, binding, ident, ..) = local.pat.kind
		{
			self.guards.push(GuardBinding {
				hir_id: binding,
				name: ident.name.to_string(),
				span: initializer.span,
			});
		}

		rustc_hir::intravisit::walk_stmt(self, statement);
	}

	fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
		if let ExprKind::Call(callee, args) = &expr.kind
			&& is_drop_callee(callee)
		{
			// A bare local passed to `drop` releases the guard without reading it,
			// so it is neither a use nor worth descending into.
			self.visit_expr(callee);
			for argument in *args {
				self.visit_arg_of_drop(argument);
			}
			return;
		}
		if let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = &expr.kind
			&& let rustc_hir::def::Res::Local(binding) = path.res
		{
			self.referenced.insert(binding);
		}

		if let ExprKind::Closure(closure) = &expr.kind {
			// Nested bodies are not visited by default; closure captures and
			// reads count as guard uses.
			self.visit_body(self.cx.tcx.hir_body(closure.body));
			return;
		}

		rustc_hir::intravisit::walk_expr(self, expr);
	}
}

impl<'tcx> LateLintPass<'tcx> for DenyUnusedAccountBorrowGuards {
	fn check_fn(
		&mut self,
		cx: &LateContext<'tcx>,
		_: rustc_hir::intravisit::FnKind<'tcx>,
		_: &'tcx rustc_hir::FnDecl<'tcx>,
		body: &'tcx rustc_hir::Body<'tcx>,
		_: rustc_span::Span,
		_: rustc_hir::def_id::LocalDefId,
	) {
		let mut analyzer = Analyzer {
			cx,
			guards: Vec::new(),
			referenced: HashSet::new(),
		};
		analyzer.visit_body(body);

		for guard in analyzer.guards {
			if analyzer.referenced.contains(&guard.hir_id) {
				continue;
			}

			cx.lint(DENY_UNUSED_ACCOUNT_BORROW_GUARDS, |diag| {
				diag.span(guard.span);
				diag.primary_message(format!(
					"account borrow guard `{}` is never read",
					guard.name
				));
				diag.help(
					"the guard holds the account data borrow until the end of its scope; discard \
					 the validation value immediately by calling it as a `?` statement, binding \
					 it with `let _ = ...`, or reading it before the scope ends",
				);
			});
		}
	}
}
