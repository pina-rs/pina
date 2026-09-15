extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_hir::Block;
use rustc_hir::Body;
use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::HirId;
use rustc_hir::MatchSource;
use rustc_hir::PatKind;
use rustc_hir::QPath;
use rustc_hir::Stmt;
use rustc_hir::StmtKind;
use rustc_hir::def::Res;
use rustc_hir::intravisit::FnKind;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;
use rustc_span::Span;

use crate::shared;

crate::declare_late_lint! {
	/// ### What it does
	///
	/// Warns when an instruction handler can sweep an account's entire
	/// balance to a recipient in a single call without a preceding pause,
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

/// What the analysis learned about one full-balance drain candidate.
#[derive(Debug, Clone, Copy)]
struct DrainFacts {
	/// The drained account's own complete balance is what leaves the account.
	full_balance: bool,
	/// A guard-shaped call runs first on every path that reaches the drain.
	guarded: bool,
	/// A same-receiver close or zeroing call runs first the same way.
	closing: bool,
}

/// A guard or close call seen so far, tagged with the scope stack it lives in.
#[derive(Debug, Clone)]
struct SeenGuard {
	/// Scope identifiers from the function body down to the call's own block.
	scopes: Vec<u32>,
	/// The call's receiver identity, used to match close intent per account.
	receiver: Option<String>,
	/// Whether this is a close/zeroing call rather than a generic guard.
	is_close: bool,
}

/// One traversal that records every drain candidate with its dominance
/// evidence.
///
/// Guards and close calls only count when they precede the drain *and* their
/// enclosing scopes are a prefix of the drain's own. That prefix rule rejects a
/// guard confined to one `if`/`match` arm or a loop body, while still accepting
/// an early-return guard written in the `if` condition, which is evaluated in
/// the enclosing scope. Proving full dominance is beyond a lexical lint, so the
/// diagnostic also states its control-flow limitation.
#[derive(Default)]
struct DrainAnalyzer<'tcx> {
	/// One identifier per scope pushed as the traversal descends.
	scopes: Vec<u32>,
	next_scope: u32,
	/// Guard-shaped and close calls visited so far, in traversal order.
	guards: Vec<SeenGuard>,
	/// Initializer of every `let` binding, so rebinding chains resolve.
	initializers: HashMap<HirId, &'tcx Expr<'tcx>>,
	/// Facts for every drain candidate, keyed by the call's span.
	drains: HashMap<Span, DrainFacts>,
	/// Drain call spans in traversal order, so diagnostics stay source-ordered.
	order: Vec<Span>,
}

impl<'tcx> DrainAnalyzer<'tcx> {
	/// Analyze a whole function body.
	fn analyze(body: &'tcx Body<'tcx>) -> Self {
		let mut analyzer = Self::default();

		analyzer.visit_expr(body.value);

		analyzer
	}

	/// Whether a guard satisfying `keep` runs before the current position on
	/// every path that can reach it.
	fn has_dominant(&self, keep: impl Fn(&SeenGuard) -> bool) -> bool {
		self.guards
			.iter()
			.any(|guard| keep(guard) && self.scopes.starts_with(&guard.scopes))
	}

	/// Whether `expr` resolves to `<receiver>.lamports()`, following plain
	/// `let` rebindings.
	///
	/// The amount must be the bare balance call: `lamports() / 2` and
	/// `lamports() - reserve` send less than the balance and are not drains.
	fn resolves_to_full_balance(&self, expr: &Expr<'_>, receiver: &str) -> bool {
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
						self.initializers.get(&binding).is_some_and(|initializer| {
							self.resolves_to_full_balance(initializer, receiver)
						})
					}
					_ => false,
				}
			}
			ExprKind::DropTemps(inner)
			| ExprKind::Use(inner, _)
			| ExprKind::AddrOf(_, _, inner)
			| ExprKind::Unary(_, inner) => self.resolves_to_full_balance(inner, receiver),
			_ => false,
		}
	}

	/// Record a call that may guard or close a later drain.
	fn record_guard(&mut self, method: &str, receiver: Option<String>) {
		let lowercase = method.to_ascii_lowercase();
		let is_guard = GUARD_TERMS.iter().any(|term| lowercase.contains(term));
		let is_close = CLOSE_METHODS.contains(&method);

		if is_guard || is_close {
			self.guards.push(SeenGuard {
				scopes: self.scopes.clone(),
				receiver,
				is_close,
			});
		}
	}

	/// Visit `expr` inside a scope a guard cannot escape to stay dominant.
	fn visit_branch(&mut self, expr: &'tcx Expr<'tcx>) {
		let id = self.next_scope;

		self.next_scope += 1;

		self.scopes.push(id);
		self.visit_expr(expr);
		self.scopes.pop();
	}

	/// Visit a block in its own scope, for constructs whose body is a block
	/// rather than an expression.
	fn visit_branch_block(&mut self, block: &'tcx Block<'tcx>) {
		let id = self.next_scope;

		self.next_scope += 1;

		self.scopes.push(id);
		self.visit_block(block);
		self.scopes.pop();
	}

	/// Visit every statement of a block in order, in the current scope.
	fn visit_block(&mut self, block: &'tcx Block<'tcx>) {
		for statement in block.stmts {
			self.visit_stmt(statement);
		}

		if let Some(tail) = block.expr {
			self.visit_expr(tail);
		}
	}

	fn visit_stmt(&mut self, statement: &'tcx Stmt<'tcx>) {
		if let StmtKind::Let(local) = &statement.kind {
			if let (PatKind::Binding(_, binding, ..), Some(initializer)) =
				(&local.pat.kind, local.init)
			{
				self.initializers.insert(*binding, initializer);
			}

			if let Some(initializer) = local.init {
				self.visit_expr(initializer);
			}

			if let Some(else_block) = local.els {
				self.visit_branch_block(else_block);
			}
		} else if let StmtKind::Expr(expr) | StmtKind::Semi(expr) = &statement.kind {
			self.visit_expr(expr);
		}
	}

	fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
		match &expr.kind {
			ExprKind::MethodCall(segment, receiver, arguments, _) => {
				let method = segment.ident.name.as_str();
				let receiver_identity = shared::expression_identity(receiver);

				if DRAIN_METHODS.contains(&method) && arguments.len() == 3 {
					let full_balance = receiver_identity.as_deref().is_some_and(|receiver| {
						arguments
							.get(1)
							.is_some_and(|amount| self.resolves_to_full_balance(amount, receiver))
					});
					let guarded = self.has_dominant(|guard| !guard.is_close);
					let closing = self.has_dominant(|guard| {
						guard.is_close
							&& guard.receiver.is_some()
							&& guard.receiver == receiver_identity
					});

					// Recorded before the arguments are visited: an amount
					// expression cannot guard the sweep it is part of.
					self.drains.insert(
						expr.span,
						DrainFacts {
							full_balance,
							guarded,
							closing,
						},
					);
					self.order.push(expr.span);
				} else {
					self.record_guard(method, receiver_identity);
				}

				self.visit_expr(receiver);

				for argument in *arguments {
					self.visit_expr(argument);
				}
			}
			ExprKind::Block(block, _) => self.visit_block(block),
			ExprKind::If(condition, then, otherwise) => {
				// The condition runs in the enclosing scope, so an early-return
				// guard written there still dominates what follows.
				self.visit_expr(condition);
				self.visit_branch(then);

				if let Some(otherwise) = otherwise {
					self.visit_branch(otherwise);
				}
			}
			ExprKind::Match(scrutinee, arms, source) => {
				// `?` desugars into a match, and its arms are compiler-generated.
				// Treating them as branches would hide every checked guard.
				let try_desugar = matches!(source, MatchSource::TryDesugar(_));

				self.visit_expr(scrutinee);

				for arm in *arms {
					if let Some(guard) = arm.guard {
						self.visit_branch(guard);
					}

					if try_desugar {
						self.visit_expr(arm.body);
					} else {
						self.visit_branch(arm.body);
					}
				}
			}
			// A loop body may never run, so nothing inside it dominates the
			// code that follows the loop.
			ExprKind::Loop(block, ..) => self.visit_branch_block(block),
			ExprKind::Closure(closure) => {
				// A closure body runs when it is called, not where it is
				// written, so its calls cannot guard the enclosing drain.
				let _ = closure;
			}
			ExprKind::Call(callee, arguments) => {
				self.visit_expr(callee);

				for argument in *arguments {
					self.visit_expr(argument);
				}
			}
			ExprKind::Binary(_, left, right) => {
				self.visit_expr(left);
				self.visit_expr(right);
			}
			ExprKind::Assign(left, right, _) | ExprKind::AssignOp(_, left, right) => {
				self.visit_expr(left);
				self.visit_expr(right);
			}
			ExprKind::Index(base, index, _) => {
				self.visit_expr(base);
				self.visit_expr(index);
			}
			ExprKind::Let(let_expr) => self.visit_expr(let_expr.init),
			ExprKind::Tup(expressions) | ExprKind::Array(expressions) => {
				for expression in *expressions {
					self.visit_expr(expression);
				}
			}
			ExprKind::Struct(_, fields, tail) => {
				for field in *fields {
					self.visit_expr(field.expr);
				}

				if let rustc_hir::StructTailExpr::Base(base) = tail {
					self.visit_expr(base);
				}
			}
			ExprKind::Ret(Some(inner)) | ExprKind::Break(_, Some(inner)) => self.visit_expr(inner),
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
			| ExprKind::UnsafeBinderCast(_, inner, _) => self.visit_expr(inner),
			_ => {}
		}
	}
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

		let analyzer = DrainAnalyzer::analyze(body);

		for span in &analyzer.order {
			let Some(facts) = analyzer.drains.get(span) else {
				continue;
			};
			if !facts.full_balance || facts.guarded || facts.closing {
				continue;
			}

			cx.lint(REQUIRE_GUARDED_FULL_BALANCE_DRAIN, |diag| {
				diag.span(*span);
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
