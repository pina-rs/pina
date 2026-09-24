extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashSet;

use rustc_ast::LitKind;
use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::HirId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::intravisit::Visitor;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_span::Span;

use crate::diagnostics;
use crate::shared;

crate::declare_late_lint! {
	/// ### What it does
	///
	/// Warns when an instruction path closes an account with `close()` or
	/// `close_with_recipient()` without first zeroing that same account's data.
	///
	/// ### Why is this bad?
	///
	/// Closing an account without clearing its bytes first can leave stale data
	/// readable during the same transaction window.
	///
	/// ### Example
	///
	/// ```ignore
	/// // Preferred: zero the data and close in one operation.
	/// state.close_account_zeroed(&ID, recipient)?;
	///
	/// // When the close must stay separate, clear the whole buffer first.
	/// state.try_borrow_mut()?.fill(0);
	/// state.close_with_recipient(&ID, recipient)?;
	/// ```
	///
	/// The zeroing proof is a `fill(0)` over the entire buffer returned by
	/// `try_borrow_mut()?`, either chained directly or through a `let` binding
	/// of that buffer. Receivers resolve through the shared fact collector's
	/// alias chains, so zeroing through `let a = &mut state;` proves `state`,
	/// while zeroing a different account proves nothing. A partial fill, a
	/// non-zero fill, or a binding reassigned after its `let` is not a proof.
	/// `close_account_zeroed()` and the `CloseAccountZeroed` builder zero before
	/// closing, so they are never flagged.
	pub REQUIRE_ZEROED_BEFORE_CLOSE,
	Deny,
	"account close should be preceded by zeroing the same account's data"
}

const TARGET_METHODS: &[&str] = &["close_with_recipient", "close"];
const TARGET_NEEDLES: &[&str] = &["process", "process_instruction", "instruction"];

impl<'tcx> LateLintPass<'tcx> for RequireZeroedBeforeClose {
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
			|| !shared::def_path_matches(&def_path, TARGET_NEEDLES)
		{
			return;
		}

		let facts = shared::collect_function_facts(cx, body);
		let zeroing = ZeroingFacts::collect(cx, body);
		for (index, call) in facts.calls.iter().enumerate() {
			if !TARGET_METHODS.contains(&call.method.as_str()) {
				continue;
			}

			let closed = account_key(&facts, call, &zeroing.reassigned);
			let is_zeroed = closed.is_some()
				&& facts.calls[..index].iter().any(|prior| {
					zeroing.zero_fills.contains(&prior.span)
						&& account_key(&facts, prior, &zeroing.reassigned) == closed
				});
			if is_zeroed {
				continue;
			}

			diagnostics::emit(cx, REQUIRE_ZEROED_BEFORE_CLOSE, |diag| {
				diag.span(call.span);
				diag.primary_message(
					"account close should be preceded by zeroing the same account's data",
				);
				diag.help(
					"close with `account.close_account_zeroed(&ID, recipient)?` (or the \
					 `CloseAccountZeroed` builder), which zeroes the data and closes in one step",
				);
				diag.help(
					"to keep a separate close, first clear the whole data buffer of the same \
					 account with `account.try_borrow_mut()?.fill(0);`",
				);
				diag.help(shared::CONTROL_FLOW_LIMITATION_HELP);
			});
		}
	}
}

/// The account a method receiver names, after resolving `let` aliases.
#[derive(Debug, PartialEq, Eq)]
enum AccountKey<'facts> {
	/// The receiver resolves to this root local binding, such as a parameter.
	Local(HirId),
	/// The receiver resolves to a place that is not a plain local binding,
	/// such as the field `self.escrow`.
	Place(&'facts str),
}

/// Resolve `call`'s receiver through the fact collector's alias chains.
///
/// `let a = &mut state;` records `a` as an alias of `state`, so a receiver of
/// `a` and a receiver of `state` resolve to the same key. Returns `None` when
/// the receiver is not trackable or the chain passes through a binding that is
/// reassigned after its `let`: the recorded alias describes the value at
/// binding time, so it cannot prove anything about the value at the call.
fn account_key<'facts>(
	facts: &'facts shared::FunctionFacts,
	call: &'facts shared::CallInfo,
	reassigned: &HashSet<HirId>,
) -> Option<AccountKey<'facts>> {
	let mut identity = call.receiver.as_deref()?;
	let mut binding = call.receiver_binding;
	let mut visited = HashSet::new();
	while let Some(current) = binding {
		if reassigned.contains(&current) || !visited.insert(current) {
			return None;
		}

		let Some(alias) = facts.aliases.get(&current) else {
			return Some(AccountKey::Local(current));
		};
		identity = &alias.identity;
		binding = alias.binding;
	}

	Some(AccountKey::Place(identity))
}

/// Lint-specific facts gathered in one walk over the function body.
#[derive(Default)]
struct ZeroingFacts {
	/// Spans of `fill(0)` calls that zero an account's entire data buffer.
	zero_fills: HashSet<Span>,
	/// Local bindings assigned after their `let`.
	reassigned: HashSet<HirId>,
}

impl ZeroingFacts {
	fn collect<'tcx>(cx: &LateContext<'tcx>, body: &'tcx rustc_hir::Body<'tcx>) -> Self {
		let mut collector = ZeroingCollector {
			cx,
			borrowed_buffers: HashSet::new(),
			buffer_fills: Vec::new(),
			facts: Self::default(),
		};
		collector.visit_body(body);

		let ZeroingCollector {
			borrowed_buffers,
			buffer_fills,
			mut facts,
			..
		} = collector;
		// A binding's `let` is visited before its uses, but the filter runs
		// after the walk to keep the check independent of visit order. A buffer
		// reassigned after its `let` is rejected later, when `account_key`
		// refuses to resolve the fill's receiver through it.
		facts.zero_fills.extend(
			buffer_fills
				.into_iter()
				.filter(|(binding, _)| borrowed_buffers.contains(binding))
				.map(|(_, span)| span),
		);
		facts
	}
}

struct ZeroingCollector<'cx, 'tcx> {
	cx: &'cx LateContext<'tcx>,
	/// Bindings initialized with an account's `try_borrow_mut()?` buffer.
	borrowed_buffers: HashSet<HirId>,
	/// `fill(0)` calls on a local binding, pending the buffer check.
	buffer_fills: Vec<(HirId, Span)>,
	facts: ZeroingFacts,
}

impl<'tcx> Visitor<'tcx> for ZeroingCollector<'_, 'tcx> {
	fn visit_local(&mut self, local: &'tcx rustc_hir::LetStmt<'tcx>) {
		if let rustc_hir::PatKind::Binding(_, binding, ..) = local.pat.kind
			&& local
				.init
				.is_some_and(|init| is_borrowed_data(self.cx, init))
		{
			self.borrowed_buffers.insert(binding);
		}
		rustc_hir::intravisit::walk_local(self, local);
	}

	fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
		match expr.kind {
			ExprKind::Assign(target, ..) => {
				if let Some(binding) = local_path_binding(target) {
					self.facts.reassigned.insert(binding);
				}
			}
			ExprKind::MethodCall(segment, receiver, [value], _)
				if segment.ident.name.as_str() == "fill"
					&& is_literal_zero(value)
					&& is_slice_fill(self.cx, expr) =>
			{
				let receiver = peel_derefs(receiver);
				if is_borrowed_data(self.cx, receiver) {
					self.facts.zero_fills.insert(expr.span);
				} else if let Some(binding) = local_path_binding(receiver) {
					self.buffer_fills.push((binding, expr.span));
				}
			}
			_ => {}
		}
		rustc_hir::intravisit::walk_expr(self, expr);
	}
}

/// Whether `expr` is `account.try_borrow_mut()?`: the whole data buffer.
fn is_borrowed_data(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
	let ExprKind::Match(scrutinee, _, rustc_hir::MatchSource::TryDesugar(_)) = expr.kind else {
		return false;
	};

	shared::try_branch_argument(cx, scrutinee).is_some_and(|argument| {
		matches!(
			argument.kind,
			ExprKind::MethodCall(segment, ..) if segment.ident.name.as_str() == "try_borrow_mut"
		)
	})
}

/// Whether `expr` resolves to `core`'s slice `fill`.
fn is_slice_fill(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
	cx.typeck_results()
		.type_dependent_def_id(expr.hir_id)
		.is_some_and(|definition| {
			cx.tcx.crate_name(definition.krate).as_str() == "core"
				&& cx.tcx.item_name(definition).as_str() == "fill"
		})
}

fn is_literal_zero(expr: &Expr<'_>) -> bool {
	let ExprKind::Lit(literal) = expr.kind else {
		return false;
	};

	matches!(literal.node, LitKind::Int(value, _) if value.get() == 0)
}

fn peel_derefs<'hir>(mut expr: &'hir Expr<'hir>) -> &'hir Expr<'hir> {
	while let ExprKind::Unary(rustc_hir::UnOp::Deref, inner) = expr.kind {
		expr = inner;
	}
	expr
}

fn local_path_binding(expr: &Expr<'_>) -> Option<HirId> {
	match expr.kind {
		ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
			match path.res {
				rustc_hir::def::Res::Local(binding) => Some(binding),
				_ => None,
			}
		}
		_ => None,
	}
}
