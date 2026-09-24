extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_hir::BinOpKind;
use rustc_hir::Block;
use rustc_hir::Body;
use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::HirId;
use rustc_hir::MatchSource;
use rustc_hir::Node;
use rustc_hir::PatKind;
use rustc_hir::QPath;
use rustc_hir::Stmt;
use rustc_hir::StmtKind;
use rustc_hir::UnOp;
use rustc_hir::def::DefKind;
use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::intravisit::Visitor;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_middle::ty::TypeckResults;
use rustc_span::Span;

use crate::diagnostics;
use crate::shared;

crate::declare_late_lint! {
	/// ### What it does
	///
	/// Warns when an instruction handler can sweep an account's entire
	/// balance to a recipient in a single call without a preceding pause,
	/// circuit-breaker, or withdrawal-cap guard.
	///
	/// A call counts as the guard only when it behaves like one:
	///
	/// - its result is enforced before the drain, through `?`, `unwrap()`,
	///   `expect()`, or an `if`, `match`, or `let ... else` that leaves the
	///   function on failure — a discarded result gates nothing;
	/// - it reads the state it checks, as a receiver or an argument that
	///   refers to a local binding — a zero-argument call cannot inspect the
	///   configuration it claims to guard; and
	/// - its name says it is a pause, cap, circuit-breaker, halt, guard,
	///   limit, or throttle check, or it is a local function whose body
	///   enforces such a check on every path, so a differently named wrapper
	///   still counts.
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
/// Name fragments that state a call's intent to gate value movement.
///
/// The name alone is never enough: a guard must also enforce its result and
/// read the state it checks. The name stays a requirement because behavior
/// alone cannot tell a pause or cap check from any other propagated
/// validation — every handler propagates `assert_signer()?` on an account
/// before it moves funds, and that is not a circuit breaker.
const GUARD_TERMS: &[&str] = &[
	"pause", "cap", "circuit", "halt", "guard", "limit", "throttle",
];
const TARGET_NEEDLES: &[&str] = &["process", "process_instruction", "instruction"];
/// `Result` and `Option` adapters that keep a guard's failure observable, so
/// the enforcement can happen on the adapted value instead.
const FAILURE_PRESERVING_METHODS: &[&str] = &[
	"inspect",
	"inspect_err",
	"is_err",
	"is_none",
	"is_ok",
	"is_some",
	"map_err",
	"ok_or",
	"ok_or_else",
];
/// `Result` and `Option` extractors that abort the transaction on failure.
const FAILURE_ABORTING_METHODS: &[&str] = &["expect", "unwrap"];
/// How many nested local wrappers the analysis follows to find the guard a
/// differently named call delegates to.
const MAX_WRAPPER_DEPTH: usize = 3;

/// What the analysis learned about one full-balance drain candidate.
#[derive(Debug, Clone, Copy)]
struct DrainFacts {
	/// The drained account's own complete balance is what leaves the account.
	full_balance: bool,
	/// A behaving guard runs first on every path that reaches the drain.
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
///
/// The same traversal analyzes a local wrapper's body: the wrapper is a guard
/// when a behaving guard dominates the end of that body.
struct DrainAnalyzer<'a, 'tcx> {
	cx: &'a LateContext<'tcx>,
	/// Type-check results of the body being walked. A wrapper body has its
	/// own, so these cannot come from `cx`, which only knows the handler.
	typeck: &'tcx TypeckResults<'tcx>,
	/// The walked body's value: an expression that reaches it is returned.
	body_value: HirId,
	/// Local functions entered to reach this body, so wrapper recursion ends.
	callers: Vec<LocalDefId>,
	/// One identifier per scope pushed as the traversal descends.
	scopes: Vec<u32>,
	next_scope: u32,
	/// Guard and close calls visited so far, in traversal order.
	guards: Vec<SeenGuard>,
	/// Initializer of every `let` binding, so rebinding chains resolve.
	initializers: HashMap<HirId, &'tcx Expr<'tcx>>,
	/// Facts for every drain candidate, keyed by the call's span.
	drains: HashMap<Span, DrainFacts>,
	/// Drain call spans in traversal order, so diagnostics stay source-ordered.
	order: Vec<Span>,
}

impl<'a, 'tcx> DrainAnalyzer<'a, 'tcx> {
	/// Analyze a whole function body.
	fn analyze(
		cx: &'a LateContext<'tcx>,
		typeck: &'tcx TypeckResults<'tcx>,
		body: &'tcx Body<'tcx>,
		callers: Vec<LocalDefId>,
	) -> Self {
		let mut analyzer = Self {
			cx,
			typeck,
			body_value: body.value.hir_id,
			callers,
			scopes: Vec::new(),
			next_scope: 0,
			guards: Vec::new(),
			initializers: HashMap::new(),
			drains: HashMap::new(),
			order: Vec::new(),
		};

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

	/// Record a call when it behaves like a guard or closes an account.
	fn record_call(
		&mut self,
		call: &'tcx Expr<'tcx>,
		name: &str,
		receiver: Option<String>,
		inputs: &[&'tcx Expr<'tcx>],
		callee: Option<DefId>,
	) {
		let is_close = CLOSE_METHODS.contains(&name) && receiver.is_some();

		if is_close || self.is_guard(call, name, inputs, callee) {
			self.guards.push(SeenGuard {
				scopes: self.scopes.clone(),
				receiver,
				is_close,
			});
		}
	}

	/// Whether `call` behaves like a guard: it enforces its result, reads the
	/// state it checks, and either names the check or delegates to a local
	/// function that performs one.
	fn is_guard(
		&self,
		call: &'tcx Expr<'tcx>,
		name: &str,
		inputs: &[&'tcx Expr<'tcx>],
		callee: Option<DefId>,
	) -> bool {
		if !inputs.iter().any(|input| reads_local_binding(input)) || !self.result_is_enforced(call)
		{
			return false;
		}

		names_guard(name) || callee.is_some_and(|callee| self.is_guard_wrapper(callee))
	}

	/// Whether a local function enforces a guard on every path through its
	/// body, so a call to it gates the caller the same way.
	fn is_guard_wrapper(&self, callee: DefId) -> bool {
		let tcx = self.cx.tcx;
		let Some(local) = callee.as_local() else {
			return false;
		};

		if self.callers.len() > MAX_WRAPPER_DEPTH
			|| self.callers.contains(&local)
			|| !matches!(tcx.def_kind(callee), DefKind::Fn | DefKind::AssocFn)
		{
			return false;
		}

		let Some(body) = tcx.hir_maybe_body_owned_by(local) else {
			return false;
		};
		let mut callers = self.callers.clone();

		callers.push(local);

		// The analysis ends at the body's outermost scope, so a guard counts
		// only when no branch or loop can skip it.
		DrainAnalyzer::analyze(self.cx, tcx.typeck(local), body, callers)
			.has_dominant(|guard| !guard.is_close)
	}

	/// Whether a failure of `call` stops execution before the code after it:
	/// the result is propagated with `?`, extracted with `unwrap()` or
	/// `expect()`, returned from the function, or tested by an `if`, `match`,
	/// or `let ... else` whose failure side leaves the function.
	///
	/// A call whose result is discarded, or only inspected, gates nothing.
	fn result_is_enforced(&self, call: &Expr<'_>) -> bool {
		let tcx = self.cx.tcx;
		let mut child = call.hir_id;

		loop {
			if child == self.body_value {
				return true;
			}

			let parent = match tcx.parent_hir_node(child) {
				Node::Expr(parent) => parent,
				// A block's tail is the block's value.
				Node::Block(block) if block.expr.is_some_and(|tail| tail.hir_id == child) => {
					child = block.hir_id;
					continue;
				}
				// `let ... else` requires the else block to diverge.
				Node::LetStmt(local) => {
					return local.els.is_some()
						&& local.init.is_some_and(|init| init.hir_id == child);
				}
				_ => return false,
			};

			match &parent.kind {
				// `guard(..)?` lowers to a match on `Try::branch(guard(..))`.
				ExprKind::Call(_, [argument]) if argument.hir_id == child => {
					return matches!(
						tcx.parent_hir_node(parent.hir_id),
						Node::Expr(Expr {
							kind: ExprKind::Match(scrutinee, _, MatchSource::TryDesugar(_)),
							..
						}) if scrutinee.hir_id == parent.hir_id
					);
				}
				ExprKind::MethodCall(_, receiver, ..) if receiver.hir_id == child => {
					if self.is_fallible_method(parent, FAILURE_ABORTING_METHODS) {
						return true;
					}

					if !self.is_fallible_method(parent, FAILURE_PRESERVING_METHODS) {
						return false;
					}
				}
				ExprKind::If(condition, then, _) if condition.hir_id == child => {
					return self.diverges(then);
				}
				ExprKind::Match(scrutinee, arms, MatchSource::Normal)
					if scrutinee.hir_id == child =>
				{
					return arms.iter().any(|arm| self.diverges(arm.body));
				}
				ExprKind::Ret(Some(value)) if value.hir_id == child => return true,
				// Either failing operand takes the diverging branch.
				ExprKind::Binary(operator, ..) if operator.node == BinOpKind::Or => {}
				ExprKind::Unary(UnOp::Not, _)
				| ExprKind::Let(_)
				| ExprKind::Block(..)
				| ExprKind::DropTemps(_)
				| ExprKind::Use(..)
				| ExprKind::Type(..) => {}
				_ => return false,
			}

			child = parent.hir_id;
		}
	}

	/// Whether `expr` resolves to one of `methods` on core's `Result` or
	/// `Option`, rather than a local method that happens to share the name.
	fn is_fallible_method(&self, expr: &Expr<'_>, methods: &[&str]) -> bool {
		let Some(definition) = self.typeck.type_dependent_def_id(expr.hir_id) else {
			return false;
		};
		let tcx = self.cx.tcx;
		let path = tcx.def_path_str(definition);

		tcx.crate_name(definition.krate).as_str() == "core"
			&& (path.contains("::result::Result") || path.contains("::option::Option"))
			&& methods.contains(&tcx.item_name(definition).as_str())
	}

	/// Whether evaluating `expr` never continues past it.
	fn diverges(&self, expr: &Expr<'_>) -> bool {
		self.typeck.expr_ty(expr).is_never()
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
					// The receiver is the state a method-style guard reads.
					let inputs: Vec<&'tcx Expr<'tcx>> =
						std::iter::once(*receiver).chain(arguments.iter()).collect();
					let callee = self.typeck.type_dependent_def_id(expr.hir_id);

					self.record_call(expr, method, receiver_identity, &inputs, callee);
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
				if let ExprKind::Path(qpath) = &callee.kind
					&& let Some(name) = qpath_name(qpath)
				{
					let definition = match self.typeck.qpath_res(qpath, callee.hir_id) {
						Res::Def(_, definition) => Some(definition),
						_ => None,
					};
					let inputs: Vec<&'tcx Expr<'tcx>> = arguments.iter().collect();

					self.record_call(expr, name, None, &inputs, definition);
				}

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

/// Whether a call name states pause, cap, or circuit-breaker intent.
fn names_guard(name: &str) -> bool {
	let lowercase = name.to_ascii_lowercase();

	GUARD_TERMS.iter().any(|term| lowercase.contains(term))
}

/// The final segment of a called path: `check` in `limits::check(..)`.
fn qpath_name<'hir>(qpath: &QPath<'hir>) -> Option<&'hir str> {
	match qpath {
		QPath::Resolved(_, path) => {
			path.segments
				.last()
				.map(|segment| segment.ident.name.as_str())
		}
		QPath::TypeRelative(_, segment) => Some(segment.ident.name.as_str()),
	}
}

/// Whether `expr` mentions a local binding — a parameter such as `self` or
/// the handler's accounts, or a value computed from them.
///
/// Literals and constants cannot carry the state a guard checks, so a guard
/// whose inputs are all compile-time values is a name, not a check.
fn reads_local_binding<'hir>(expr: &'hir Expr<'hir>) -> bool {
	struct LocalFinder {
		found: bool,
	}

	impl<'hir> Visitor<'hir> for LocalFinder {
		fn visit_path(&mut self, path: &rustc_hir::Path<'hir>, _: HirId) {
			self.found |= matches!(path.res, Res::Local(_));

			rustc_hir::intravisit::walk_path(self, path);
		}
	}

	let mut finder = LocalFinder { found: false };

	finder.visit_expr(expr);

	finder.found
}

impl<'tcx> LateLintPass<'tcx> for RequireGuardedFullBalanceDrain {
	fn check_fn(
		&mut self,
		cx: &LateContext<'tcx>,
		_: FnKind<'tcx>,
		_: &'tcx rustc_hir::FnDecl<'tcx>,
		body: &'tcx Body<'tcx>,
		_: Span,
		def_id: LocalDefId,
	) {
		let def_path = cx.tcx.def_path_str(def_id.to_def_id());
		if shared::should_skip_def_path(&def_path)
			|| !shared::def_path_matches(&def_path, TARGET_NEEDLES)
		{
			return;
		}

		let analyzer = DrainAnalyzer::analyze(cx, cx.typeck_results(), body, vec![def_id]);

		for span in &analyzer.order {
			let Some(facts) = analyzer.drains.get(span) else {
				continue;
			};
			if !facts.full_balance || facts.guarded || facts.closing {
				continue;
			}

			diagnostics::emit(cx, REQUIRE_GUARDED_FULL_BALANCE_DRAIN, |diag| {
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
					"a guard counts only when it reads the state it checks (a receiver or \
					 argument) and its failure stops the handler before the drain (`?`, \
					 `unwrap`/`expect`, or an `if`/`match`/`let ... else` that returns); a \
					 differently named local wrapper counts when its body enforces such a guard",
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
