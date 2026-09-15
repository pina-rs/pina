extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_hir::Body;
use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::HirId;
use rustc_hir::PatKind;
use rustc_hir::QPath;
use rustc_hir::StmtKind;
use rustc_hir::def::Res;
use rustc_hir::intravisit::FnKind;
use rustc_hir::intravisit::Visitor;
use rustc_hir::intravisit::walk_expr;
use rustc_hir::intravisit::walk_stmt;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;
use rustc_span::Span;

use crate::shared;

crate::declare_late_lint! {
	/// ### What it does
	///
	/// Warns when an instruction handler can sweep an account's entire
	/// balance to a recipient in a single call without a visible pause,
	/// circuit-breaker, or withdrawal-cap guard.
	///
	/// ### Why is this bad?
	///
	/// Ungated full-balance drains are the shape real key-compromise exploits
	/// use: once the sweep authority leaks, nothing on-chain slows the drain.
	/// A pause switch plus a per-window withdrawal cap bounds the blast
	/// radius, and a separate guardian key can halt the program without being
	/// able to move funds.
	pub REQUIRE_GUARDED_FULL_BALANCE_DRAIN,
	Warn,
	"full-balance drains should be gated by a pause or circuit-breaker guard"
}

const DRAIN_METHODS: &[&str] = &["send", "send_owned"];
const CLOSE_METHODS: &[&str] = &[
	"zeroed",
	"close",
	"close_with_recipient",
	"close_account_zeroed",
];
const GUARD_TERMS: &[&str] = &[
	"pause", "cap", "circuit", "halt", "guard", "limit", "throttle",
];
const TARGET_NEEDLES: &[&str] = &["process", "process_instruction", "instruction"];

/// HIR facts the lexical call list cannot answer precisely: the amount each
/// drain call passes, and the initializer of every `let` binding.
///
/// The call list records a binding name for a nested call, so `lamports() / 2`
/// and `lamports()` look identical there. Resolving the actual amount
/// expression keeps partial withdrawals from being reported as full drains.
#[derive(Default)]
struct DrainAmounts<'tcx> {
	drain_arguments: HashMap<Span, &'tcx [Expr<'tcx>]>,
	initializers: HashMap<HirId, &'tcx Expr<'tcx>>,
}

impl<'tcx> Visitor<'tcx> for DrainAmounts<'tcx> {
	fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
		if let ExprKind::MethodCall(segment, _, arguments, _) = &expr.kind
			&& DRAIN_METHODS.contains(&segment.ident.name.as_str())
			&& arguments.len() == 3
		{
			self.drain_arguments.insert(expr.span, arguments);
		}

		walk_expr(self, expr);
	}

	fn visit_stmt(&mut self, statement: &'tcx rustc_hir::Stmt<'tcx>) {
		if let StmtKind::Let(local) = &statement.kind
			&& let (PatKind::Binding(_, binding, ..), Some(initializer)) =
				(&local.pat.kind, local.init)
		{
			self.initializers.insert(*binding, initializer);
		}

		walk_stmt(self, statement);
	}
}

/// Whether an expression resolves to the drained account's own complete
/// balance, following any number of plain `let` rebindings.
///
/// The amount must be the bare balance call: `lamports() / 2` and
/// `lamports() - reserve` send less than the balance and are not drains.
fn resolves_to_full_balance(expr: &Expr<'_>, receiver: &str, amounts: &DrainAmounts<'_>) -> bool {
	match &expr.kind {
		ExprKind::MethodCall(segment, inner_receiver, arguments, _) => {
			segment.ident.name.as_str() == "lamports"
				&& arguments.is_empty()
				&& shared::expression_identity(inner_receiver).as_deref() == Some(receiver)
		}
		// `let amount = balance;` keeps the value, so keep following it.
		ExprKind::Path(QPath::Resolved(_, path)) => {
			match path.res {
				Res::Local(binding) => {
					amounts
						.initializers
						.get(&binding)
						.is_some_and(|initializer| {
							resolves_to_full_balance(initializer, receiver, amounts)
						})
				}
				_ => false,
			}
		}
		ExprKind::DropTemps(inner)
		| ExprKind::Use(inner, _)
		| ExprKind::AddrOf(_, _, inner)
		| ExprKind::Unary(_, inner) => resolves_to_full_balance(inner, receiver, amounts),
		_ => false,
	}
}

/// Whether the drain call passes the drained account's own complete balance,
/// inline or through plain `let` bindings.
fn drains_full_balance(drain: &shared::CallInfo, amounts: &DrainAmounts<'_>) -> bool {
	let Some(receiver) = drain.receiver.as_deref() else {
		return false;
	};
	let Some(arguments) = amounts.drain_arguments.get(&drain.span) else {
		return false;
	};
	let Some(amount) = arguments.get(1) else {
		return false;
	};

	resolves_to_full_balance(amount, receiver, amounts)
}

fn has_close_intent(facts: &shared::FunctionFacts, drain: &shared::CallInfo) -> bool {
	facts.calls.iter().any(|call| {
		CLOSE_METHODS.contains(&call.method.as_str()) && call.receiver == drain.receiver
	})
}

fn has_guard(facts: &shared::FunctionFacts) -> bool {
	facts.calls.iter().any(|call| {
		let method = call.method.to_ascii_lowercase();

		GUARD_TERMS.iter().any(|term| method.contains(term))
	})
}

impl<'tcx> LateLintPass<'tcx> for RequireGuardedFullBalanceDrain {
	fn check_fn(
		&mut self,
		cx: &LateContext<'tcx>,
		_: FnKind<'tcx>,
		_: &'tcx rustc_hir::FnDecl<'tcx>,
		body: &'tcx Body<'tcx>,
		_: Span,
		def_id: rustc_hir::def_id::LocalDefId,
	) {
		let def_path = cx.tcx.def_path_str(def_id.to_def_id());
		if shared::should_skip_def_path(&def_path)
			|| !shared::def_path_matches(&def_path, TARGET_NEEDLES)
		{
			return;
		}

		let facts = shared::collect_function_facts(cx, body);

		if has_guard(&facts) {
			return;
		}

		let mut amounts = DrainAmounts::default();
		amounts.visit_body(body);

		for drain in &facts.calls {
			if !DRAIN_METHODS.contains(&drain.method.as_str())
				|| !drains_full_balance(drain, &amounts)
				|| has_close_intent(&facts, drain)
			{
				continue;
			}

			cx.lint(REQUIRE_GUARDED_FULL_BALANCE_DRAIN, |diag| {
				diag.span(drain.span);
				diag.primary_message(
					"an instruction path can sweep an account's entire balance in one call",
				);
				diag.help(
					"gate full-balance sweeps behind a pause or circuit-breaker check (a pause \
					 flag plus a per-window withdrawal cap bounds a compromised key's blast \
					 radius)",
				);
				diag.help(
					"if this drain is an account-close path, use `close_account_zeroed` so \
					 `require_zeroed_before_close` covers the stale-data risk too",
				);
				diag.help(shared::CONTROL_FLOW_LIMITATION_HELP);
			});
		}
	}
}
