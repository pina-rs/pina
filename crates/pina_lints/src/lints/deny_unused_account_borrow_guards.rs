extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashMap;
use std::collections::HashSet;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::Pat;
use rustc_hir::StmtKind;
use rustc_hir::intravisit::Visitor;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;
use rustc_span::Span;

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

/// Whether a pattern binds one of the runtime account-data guards re-exported
/// by Pinocchio and Pina.
///
/// Classifying the result type avoids two syntax-based failures: an unrelated
/// function named `load_pda` no longer looks like a guard, and consuming a
/// guard inside a wrapper expression no longer makes the wrapper result look
/// like one. Type aliases retain the underlying ADT definition, so Pina's
/// `LoadedAccount` aliases continue to work without special cases.
fn is_account_borrow_guard(cx: &LateContext<'_>, pattern: &Pat<'_>) -> bool {
	let ty = cx.typeck_results().pat_ty(pattern).peel_refs();
	let Some(definition) = ty.ty_adt_def() else {
		return false;
	};
	let definition = definition.did();

	cx.tcx.crate_name(definition.krate).as_str() == "solana_account_view"
		&& matches!(cx.tcx.item_name(definition).as_str(), "Ref" | "RefMut")
}

fn is_drop_callee(cx: &LateContext<'_>, callee: &Expr<'_>) -> bool {
	let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = &callee.kind else {
		return false;
	};

	// Resolve the callee definition: only `std::mem::drop` counts as disposal.
	// A local or imported function named `drop` is an ordinary use of its
	// argument, and removing it would delete the user's side effects.
	match path.res {
		rustc_hir::def::Res::Def(_, def_id) => {
			matches!(
				cx.tcx.def_path_str(def_id).as_str(),
				"std::mem::drop" | "core::mem::drop"
			)
		}
		_ => false,
	}
}

// Always inlined at its call sites; the out-of-line copy never runs, which
// would otherwise show up as a permanently uncovered signature line.
#[coverage(off)]
fn local_binding(expr: &Expr<'_>) -> Option<rustc_hir::hir_id::HirId> {
	match &expr.kind {
		ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
			match path.res {
				rustc_hir::def::Res::Local(binding) => Some(binding),
				_ => None,
			}
		}
		_ => None,
	}
}

/// One guard binding discovered in the function body.
struct GuardBinding {
	hir_id: rustc_hir::hir_id::HirId,
	name: String,
	span: Span,
	statement_span: Span,
	initializer_span: Option<Span>,
}

/// Finds every guard binding in a `let` pattern, including bindings nested in
/// tuples, structs, and `let ... else` patterns.
struct GuardPatternCollector<'cx, 'tcx, 'guards> {
	cx: &'cx LateContext<'tcx>,
	guards: &'guards mut Vec<GuardBinding>,
	root_pattern: rustc_hir::hir_id::HirId,
	statement_span: Span,
	initializer_span: Span,
}

impl<'tcx> Visitor<'tcx> for GuardPatternCollector<'_, 'tcx, '_> {
	fn visit_pat(&mut self, pattern: &'tcx Pat<'tcx>) {
		if let rustc_hir::PatKind::Binding(_, binding, ident, subpattern) = pattern.kind
			&& is_account_borrow_guard(self.cx, pattern)
		{
			let is_plain_root_binding = pattern.hir_id == self.root_pattern && subpattern.is_none();
			self.guards.push(GuardBinding {
				hir_id: binding,
				name: ident.name.to_string(),
				span: if is_plain_root_binding {
					self.initializer_span
				} else {
					pattern.span
				},
				statement_span: self.statement_span,
				initializer_span: is_plain_root_binding.then_some(self.initializer_span),
			});
		}

		rustc_hir::intravisit::walk_pat(self, pattern);
	}
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
	/// Number of `drop(local)` calls per binding, regardless of where the
	/// call appears.
	all_drops: HashMap<rustc_hir::hir_id::HirId, usize>,
	/// Spans of `drop(local)` statements per binding. Bindings whose only
	/// drops are statements can be fixed by removing those statements.
	statement_drops: HashMap<rustc_hir::hir_id::HirId, Vec<Span>>,
}

impl<'cx, 'tcx> Analyzer<'cx, 'tcx> {
	fn visit_arg_of_drop(&mut self, argument: &'tcx Expr<'tcx>) {
		// A bare local passed to `drop` releases the guard without reading it,
		// so it is not a use. Anything else still counts as a read.
		if let Some(binding) = local_binding(argument) {
			self.all_drops
				.entry(binding)
				.and_modify(|count| *count += 1)
				.or_insert(1);
		} else {
			self.visit_expr(argument);
		}
	}
}

impl<'tcx> Visitor<'tcx> for Analyzer<'_, 'tcx> {
	fn visit_stmt(&mut self, statement: &'tcx rustc_hir::Stmt<'tcx>) {
		if let StmtKind::Let(local) = statement.kind
			&& let Some(initializer) = local.init
		{
			let mut collector = GuardPatternCollector {
				cx: self.cx,
				guards: &mut self.guards,
				root_pattern: local.pat.hir_id,
				statement_span: local.span,
				initializer_span: initializer.span,
			};
			collector.visit_pat(local.pat);

			// A wildcard pattern does not bind or move a bare local. In
			// particular, `let _ = guard;` leaves an account borrow guard alive
			// until its original scope ends, so it must not count as a read or
			// disposal. Complex initializers such as `let _ = guard.value();`
			// still descend normally and count their actual reads.
			if matches!(local.pat.kind, rustc_hir::PatKind::Wild)
				&& local_binding(initializer).is_some()
			{
				return;
			}
		}

		if let StmtKind::Semi(expr) = statement.kind
			&& let ExprKind::Call(callee, args) = &expr.kind
			&& is_drop_callee(self.cx, callee)
			&& let Some(binding) = args.first().and_then(local_binding)
		{
			self.statement_drops
				.entry(binding)
				.or_default()
				.push(statement.span);
		}

		rustc_hir::intravisit::walk_stmt(self, statement);
	}

	fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
		if let ExprKind::Call(callee, args) = &expr.kind
			&& is_drop_callee(self.cx, callee)
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
			all_drops: HashMap::new(),
			statement_drops: HashMap::new(),
		};
		analyzer.visit_body(body);

		let source_map = cx.sess().source_map();

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

				let fix = suggestion::for_binding(
					source_map,
					&guard,
					analyzer.all_drops.get(&guard.hir_id).copied().unwrap_or(0),
					analyzer.statement_drops.get(&guard.hir_id),
				);
				if let Some(suggestion::Fix {
					replacement,
					applicability,
				}) = fix
				{
					match replacement {
						suggestion::Replacement::Single(span, snippet) => {
							diag.span_suggestion(
								span,
								"discard this guard immediately",
								snippet,
								applicability,
							);
						}
						suggestion::Replacement::Many(parts) => {
							diag.multipart_suggestion(
								"discard this guard immediately",
								parts,
								applicability,
							);
						}
					}
				}
			});
		}
	}
}

/// Machine-applicable rewrites for flagged guards.
///
/// Rewriting a guard binding into its initializer expression preserves the
/// validation and releases the borrow at the end of the statement. When the
/// only other occurrence of the binding is a `drop(local);` statement, that
/// statement is removed in the same suggestion so the rewritten source still
/// compiles.
mod suggestion {
	extern crate rustc_errors;
	extern crate rustc_span;

	use rustc_span::Span;

	use super::GuardBinding;

	pub enum Replacement {
		Single(Span, String),
		Many(Vec<(Span, String)>),
	}

	pub struct Fix {
		pub replacement: Replacement,
		pub applicability: rustc_errors::Applicability,
	}

	pub fn for_binding(
		source_map: &rustc_span::source_map::SourceMap,
		guard: &GuardBinding,
		all_drops: usize,
		statement_drops: Option<&Vec<Span>>,
	) -> Option<Fix> {
		// Suggestions must not reach into macro expansions; the spans there
		// belong to the macro definition rather than the call site.
		if guard.statement_span.from_expansion() {
			return None;
		}

		let initializer_span = guard.initializer_span?;
		let initializer = source_map.span_to_snippet(initializer_span).ok()? + ";";

		// No `drop(local)` calls: replacing the binding with the initializer
		// expression only releases the never-read borrow sooner, so the
		// rewrite is exact.
		if all_drops == 0 {
			return Some(Fix {
				replacement: Replacement::Single(guard.statement_span, initializer),
				applicability: rustc_errors::Applicability::MachineApplicable,
			});
		}

		let drop_spans = statement_drops.map_or(0, Vec::len);
		if all_drops > drop_spans {
			// A drop appears outside a statement (for example inside a closure
			// tail); removing the binding would break that code.
			return None;
		}
		let statement_drops = statement_drops
			.filter(|spans| !spans.is_empty())
			.expect("all drops are statements");

		// Removing a later `drop(local);` releases the borrow earlier than the
		// user wrote it. Code between the binding and the drop can observe the
		// held borrow (a later `try_borrow_mut` flips from panic to success),
		// so the rewrite is only a suggestion for the user to review.
		let mut replacements = vec![(guard.statement_span, initializer)];
		replacements.extend(statement_drops.iter().map(|span| (*span, String::new())));
		Some(Fix {
			replacement: Replacement::Many(replacements),
			applicability: rustc_errors::Applicability::MaybeIncorrect,
		})
	}
}
