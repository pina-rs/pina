extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashMap;
use std::collections::HashSet;

use rustc_ast::LitKind;
use rustc_hir::BinOpKind;
use rustc_hir::Block;
use rustc_hir::Body;
use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::HirId;
use rustc_hir::LetStmt;
use rustc_hir::MatchSource;
use rustc_hir::Node;
use rustc_hir::Pat;
use rustc_hir::PatExprKind;
use rustc_hir::PatKind;
use rustc_hir::QPath;
use rustc_hir::Stmt;
use rustc_hir::StmtKind;
use rustc_hir::UnOp;
use rustc_hir::def::CtorOf;
use rustc_hir::def::DefKind;
use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::intravisit::Visitor;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_middle::ty::EarlyBinder;
use rustc_middle::ty::GenericArgsRef;
use rustc_middle::ty::Instance;
use rustc_middle::ty::InstanceKind;
use rustc_middle::ty::TyKind;
use rustc_middle::ty::TypeckResults;
use rustc_middle::ty::TypingEnv;
use rustc_middle::ty::Unnormalized;
use rustc_span::ExpnKind;
use rustc_span::MacroKind;
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
	/// A call counts as the guard only when all of the following hold:
	///
	/// - **The guard's failure stops the drain.** A `Result` or `Option`
	///   guard is propagated with `?`, extracted with `unwrap()`/`expect()`,
	///   returned, or tested by a `match`, `if let`, or `let ... else` in which
	///   every arm that can receive the failure returns `Err`/`None`, returns
	///   the scrutinee's own binding, or panics. Arms are read in order, so a
	///   `_` after an unguarded `Err(_)` arm only receives success. Adapters
	///   that keep the failure are followed: `map_err`, `inspect_err`, `map`,
	///   `and_then`, `and`, `ok`, `ok_or`, `or(Err(..))`, `or_else` with a
	///   fallback that can only fail, `clone()`, and `into()`/`From::from` into
	///   a `Result` or the same type (never into `Option<Result<..>>`). A `bool`
	///   guard, `is_err()`/`is_ok()`, or `eq`/`ne`/`==`/`!=` against `Ok(..)`
	///   must gate an `if`, `assert!`, `assert_eq!`,
	///   `assert_ne!`, or `pina::assert(..)?` so that execution continues only
	///   on the passing value; that polarity is checked whenever the flag comes
	///   from a fallible guard. A failing branch returns `Err`/`None`, returns
	///   a local helper that can only fail, or panics; a branch that returns
	///   `Ok`, breaks, or continues is not a failure. A guard-named local method
	///   returning `()` counts where it is called when its body can panic, as
	///   an `assert!`-style guard does. A guard bound to a local
	///   counts where the local is enforced, and a labeled block that can
	///   `break` with a success does not carry its tail guard out.
	/// - **It reads the handler's inputs.** Its receiver or an argument is
	///   derived from a function parameter (including `self`), directly or
	///   through locals bound from one; a zero-argument call, or one fed only
	///   literals and constants, cannot inspect the state it claims to guard.
	/// - **It names the check, or delegates to one.** It is a function or
	///   method whose name contains `pause`, `cap`, `circuit`, `halt`,
	///   `guard`, `limit`, or `throttle`, or a local function returning
	///   `Result`/`Option` whose body enforces such a guard in its outermost
	///   scope before any early non-error `return` or `break` (followed up to
	///   three wrappers deep). A generic wrapper is instantiated with its
	///   caller's arguments, so its trait calls are judged by the impl that
	///   runs; inside a wrapper, a trait call that cannot be resolved (`dyn`,
	///   an unconstrained generic) and a returned value the lint cannot see
	///   into count as neither a guard nor a failure. Closures, fn pointers,
	///   and generic callables are named by their binding and never count by
	///   name.
	/// - **It is not a constant success.** A local callee whose every returned
	///   value is a literal `Ok(..)`/`Some(..)` (or `bool` literal), directly
	///   or through a `let` binding that is never reassigned, and that has no
	///   reachable `?`, `Err`/`None`, or panic, is not a guard. Branches behind
	///   a literal `if true`/`if false` are treated as unreachable. Trait
	///   methods are judged by the implementation that runs; in the handler
	///   itself, when the lint cannot resolve it, only the method name is used.
	///
	/// An early `return Ok(..)` on the failing side, such as
	/// `if state.is_paused() { return Ok(()) }`, deliberately does not count:
	/// for a `bool` guard the lint cannot tell which value is the failing one,
	/// and reporting success for a blocked sweep hides the pause from callers.
	///
	/// ### Why is this bad?
	///
	/// Ungated full-balance drains are the shape real key-compromise exploits
	/// use: once the sweep authority leaks, nothing on-chain slows the drain.
	/// A pause switch plus a per-window withdrawal cap bounds the blast
	/// radius, and a separate guardian key can halt the program without being
	/// able to move funds.
	///
	/// ### Known limits
	///
	/// The analysis is lexical and follows only local wrappers, so:
	///
	/// - a `bool` guard's polarity is unknown, so a failing branch on either
	///   side of its `if` counts;
	/// - a callee from another crate, or a trait call that cannot be resolved
	///   in a generic handler (not a wrapper), is judged by its name and
	///   call-site behavior only, and a unit guard from another crate never
	///   counts;
	/// - a local guard-named callee counts when anything in its body can fail
	///   or panic, even for reasons unrelated to the check its name claims,
	///   and the constant-success test does not evaluate conditions beyond
	///   literal `true`/`false`;
	/// - in the handler (not in a wrapper), `return helper()` on the failing
	///   side counts when `helper` cannot be analyzed (another crate or an
	///   unresolved trait call), as the name rule did before, because any
	///   `return` there skips the drain;
	/// - `async fn` handlers and guards are not analyzed through `.await`
	///   (irrelevant for SBF programs);
	/// - wrappers deeper than three levels are not followed; and
	/// - a pause check with no guard-named call, such as
	///   `if state.paused { return Err(..) }`, is not recognized.
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
/// The name alone is never enough: a guard must also enforce its failure and
/// read the handler's inputs. The name stays a requirement because behavior
/// alone cannot tell a pause or cap check from any other propagated
/// validation — every handler propagates `assert_signer()?` on an account
/// before it moves funds, and that is not a circuit breaker.
const GUARD_TERMS: &[&str] = &[
	"pause", "cap", "circuit", "halt", "guard", "limit", "throttle",
];
const TARGET_NEEDLES: &[&str] = &["process", "process_instruction", "instruction"];
/// `Result`/`Option` methods whose result still fails exactly when the
/// receiver failed, so enforcing the adapted value enforces the guard.
const FAILURE_PRESERVING_METHODS: &[&str] = &[
	"and",
	"and_then",
	"inspect",
	"inspect_err",
	"map",
	"map_err",
	"ok",
	"ok_or",
	"ok_or_else",
];
/// `Result`/`Option` extractors that panic on failure.
const FAILURE_ABORTING_METHODS: &[&str] = &["expect", "unwrap"];
/// Predicates that are `true` exactly when the receiver failed.
const FAILURE_TRUE_PREDICATES: &[&str] = &["is_err", "is_none"];
/// Predicates that are `false` whenever the receiver failed.
const FAILURE_FALSE_PREDICATES: &[&str] = &["is_ok", "is_ok_and", "is_some", "is_some_and"];
/// Standard assertion macros whose failure panics. `debug_assert*` expands
/// to one of these under `if cfg!(debug_assertions)`, so the branch scope
/// already keeps it from counting.
const ASSERT_EQUALITY_MACROS: &[&str] = &["assert_eq", "assert_ne"];
/// How many nested local wrappers the analysis follows to find the guard a
/// differently named call delegates to.
const MAX_WRAPPER_DEPTH: usize = 3;

/// What the analysis learned about one full-balance drain candidate.
#[derive(Debug, Clone, Copy)]
struct DrainFacts {
	/// The drained account's own complete balance is what leaves the account.
	full_balance: bool,
	/// An enforced guard runs first on every path that reaches the drain.
	guarded: bool,
	/// A same-receiver close or zeroing call runs first the same way.
	closing: bool,
}

/// A guard or close call seen so far, tagged with the scope stack it lives in.
#[derive(Debug, Clone)]
struct SeenGuard {
	/// Scope identifiers from the function body down to the enforcement point.
	scopes: Vec<u32>,
	/// The call's receiver identity, used to match close intent per account.
	receiver: Option<String>,
	/// Whether this is a close/zeroing call rather than a generic guard.
	is_close: bool,
}

/// The value a guard call produces, as the enforcement walk follows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GuardValue {
	/// A `Result` or `Option`: `Err`/`None` is the guard failing.
	Fallible,
	/// A `bool`. `Some(true)` means `true` is the failing value, `Some(false)`
	/// means `false` is, and `None` means the guard returned the `bool`
	/// itself, whose polarity the name does not reveal.
	Flag(Option<bool>),
}

/// Where the enforcement walk ended for one guard value.
#[derive(Debug)]
enum Enforcement {
	/// The guard's failure stops execution. The listed `&&`/`||` operators
	/// hold the guard in their conditionally evaluated right operand, and
	/// their outcome is part of the enforcement.
	Enforced(Vec<HirId>),
	/// The value was bound to a local; its uses decide.
	Bound(HirId, GuardValue),
	Unenforced,
}

/// How evaluating an expression can leave it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Exit {
	/// Every path returns `Err`/`None` or panics.
	Failure,
	/// Some path leaves with a non-failure `return`, `break`, or `continue`.
	Escape,
	/// Some path continues past the expression.
	FallThrough,
}

/// Which `Result`/`Option` values a pattern can match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatternClass {
	Success,
	Failure,
	Both,
}

/// One traversal that records every drain candidate with its dominance
/// evidence.
///
/// Guards and close calls only count when they precede the drain *and* their
/// enclosing scopes are a prefix of the drain's own. That prefix rule rejects a
/// guard confined to one `if`/`match` arm, a loop body, or the right operand of
/// `&&`/`||`, while still accepting an early-return guard written in the `if`
/// condition, which is evaluated in the enclosing scope. Proving full dominance
/// is beyond a lexical lint, so the diagnostic also states its control-flow
/// limitation.
///
/// The same traversal analyzes a local wrapper's body: the wrapper is a guard
/// when an enforced guard sits in the body's outermost scope before any early
/// non-error `return`.
struct DrainAnalyzer<'a, 'tcx> {
	cx: &'a LateContext<'tcx>,
	/// Type-check results of the body being walked. A wrapper body has its
	/// own, so these cannot come from `cx`, which only knows the handler.
	typeck: &'tcx TypeckResults<'tcx>,
	/// The walked body's value: an expression that reaches it is returned.
	body_value: HirId,
	/// Local functions entered to reach this body: the handler first, the
	/// body's own function last.
	callers: Vec<LocalDefId>,
	/// The generic arguments this body is instantiated with at the call that
	/// entered it, in terms of the handler's generics. `None` for the handler,
	/// whose own generics stand for themselves.
	instance_args: Option<GenericArgsRef<'tcx>>,
	/// Bindings introduced by the body's parameters, `self` included.
	parameters: HashSet<HirId>,
	/// The expression each pattern binding was bound from.
	origins: HashMap<HirId, &'tcx Expr<'tcx>>,
	/// Locals holding a guard's unenforced value, keyed by binding.
	pending: HashMap<HirId, GuardValue>,
	/// Scope depth outside each visited `&&`/`||` right operand.
	short_circuit_depths: HashMap<HirId, usize>,
	/// One identifier per scope pushed as the traversal descends.
	scopes: Vec<u32>,
	next_scope: u32,
	/// Guard and close calls enforced so far, in traversal order.
	guards: Vec<SeenGuard>,
	/// `guards.len()` at every non-error `return`, in traversal order.
	escapes: Vec<usize>,
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
		instance_args: Option<GenericArgsRef<'tcx>>,
	) -> Self {
		let mut parameters = HashSet::new();

		for parameter in body.params {
			parameter.pat.each_binding(|_, binding, _, _| {
				parameters.insert(binding);
			});
		}

		let mut analyzer = Self {
			cx,
			typeck,
			body_value: body.value.hir_id,
			callers,
			instance_args,
			parameters,
			origins: HashMap::new(),
			pending: HashMap::new(),
			short_circuit_depths: HashMap::new(),
			scopes: Vec::new(),
			next_scope: 0,
			guards: Vec::new(),
			escapes: Vec::new(),
			initializers: HashMap::new(),
			drains: HashMap::new(),
			order: Vec::new(),
		};

		analyzer.visit_expr(body.value);

		analyzer
	}

	/// The handler the analysis started from; every resolved argument list
	/// is expressed in its generics.
	fn root(&self) -> LocalDefId {
		*self
			.callers
			.first()
			.expect("an analysis always starts from its own body")
	}

	/// Whether this body is the handler itself rather than a wrapper it calls.
	fn is_handler(&self) -> bool {
		self.callers.len() == 1
	}

	/// Whether a guard satisfying `keep` runs before the current position on
	/// every path that can reach it.
	fn has_dominant(&self, keep: impl Fn(&SeenGuard) -> bool) -> bool {
		self.guards
			.iter()
			.any(|guard| keep(guard) && self.scopes.starts_with(&guard.scopes))
	}

	/// Whether an enforced guard sits in the body's outermost scope and runs
	/// before any early non-error `return` could skip it.
	fn guards_every_exit(&self) -> bool {
		let first_escape = self.escapes.first().copied().unwrap_or(usize::MAX);

		self.guards
			.iter()
			.take(first_escape)
			.any(|guard| !guard.is_close && guard.scopes.is_empty())
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

	/// Record a call when it closes an account, or when it behaves like a
	/// guard and its failure is enforced.
	fn record_call(
		&mut self,
		call: &'tcx Expr<'tcx>,
		name: &str,
		receiver: Option<String>,
		inputs: &[&'tcx Expr<'tcx>],
		callee: Option<(DefId, HirId)>,
	) {
		if CLOSE_METHODS.contains(&name) && receiver.is_some() {
			self.guards.push(SeenGuard {
				scopes: self.scopes.clone(),
				receiver,
				is_close: true,
			});

			return;
		}

		if !inputs.iter().any(|input| self.reads_parameter(input)) {
			return;
		}

		let implementation = callee.and_then(|(definition, args)| self.resolve(definition, args));

		// Inside a wrapper, a trait call that cannot be resolved to the impl
		// that runs is unknown: its name belongs to the trait, not the code.
		if implementation.is_none()
			&& !self.is_handler()
			&& callee
				.is_some_and(|(definition, _)| self.cx.tcx.trait_of_assoc(definition).is_some())
		{
			return;
		}

		let local_body = implementation.and_then(|(definition, args)| {
			self.local_body(definition)
				.map(|(local, body)| (local, body, args))
		});

		let Some(value) = self.guard_value(call) else {
			// A guard-named local method returning `()` enforces itself when its
			// body can panic, as `assert!`-style guards do.
			if names_guard(name)
				&& self.typeck.expr_ty(call).is_unit()
				&& local_body.is_some_and(|(local, body, _)| body_can_panic(self.cx, local, body))
			{
				self.guards.push(SeenGuard {
					scopes: self.scopes.clone(),
					receiver: None,
					is_close: false,
				});
			}

			return;
		};

		// A local body shows what the call really does: one that can only
		// succeed gates nothing, whatever its name.
		if let Some((local, body, _)) = local_body
			&& returns_constant(self.cx, local, body, value, Outcome::Succeeds)
		{
			return;
		}

		let delegates = value == GuardValue::Fallible
			&& local_body
				.is_some_and(|(local, body, args)| self.is_guard_wrapper(local, body, args));

		if names_guard(name) || delegates {
			self.enforce(call, value);
		}
	}

	/// Follow a guard value to its enforcement and record or bind it.
	fn enforce(&mut self, expr: &'tcx Expr<'tcx>, value: GuardValue) {
		match self.enforcement(expr, value) {
			Enforcement::Enforced(short_circuits) => {
				// A guard inside a short-circuit operand whose outcome gates the
				// path is enforced where the whole operator is evaluated.
				let depth = short_circuits
					.iter()
					.filter_map(|operator| self.short_circuit_depths.get(operator))
					.min()
					.copied()
					.unwrap_or(self.scopes.len());

				self.guards.push(SeenGuard {
					scopes: self.scopes[..depth].to_vec(),
					receiver: None,
					is_close: false,
				});
			}
			Enforcement::Bound(binding, value) => {
				self.pending.insert(binding, value);
			}
			Enforcement::Unenforced => {}
		}
	}

	/// The kind of value a call produces, when it can express a failure.
	fn guard_value(&self, call: &Expr<'_>) -> Option<GuardValue> {
		let ty = self.typeck.expr_ty(call);

		if ty.is_bool() {
			return Some(GuardValue::Flag(None));
		}

		let TyKind::Adt(definition, _) = ty.kind() else {
			return None;
		};

		is_core_item(self.cx, definition.did(), &["Result", "Option"])
			.then_some(GuardValue::Fallible)
	}

	/// The implementation a call runs, with the arguments it runs with: a
	/// trait method resolves to the impl's method (or the trait default the
	/// impl inherits). Inside a wrapper, the call's arguments are first
	/// instantiated with the wrapper's own arguments, so a generic wrapper is
	/// judged by the concrete impl its caller picked. `None` when the instance
	/// still depends on unknown generics or is not a plain item.
	fn resolve(
		&self,
		definition: DefId,
		args_owner: HirId,
	) -> Option<(DefId, GenericArgsRef<'tcx>)> {
		let tcx = self.cx.tcx;

		if !matches!(tcx.def_kind(definition), DefKind::Fn | DefKind::AssocFn) {
			return None;
		}

		let args = self.typeck.node_args(args_owner);

		// Typeck only records instantiation arguments where it resolved the
		// call itself; resolving with a mismatched list would ICE.
		if args.len() != tcx.generics_of(definition).count() {
			return None;
		}

		let args = match self.instance_args {
			Some(instance_args) => EarlyBinder::bind(tcx, args).instantiate(tcx, instance_args),
			None => Unnormalized::new_wip(args),
		};
		let typing_env = TypingEnv::post_analysis(tcx, self.root().to_def_id());
		let Ok(args) = tcx.try_normalize_erasing_regions(typing_env, args) else {
			return None;
		};

		match Instance::try_resolve(tcx, typing_env, definition, args) {
			Ok(Some(instance)) => {
				match instance.def {
					InstanceKind::Item(item) => Some((item, instance.args)),
					_ => None,
				}
			}
			_ => None,
		}
	}

	/// The body of a local function the analysis may enter.
	fn local_body(&self, definition: DefId) -> Option<(LocalDefId, &'tcx Body<'tcx>)> {
		let local = definition.as_local()?;
		let body = self.cx.tcx.hir_maybe_body_owned_by(local)?;

		Some((local, body))
	}

	/// Whether a local function enforces a guard before any early non-error
	/// `return`, so a call to it gates the caller the same way.
	fn is_guard_wrapper(
		&self,
		local: LocalDefId,
		body: &'tcx Body<'tcx>,
		args: GenericArgsRef<'tcx>,
	) -> bool {
		if self.callers.len() > MAX_WRAPPER_DEPTH
			|| self.callers.contains(&local)
			|| args.len() != self.cx.tcx.generics_of(local).count()
		{
			return false;
		}

		let mut callers = self.callers.clone();

		callers.push(local);

		DrainAnalyzer::analyze(
			self.cx,
			self.cx.tcx.typeck(local),
			body,
			callers,
			Some(args),
		)
		.guards_every_exit()
	}

	/// Whether `expr` mentions a local derived from a parameter.
	fn reads_parameter(&self, expr: &'tcx Expr<'tcx>) -> bool {
		let mut seen = HashSet::new();

		locals_in(expr)
			.into_iter()
			.any(|local| self.local_from_parameter(local, &mut seen))
	}

	fn local_from_parameter(&self, local: HirId, seen: &mut HashSet<HirId>) -> bool {
		if self.parameters.contains(&local) {
			return true;
		}

		if !seen.insert(local) {
			return false;
		}

		self.origins.get(&local).is_some_and(|origin| {
			locals_in(origin)
				.into_iter()
				.any(|inner| self.local_from_parameter(inner, seen))
		})
	}

	/// Record the origin of every binding `pattern` introduces.
	fn bind_pattern(&mut self, pattern: &'tcx Pat<'tcx>, origin: &'tcx Expr<'tcx>) {
		pattern.each_binding(|_, binding, _, _| {
			self.origins.insert(binding, origin);
		});
	}

	/// Follow `start`'s value upward until it is enforced, bound, or lost.
	fn enforcement(&self, start: &Expr<'_>, mut value: GuardValue) -> Enforcement {
		let tcx = self.cx.tcx;
		let mut child = start.hir_id;
		let mut short_circuits = Vec::new();

		loop {
			if child == self.body_value {
				return returned(value, short_circuits);
			}

			let parent = match tcx.parent_hir_node(child) {
				Node::Expr(parent) => parent,
				// A block's tail is the block's value.
				Node::Block(block) if block.expr.is_some_and(|tail| tail.hir_id == child) => {
					child = block.hir_id;
					continue;
				}
				Node::LetStmt(local) if local.init.is_some_and(|init| init.hir_id == child) => {
					return self.let_statement(local, value, short_circuits);
				}
				_ => return Enforcement::Unenforced,
			};

			match &parent.kind {
				ExprKind::Call(callee, arguments) => {
					let is_first = arguments.first().is_some_and(|first| first.hir_id == child);

					if is_first && arguments.len() == 1 && is_try_branch(tcx, parent) {
						return match value {
							GuardValue::Fallible => Enforcement::Enforced(short_circuits),
							GuardValue::Flag(_) => Enforcement::Unenforced,
						};
					}

					// `pina::assert(ok, error, message)?` fails when `ok` is false.
					if is_first
						&& matches!(value, GuardValue::Flag(None | Some(false)))
						&& self.is_pina_assert(callee)
					{
						value = GuardValue::Fallible;
					} else if !(is_first
						&& arguments.len() == 1
						&& value == GuardValue::Fallible
						&& self.is_fallible_from(callee, parent, &arguments[0]))
					{
						return Enforcement::Unenforced;
					}
				}
				ExprKind::MethodCall(segment, receiver, arguments, _)
					if receiver.hir_id == child && value == GuardValue::Fallible =>
				{
					let method = segment.ident.name.as_str();

					if !self.is_core_fallible_method(parent) {
						let Some(adapted) =
							self.core_trait_adapter(parent, receiver, method, arguments)
						else {
							return Enforcement::Unenforced;
						};

						value = adapted;
						child = parent.hir_id;
						continue;
					}

					if FAILURE_ABORTING_METHODS.contains(&method) {
						return Enforcement::Enforced(short_circuits);
					}

					if FAILURE_TRUE_PREDICATES.contains(&method) {
						value = GuardValue::Flag(Some(true));
					} else if FAILURE_FALSE_PREDICATES.contains(&method) {
						value = GuardValue::Flag(Some(false));
					} else if method == "or"
						&& arguments
							.first()
							.is_some_and(|fallback| self.is_variant(fallback, &["Err", "None"]))
					{
						// `or(Err(..))` swaps the error but keeps the failure.
					} else if method == "or_else"
						&& arguments
							.first()
							.is_some_and(|fallback| self.always_fails(fallback))
					{
						// `or_else(|e| Err(..))` maps the error but keeps the failure.
					} else if !FAILURE_PRESERVING_METHODS.contains(&method) {
						return Enforcement::Unenforced;
					}
				}
				ExprKind::Unary(UnOp::Not, _) => {
					let GuardValue::Flag(polarity) = value else {
						return Enforcement::Unenforced;
					};

					value = GuardValue::Flag(polarity.map(|failing| !failing));
				}
				ExprKind::Binary(operator, left, right) => {
					let other = if left.hir_id == child { right } else { left };

					// `guard(..) == Ok(..)` is false exactly when the guard failed.
					if value == GuardValue::Fallible {
						if !matches!(operator.node, BinOpKind::Eq | BinOpKind::Ne)
							|| !self.is_variant(peel_reference(other), &["Ok", "Some"])
						{
							return Enforcement::Unenforced;
						}

						value = GuardValue::Flag(Some(operator.node == BinOpKind::Ne));
						child = parent.hir_id;
						continue;
					}

					let GuardValue::Flag(polarity) = value else {
						return Enforcement::Unenforced;
					};

					value = match operator.node {
						// `failing || other` is true whenever the guard failed.
						BinOpKind::Or if polarity != Some(false) => GuardValue::Flag(Some(true)),
						// `passing && other` is false whenever the guard failed.
						BinOpKind::And if polarity != Some(true) => GuardValue::Flag(Some(false)),
						BinOpKind::Eq | BinOpKind::Ne => {
							let Some(literal) = bool_literal(other) else {
								return Enforcement::Unenforced;
							};
							let flips = (!literal) != (operator.node == BinOpKind::Ne);

							GuardValue::Flag(polarity.map(|failing| failing != flips))
						}
						_ => return Enforcement::Unenforced,
					};

					if matches!(operator.node, BinOpKind::And | BinOpKind::Or)
						&& right.hir_id == child
					{
						short_circuits.push(parent.hir_id);
					}
				}
				ExprKind::If(condition, then, otherwise) if condition.hir_id == child => {
					let GuardValue::Flag(polarity) = value else {
						return Enforcement::Unenforced;
					};
					let then_fails = self.branch_fails(then, parent);
					let else_fails =
						otherwise.is_some_and(|otherwise| self.branch_fails(otherwise, parent));
					let enforced = match polarity {
						Some(true) => then_fails,
						Some(false) => else_fails,
						None => then_fails || else_fails,
					};

					return enforced_if(enforced, short_circuits);
				}
				ExprKind::Let(let_expr) if let_expr.init.hir_id == child => {
					if value != GuardValue::Fallible {
						return Enforcement::Unenforced;
					}

					return enforced_if(
						self.if_let_enforces(parent.hir_id, let_expr.pat),
						short_circuits,
					);
				}
				ExprKind::Match(scrutinee, arms, MatchSource::Normal)
					if scrutinee.hir_id == child && value == GuardValue::Fallible =>
				{
					return enforced_if(self.arms_fail(arms, parent), short_circuits);
				}
				ExprKind::Ret(Some(returned_value)) if returned_value.hir_id == child => {
					return returned(value, short_circuits);
				}
				ExprKind::AddrOf(_, _, inner) if inner.hir_id == child => {
					return enforced_if(self.asserted_equal(parent, value), short_circuits);
				}
				// A labeled block's value also comes from its `break`s: one that
				// breaks with a success skips the guard in the tail.
				ExprKind::Block(_, Some(_)) if self.breaks_without_failure(parent) => {
					return Enforcement::Unenforced;
				}
				ExprKind::Block(..)
				| ExprKind::DropTemps(_)
				| ExprKind::Use(..)
				| ExprKind::Type(..) => {}
				_ => return Enforcement::Unenforced,
			}

			child = parent.hir_id;
		}
	}

	/// Whether taking `branch` of `conditional` fails the caller: it returns
	/// `Err`/`None` or panics, or it evaluates to `Err`/`None` and the whole
	/// conditional's value is itself enforced (returned or propagated).
	fn branch_fails(&self, branch: &Expr<'_>, conditional: &Expr<'_>) -> bool {
		self.exit(branch) == Exit::Failure
			|| (self.yields_failure(branch)
				&& matches!(
					self.enforcement(conditional, GuardValue::Fallible),
					Enforcement::Enforced(_)
				))
	}

	/// Whether `expr` evaluates to an `Err`/`None` without leaving first.
	fn yields_failure(&self, expr: &Expr<'_>) -> bool {
		match &expr.kind {
			ExprKind::Block(block, _) => {
				block.stmts.iter().all(|statement| {
					match &statement.kind {
						StmtKind::Expr(inner) | StmtKind::Semi(inner) => {
							self.exit(inner) == Exit::FallThrough
						}
						StmtKind::Let(local) => {
							local.els.is_none()
								&& local
									.init
									.is_none_or(|init| self.exit(init) == Exit::FallThrough)
						}
						StmtKind::Item(_) => true,
					}
				}) && block.expr.is_some_and(|tail| self.yields_failure(tail))
			}
			_ => self.is_variant(expr, &["Err", "None"]),
		}
	}

	/// Whether every arm that can receive the guard's failure fails too.
	///
	/// Arms are tried in order: an unguarded `Err(_)`/`Ok(_)` arm consumes that
	/// side, so a later `_` arm only receives what is left. An arm that binds
	/// the whole scrutinee and returns the binding propagates the failure.
	fn arms_fail(&self, arms: &[rustc_hir::Arm<'_>], matched: &Expr<'_>) -> bool {
		let mut success_left = true;
		let mut failure_left = true;

		for arm in arms {
			let class = classify_pattern(arm.pat);
			let receives_failure = failure_left && class != PatternClass::Success;

			if receives_failure {
				let only_failure = class == PatternClass::Failure || !success_left;
				let propagates = only_failure && self.returns_own_binding(arm);

				if !propagates && !self.branch_fails(arm.body, matched) {
					return false;
				}
			}

			if arm.guard.is_none() && covers_class(arm.pat) {
				match class {
					PatternClass::Success => success_left = false,
					PatternClass::Failure => failure_left = false,
					PatternClass::Both => return true,
				}
			}
		}

		true
	}

	/// Whether `arm` binds the whole scrutinee and hands that binding back as
	/// its `return` value (or, when the `match` is returned, as its value).
	fn returns_own_binding(&self, arm: &rustc_hir::Arm<'_>) -> bool {
		let PatKind::Binding(_, binding, ..) = arm.pat.kind else {
			return false;
		};
		let is_binding = |expr: &Expr<'_>| {
			matches!(
				&expr.kind,
				ExprKind::Path(QPath::Resolved(_, path)) if path.res == Res::Local(binding)
			)
		};
		let mut body = arm.body;

		loop {
			match &body.kind {
				ExprKind::Ret(Some(value)) => return is_binding(value),
				ExprKind::Block(block, None) if block.stmts.is_empty() => {
					let Some(tail) = block.expr else {
						return false;
					};

					body = tail;
				}
				ExprKind::Block(block, None) => {
					let [rest @ .., last] = block.stmts else {
						return false;
					};
					let leaves_early = rest.iter().any(|statement| {
						matches!(
							&statement.kind,
							StmtKind::Expr(inner) | StmtKind::Semi(inner)
								if self.exit(inner) != Exit::FallThrough
						)
					});

					if leaves_early || block.expr.is_some() {
						return false;
					}

					let (StmtKind::Expr(inner) | StmtKind::Semi(inner)) = &last.kind else {
						return false;
					};

					body = inner;
				}
				_ => return false,
			}
		}
	}

	/// Whether a `return` of `value` fails the caller: an `Err`/`None`, or a
	/// call to a local function that can only fail. With `lenient`, a call
	/// the analysis cannot see into also counts, as the name rule did before.
	fn returns_failure(&self, value: &Expr<'_>, lenient: bool) -> bool {
		if let Some(variant) = variant_constructed(self.cx, self.typeck, value) {
			return matches!(variant.as_str(), "Err" | "None");
		}

		let callee = match &value.kind {
			ExprKind::Call(callee, _) => {
				match &callee.kind {
					ExprKind::Path(qpath) => {
						match self.typeck.qpath_res(qpath, callee.hir_id) {
							Res::Def(DefKind::Fn | DefKind::AssocFn, definition) => {
								Some((definition, callee.hir_id))
							}
							_ => None,
						}
					}
					_ => None,
				}
			}
			ExprKind::MethodCall(..) => {
				self.typeck
					.type_dependent_def_id(value.hir_id)
					.map(|definition| (definition, value.hir_id))
			}
			_ => return false,
		};
		let Some((definition, args_owner)) = callee else {
			return false;
		};
		let local_body = self
			.resolve(definition, args_owner)
			.and_then(|(implementation, _)| self.local_body(implementation));

		match local_body {
			Some((local, body)) => {
				returns_constant(self.cx, local, body, GuardValue::Fallible, Outcome::Fails)
			}
			None => lenient,
		}
	}

	/// Whether calling `fallback` (a closure or `Err`/`None` itself) can only
	/// produce a failure.
	fn always_fails(&self, fallback: &Expr<'_>) -> bool {
		match &fallback.kind {
			ExprKind::Closure(closure) => {
				let body = self.cx.tcx.hir_body(closure.body);

				returns_constant(
					self.cx,
					closure.def_id,
					body,
					GuardValue::Fallible,
					Outcome::Fails,
				)
			}
			_ => self.is_variant(fallback, &["Err", "None"]),
		}
	}

	/// Whether a labeled block has a `break` that leaves it without a failure.
	fn breaks_without_failure(&self, block: &Expr<'_>) -> bool {
		breaks_to(block).any(|value| !value.is_some_and(|value| self.returns_failure(value, false)))
	}

	/// Enforcement of a guard value used as a `let` initializer.
	fn let_statement(
		&self,
		local: &'tcx LetStmt<'tcx>,
		value: GuardValue,
		short_circuits: Vec<HirId>,
	) -> Enforcement {
		if let Some(otherwise) = local.els {
			// `let Ok(..) = guard(..) else { .. }` sends the failure to `else`.
			let enforced = value == GuardValue::Fallible
				&& classify_pattern(local.pat) == PatternClass::Success
				&& self.block_exit(otherwise) == Exit::Failure;

			return enforced_if(enforced, short_circuits);
		}

		match local.pat.kind {
			PatKind::Binding(_, binding, _, None) => Enforcement::Bound(binding, value),
			_ => Enforcement::Unenforced,
		}
	}

	/// Whether `if let <pattern> = guard(..)` sends the failure to a failing
	/// branch.
	fn if_let_enforces(&self, let_id: HirId, pattern: &Pat<'_>) -> bool {
		let tcx = self.cx.tcx;
		let mut child = let_id;

		loop {
			let Node::Expr(parent) = tcx.parent_hir_node(child) else {
				return false;
			};

			match &parent.kind {
				ExprKind::DropTemps(_) => child = parent.hir_id,
				ExprKind::If(condition, then, otherwise) if condition.hir_id == child => {
					return match classify_pattern(pattern) {
						PatternClass::Failure => self.branch_fails(then, parent),
						PatternClass::Success => {
							otherwise.is_some_and(|otherwise| self.branch_fails(otherwise, parent))
						}
						PatternClass::Both => false,
					};
				}
				_ => return false,
			}
		}
	}

	/// Whether `&guard(..)` is an operand of `assert_eq!`/`assert_ne!` that
	/// panics when the guard fails.
	fn asserted_equal(&self, reference: &Expr<'_>, value: GuardValue) -> bool {
		let tcx = self.cx.tcx;
		let Node::Expr(tuple) = tcx.parent_hir_node(reference.hir_id) else {
			return false;
		};
		let ExprKind::Tup([left, right]) = &tuple.kind else {
			return false;
		};
		let Node::Expr(matched) = tcx.parent_hir_node(tuple.hir_id) else {
			return false;
		};
		let ExprKind::Match(scrutinee, ..) = &matched.kind else {
			return false;
		};
		let Some(assertion) = bang_macro_name(matched.span) else {
			return false;
		};

		if scrutinee.hir_id != tuple.hir_id || !ASSERT_EQUALITY_MACROS.contains(&assertion.as_str())
		{
			return false;
		}

		let other = peel_reference(if left.hir_id == reference.hir_id {
			right
		} else {
			left
		});
		let is_equality = assertion.as_str() == "assert_eq";

		match value {
			// Execution continues only when the flag equals the literal
			// (`assert_eq!`) or its negation (`assert_ne!`); that value must be
			// the passing one. An unknown polarity accepts either.
			GuardValue::Flag(polarity) => {
				bool_literal(other).is_some_and(|literal| {
					let continuing = if is_equality { literal } else { !literal };

					polarity.is_none_or(|failing| continuing != failing)
				})
			}
			// `assert_eq!(guard(..), Ok(..))` panics on every failure.
			GuardValue::Fallible => is_equality && self.is_variant(other, &["Ok", "Some"]),
		}
	}

	/// Whether `callee` resolves to Pina's `assert(bool, error, message)`.
	fn is_pina_assert(&self, callee: &Expr<'_>) -> bool {
		let ExprKind::Path(qpath) = &callee.kind else {
			return false;
		};
		let Res::Def(DefKind::Fn, definition) = self.typeck.qpath_res(qpath, callee.hir_id) else {
			return false;
		};
		let tcx = self.cx.tcx;

		tcx.crate_name(definition.krate).as_str() == "pina"
			&& matches!(
				tcx.def_path_str(definition).as_str(),
				"pina::assert" | "pina::utils::assert"
			)
	}

	/// Whether a method call resolves to an inherent method of core's
	/// `Result` or `Option`.
	fn is_core_fallible_method(&self, call: &Expr<'_>) -> bool {
		let Some(definition) = self.typeck.type_dependent_def_id(call.hir_id) else {
			return false;
		};
		let tcx = self.cx.tcx;

		// Only an inherent impl has a self type to inspect: a trait method
		// (`Clone::clone`, `PartialEq::eq`) is owned by the trait, which has no
		// `type_of`.
		let Some(implementation) = tcx.inherent_impl_of_assoc(definition) else {
			return false;
		};
		let TyKind::Adt(owner, _) = tcx
			.type_of(implementation)
			.instantiate_identity()
			.skip_norm_wip()
			.kind()
		else {
			return false;
		};

		is_core_item(self.cx, owner.did(), &["Result", "Option"])
	}

	/// The core trait that owns the method `definition` resolves to, if any.
	fn core_trait_of(&self, definition: DefId) -> Option<rustc_span::Symbol> {
		let tcx = self.cx.tcx;
		let owner = tcx.trait_of_assoc(definition)?;

		(tcx.crate_name(owner.krate).as_str() == "core").then(|| tcx.item_name(owner))
	}

	/// How a core trait method applied to a fallible guard value changes it:
	/// `clone`/`into` keep the failure, `eq`/`ne` against `Ok`/`Some` turn it
	/// into a flag with a known polarity.
	fn core_trait_adapter(
		&self,
		call: &Expr<'_>,
		receiver: &Expr<'_>,
		method: &str,
		arguments: &[Expr<'_>],
	) -> Option<GuardValue> {
		let definition = self.typeck.type_dependent_def_id(call.hir_id)?;
		let owner = self.core_trait_of(definition)?;

		match (owner.as_str(), method) {
			("Clone", "clone") | ("Into", "into") if self.keeps_failure(receiver, call) => {
				Some(GuardValue::Fallible)
			}
			("PartialEq", "eq" | "ne") => {
				let [other] = arguments else {
					return None;
				};
				let other = peel_reference(other);

				if !self.is_variant(other, &["Ok", "Some"]) {
					return None;
				}

				// `guard.eq(&Ok(..))` is false exactly when the guard failed.
				Some(GuardValue::Flag(Some(method == "ne")))
			}
			_ => None,
		}
	}

	/// Whether a conversion from `input` to `output` keeps the failure where
	/// enforcement can see it: the output is a `Result`, or the same type.
	/// `Option<Result<..>>` would bury the failure inside a `Some`.
	fn keeps_failure(&self, input: &Expr<'_>, output: &Expr<'_>) -> bool {
		let output_ty = self.typeck.expr_ty(output);

		output_ty == self.typeck.expr_ty(input)
			|| matches!(
				output_ty.kind(),
				TyKind::Adt(definition, _) if is_core_item(self.cx, definition.did(), &["Result"])
			)
	}

	/// Whether `callee` is `From::from`, converting a fallible guard into
	/// another `Result` (or the same type) without dropping its failure.
	fn is_fallible_from(&self, callee: &Expr<'_>, call: &Expr<'_>, input: &Expr<'_>) -> bool {
		let ExprKind::Path(qpath) = &callee.kind else {
			return false;
		};
		let Res::Def(DefKind::AssocFn, definition) = self.typeck.qpath_res(qpath, callee.hir_id)
		else {
			return false;
		};

		self.core_trait_of(definition)
			.is_some_and(|owner| owner.as_str() == "From")
			&& self.keeps_failure(input, call)
	}

	/// Whether `expr` constructs one of core's `Result`/`Option` `variants`.
	fn is_variant(&self, expr: &Expr<'_>, variants: &[&str]) -> bool {
		variant_constructed(self.cx, self.typeck, expr)
			.is_some_and(|variant| variants.contains(&variant.as_str()))
	}

	/// How evaluating `expr` can leave it.
	fn exit(&self, expr: &Expr<'_>) -> Exit {
		match &expr.kind {
			ExprKind::Ret(Some(value)) => {
				// In the handler any `return` skips the drain, so an opaque
				// returned value keeps the old name-rule leniency. In a wrapper it
				// could be a success that swallows the failure.
				if self.returns_failure(value, self.is_handler()) {
					Exit::Failure
				} else {
					Exit::Escape
				}
			}
			ExprKind::Ret(None) | ExprKind::Break(..) | ExprKind::Continue(_) => Exit::Escape,
			ExprKind::Block(block, _) => self.block_exit(block),
			ExprKind::If(condition, then, otherwise) => {
				let condition = self.exit(condition);

				if condition != Exit::FallThrough {
					return condition;
				}

				let otherwise =
					otherwise.map_or(Exit::FallThrough, |otherwise| self.exit(otherwise));

				combine_branches([self.exit(then), otherwise])
			}
			ExprKind::Match(scrutinee, arms, MatchSource::TryDesugar(_)) => {
				// `Err(..)?` always takes the error path.
				let fails = match &scrutinee.kind {
					ExprKind::Call(_, [argument]) => self.is_variant(argument, &["Err", "None"]),
					_ => false,
				};
				let _ = arms;

				if fails {
					Exit::Failure
				} else {
					Exit::FallThrough
				}
			}
			ExprKind::Match(scrutinee, arms, _) => {
				let scrutinee = self.exit(scrutinee);

				if scrutinee != Exit::FallThrough {
					return scrutinee;
				}

				combine_branches(arms.iter().map(|arm| self.exit(arm.body)))
			}
			// `panic!`, `unreachable!`, and other `-> !` calls.
			ExprKind::Call(..) | ExprKind::MethodCall(..)
				if self.typeck.expr_ty(expr).is_never() =>
			{
				Exit::Failure
			}
			ExprKind::DropTemps(inner) | ExprKind::Use(inner, _) | ExprKind::Type(inner, _) => {
				self.exit(inner)
			}
			_ => Exit::FallThrough,
		}
	}

	/// How evaluating a block can leave it: its first leaving statement, or
	/// its tail.
	fn block_exit(&self, block: &Block<'_>) -> Exit {
		for statement in block.stmts {
			let exit = match &statement.kind {
				StmtKind::Expr(expr) | StmtKind::Semi(expr) => self.exit(expr),
				StmtKind::Let(local) => {
					let initializer = local.init.map_or(Exit::FallThrough, |init| self.exit(init));

					match (initializer, local.els) {
						(Exit::FallThrough, Some(otherwise))
							if self.block_exit(otherwise) == Exit::Escape =>
						{
							Exit::Escape
						}
						(exit, _) => exit,
					}
				}
				StmtKind::Item(_) => Exit::FallThrough,
			};

			if exit != Exit::FallThrough {
				return exit;
			}
		}

		block.expr.map_or(Exit::FallThrough, |tail| self.exit(tail))
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
				self.bind_pattern(local.pat, initializer);
			}

			if let Some(else_block) = local.els {
				self.visit_branch_block(else_block);
			}
		} else if let StmtKind::Expr(expr) | StmtKind::Semi(expr) = &statement.kind {
			self.visit_expr(expr);
		}
	}

	/// Forget a pending guard value that may be overwritten through `place`.
	fn forget_pending(&mut self, place: &Expr<'_>) {
		if let Some(binding) = shared::expression_local_binding(place) {
			self.pending.remove(&binding);
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
					let callee = self
						.typeck
						.type_dependent_def_id(expr.hir_id)
						.map(|definition| (definition, expr.hir_id));

					self.record_call(expr, method, receiver_identity, &inputs, callee);
				}

				self.visit_expr(receiver);

				for argument in *arguments {
					self.visit_expr(argument);
				}
			}
			ExprKind::Path(QPath::Resolved(_, path)) => {
				if let Res::Local(binding) = path.res
					&& let Some(value) = self.pending.get(&binding).copied()
				{
					self.enforce(expr, value);
				}
			}
			// A `break` can leave a labeled block before a guard inside it runs.
			ExprKind::Block(block, Some(_)) if breaks_to(expr).next().is_some() => {
				self.visit_branch_block(block);
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
			// `?` desugars into a match whose arms are compiler-generated: the
			// error arm's `return` is the failure itself, not an escape.
			ExprKind::Match(scrutinee, _, MatchSource::TryDesugar(_)) => self.visit_expr(scrutinee),
			ExprKind::Match(scrutinee, arms, _) => {
				self.visit_expr(scrutinee);

				for arm in *arms {
					self.bind_pattern(arm.pat, scrutinee);

					if let Some(guard) = arm.guard {
						self.visit_branch(guard);
					}

					self.visit_branch(arm.body);
				}
			}
			// A loop body may never run, so nothing inside it dominates the
			// code that follows the loop.
			ExprKind::Loop(block, ..) => self.visit_branch_block(block),
			// A closure body runs when it is called, not where it is written,
			// so its calls cannot guard the enclosing drain.
			ExprKind::Closure(_) => {}
			ExprKind::Call(callee, arguments) => {
				if let ExprKind::Path(qpath) = &callee.kind
					&& let Some(name) = qpath_name(qpath)
				{
					// Closures, fn pointers, and generic callables are named by
					// their binding, which says nothing about what they run.
					if let Res::Def(DefKind::Fn | DefKind::AssocFn, definition) =
						self.typeck.qpath_res(qpath, callee.hir_id)
					{
						let inputs: Vec<&'tcx Expr<'tcx>> = arguments.iter().collect();

						self.record_call(
							expr,
							name,
							None,
							&inputs,
							Some((definition, callee.hir_id)),
						);
					}
				}

				self.visit_expr(callee);

				for argument in *arguments {
					self.visit_expr(argument);
				}
			}
			ExprKind::Binary(operator, left, right) => {
				self.visit_expr(left);

				// The right operand of `&&`/`||` may never run.
				if matches!(operator.node, BinOpKind::And | BinOpKind::Or) {
					self.short_circuit_depths
						.insert(expr.hir_id, self.scopes.len());
					self.visit_branch(right);
				} else {
					self.visit_expr(right);
				}
			}
			ExprKind::Assign(left, right, _) | ExprKind::AssignOp(_, left, right) => {
				self.visit_expr(left);
				self.visit_expr(right);
				self.forget_pending(left);
			}
			ExprKind::AddrOf(_, mutability, inner) => {
				self.visit_expr(inner);

				if mutability.is_mut() {
					self.forget_pending(inner);
				}
			}
			ExprKind::Index(base, index, _) => {
				self.visit_expr(base);
				self.visit_expr(index);
			}
			ExprKind::Let(let_expr) => {
				self.visit_expr(let_expr.init);
				self.bind_pattern(let_expr.pat, let_expr.init);
			}
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
			ExprKind::Ret(value) => {
				if let Some(value) = value {
					self.visit_expr(value);
				}

				// A non-error `return` lets a wrapper succeed without reaching
				// a guard written after it.
				if !value.is_some_and(|value| self.returns_failure(value, false)) {
					self.escapes.push(self.guards.len());
				}
			}
			ExprKind::Break(_, Some(inner)) => self.visit_expr(inner),
			ExprKind::Unary(_, inner)
			| ExprKind::Use(inner, _)
			| ExprKind::Cast(inner, _)
			| ExprKind::Type(inner, _)
			| ExprKind::DropTemps(inner)
			| ExprKind::Field(inner, _)
			| ExprKind::Repeat(inner, _)
			| ExprKind::Yield(inner, _)
			| ExprKind::Become(inner)
			| ExprKind::UnsafeBinderCast(_, inner, _) => self.visit_expr(inner),
			_ => {}
		}
	}
}

/// The enforcement of a value that leaves the function as its result: a
/// fallible guard's failure reaches the caller, a `bool` loses its meaning.
fn returned(value: GuardValue, short_circuits: Vec<HirId>) -> Enforcement {
	enforced_if(value == GuardValue::Fallible, short_circuits)
}

fn enforced_if(enforced: bool, short_circuits: Vec<HirId>) -> Enforcement {
	if enforced {
		Enforcement::Enforced(short_circuits)
	} else {
		Enforcement::Unenforced
	}
}

/// Branches that all fail fail together; any escape escapes.
fn combine_branches(exits: impl IntoIterator<Item = Exit>) -> Exit {
	let mut combined = Exit::Failure;

	for exit in exits {
		match exit {
			Exit::Escape => return Exit::Escape,
			Exit::FallThrough => combined = Exit::FallThrough,
			Exit::Failure => {}
		}
	}

	combined
}

/// Which `Result`/`Option` values `pattern` can match. The scrutinee is
/// typed `Result`/`Option`, so a variant pattern names one of its variants.
fn classify_pattern(pattern: &Pat<'_>) -> PatternClass {
	let variant = match &pattern.kind {
		PatKind::TupleStruct(qpath, ..) => qpath_name(qpath),
		PatKind::Expr(expr) => {
			match &expr.kind {
				PatExprKind::Path(qpath) => qpath_name(qpath),
				_ => None,
			}
		}
		PatKind::Binding(.., Some(inner)) => return classify_pattern(inner),
		PatKind::Or(alternatives) => {
			let mut classes = alternatives
				.iter()
				.map(|alternative| classify_pattern(alternative));
			let first = classes.next().unwrap_or(PatternClass::Both);

			return if classes.all(|class| class == first) {
				first
			} else {
				PatternClass::Both
			};
		}
		_ => None,
	};

	match variant {
		Some("Ok" | "Some") => PatternClass::Success,
		Some("Err" | "None") => PatternClass::Failure,
		_ => PatternClass::Both,
	}
}

/// Whether `?` lowered `call` to `Try::branch(..)`.
fn is_try_branch(tcx: rustc_middle::ty::TyCtxt<'_>, call: &Expr<'_>) -> bool {
	matches!(
		tcx.parent_hir_node(call.hir_id),
		Node::Expr(Expr {
			kind: ExprKind::Match(scrutinee, _, MatchSource::TryDesugar(_)),
			..
		}) if scrutinee.hir_id == call.hir_id
	)
}

/// The `bool` literal `expr` spells, if any.
fn bool_literal(expr: &Expr<'_>) -> Option<bool> {
	match &expr.kind {
		ExprKind::Lit(literal) => {
			match literal.node {
				LitKind::Bool(value) => Some(value),
				_ => None,
			}
		}
		ExprKind::DropTemps(inner) => bool_literal(inner),
		_ => None,
	}
}

/// The name of the bang macro `span` was expanded from, if any.
fn bang_macro_name(span: Span) -> Option<rustc_span::Symbol> {
	match span.ctxt().outer_expn_data().kind {
		ExpnKind::Macro(MacroKind::Bang, name) => Some(name),
		_ => None,
	}
}

/// Whether `definition` is one of core's items named in `names`.
fn is_core_item(cx: &LateContext<'_>, definition: DefId, names: &[&str]) -> bool {
	cx.tcx.crate_name(definition.krate).as_str() == "core"
		&& names.contains(&cx.tcx.item_name(definition).as_str())
}

/// The core `Result`/`Option` variant `expr` constructs, if any.
fn variant_constructed(
	cx: &LateContext<'_>,
	typeck: &TypeckResults<'_>,
	expr: &Expr<'_>,
) -> Option<rustc_span::Symbol> {
	let (qpath, path_id) = match &expr.kind {
		ExprKind::Call(callee, _) => {
			match &callee.kind {
				ExprKind::Path(qpath) => (qpath, callee.hir_id),
				_ => return None,
			}
		}
		ExprKind::Path(qpath) => (qpath, expr.hir_id),
		ExprKind::DropTemps(inner) | ExprKind::Use(inner, _) | ExprKind::Type(inner, _) => {
			return variant_constructed(cx, typeck, inner);
		}
		_ => return None,
	};
	let Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) = typeck.qpath_res(qpath, path_id)
	else {
		return None;
	};
	let variant = cx.tcx.parent(constructor);

	is_core_item(cx, cx.tcx.parent(variant), &["Result", "Option"])
		.then(|| cx.tcx.item_name(variant))
}

/// The constant outcome [`returns_constant`] checks a function for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
	/// Only `Ok(..)`/`Some(..)` (or a `bool` literal) can come back, and
	/// nothing inside can fail or panic.
	Succeeds,
	/// Only `Err(..)`/`None` can come back.
	Fails,
}

/// Whether a local function can only produce `outcome`.
///
/// Every returned value (the tail and each `return`) must be the constant
/// outcome, directly or through a `let` binding that is never reassigned. A
/// branch behind a literal `if true`/`if false` that can never run is
/// ignored, so `if false { return Err(..) }` does not make a function able to
/// fail. For [`Outcome::Succeeds`], any `?`, `Err`/`None`, or panic that can
/// run means the function may fail after all.
fn returns_constant(
	cx: &LateContext<'_>,
	local: LocalDefId,
	body: &Body<'_>,
	value: GuardValue,
	outcome: Outcome,
) -> bool {
	struct Bindings<'hir> {
		initializers: HashMap<HirId, &'hir Expr<'hir>>,
		reassigned: HashSet<HirId>,
	}

	impl<'hir> Visitor<'hir> for Bindings<'hir> {
		fn visit_local(&mut self, local: &'hir LetStmt<'hir>) {
			if let (PatKind::Binding(_, binding, _, None), Some(init)) =
				(local.pat.kind, local.init)
			{
				self.initializers.insert(binding, init);
			}

			rustc_hir::intravisit::walk_local(self, local);
		}

		fn visit_expr(&mut self, expr: &'hir Expr<'hir>) {
			let place = match &expr.kind {
				ExprKind::Assign(place, ..) | ExprKind::AssignOp(_, place, _) => Some(*place),
				ExprKind::AddrOf(_, mutability, place) if mutability.is_mut() => Some(*place),
				_ => None,
			};

			if let Some(binding) = place.and_then(shared::expression_local_binding) {
				self.reassigned.insert(binding);
			}

			rustc_hir::intravisit::walk_expr(self, expr);
		}
	}

	struct Scan<'a, 'b, 'hir, 'tcx> {
		cx: &'a LateContext<'tcx>,
		typeck: &'b TypeckResults<'tcx>,
		bindings: Bindings<'hir>,
		value: GuardValue,
		outcome: Outcome,
		varies: bool,
	}

	impl<'hir> Scan<'_, '_, 'hir, '_> {
		fn constant(&self, returned: &'hir Expr<'hir>, depth: usize) -> bool {
			if let ExprKind::Path(QPath::Resolved(_, path)) = &returned.kind
				&& let Res::Local(binding) = path.res
			{
				return depth < 8
					&& !self.bindings.reassigned.contains(&binding)
					&& self
						.bindings
						.initializers
						.get(&binding)
						.is_some_and(|init| self.constant(init, depth + 1));
			}

			match (self.outcome, self.value) {
				(Outcome::Succeeds, GuardValue::Fallible) => {
					variant_constructed(self.cx, self.typeck, returned)
						.is_some_and(|variant| matches!(variant.as_str(), "Ok" | "Some"))
				}
				(Outcome::Succeeds, GuardValue::Flag(_)) => bool_literal(returned).is_some(),
				(Outcome::Fails, _) => {
					variant_constructed(self.cx, self.typeck, returned)
						.is_some_and(|variant| matches!(variant.as_str(), "Err" | "None"))
						|| self.typeck.expr_ty(returned).is_never()
				}
			}
		}
	}

	impl<'hir> Visitor<'hir> for Scan<'_, '_, 'hir, '_> {
		fn visit_expr(&mut self, expr: &'hir Expr<'hir>) {
			// Only the branch a literal condition selects can run.
			if let ExprKind::If(condition, then, otherwise) = &expr.kind
				&& let Some(taken) = bool_literal(condition)
			{
				self.visit_expr(condition);

				if taken {
					self.visit_expr(then);
				} else if let Some(otherwise) = otherwise {
					self.visit_expr(otherwise);
				}

				return;
			}

			let varies = match (&expr.kind, self.outcome) {
				(ExprKind::Ret(Some(returned)), _) => !self.constant(returned, 0),
				(ExprKind::Ret(None), _) => false,
				(ExprKind::Match(_, _, MatchSource::TryDesugar(_)), Outcome::Succeeds) => true,
				(ExprKind::Call(..) | ExprKind::MethodCall(..), Outcome::Succeeds) => {
					self.typeck.expr_ty(expr).is_never()
				}
				(_, Outcome::Succeeds) => {
					variant_constructed(self.cx, self.typeck, expr)
						.is_some_and(|variant| matches!(variant.as_str(), "Err" | "None"))
				}
				(_, Outcome::Fails) => false,
			};

			self.varies |= varies;

			rustc_hir::intravisit::walk_expr(self, expr);
		}
	}

	let mut bindings = Bindings {
		initializers: HashMap::new(),
		reassigned: HashSet::new(),
	};

	bindings.visit_expr(body.value);

	let mut tail = body.value;

	while let ExprKind::Block(block, _) = &tail.kind
		&& let Some(inner) = block.expr
	{
		tail = inner;
	}

	let mut scan = Scan {
		cx,
		typeck: cx.tcx.typeck(local),
		bindings,
		value,
		outcome,
		varies: false,
	};
	let tail_is_block_without_value =
		matches!(&tail.kind, ExprKind::Block(block, _) if block.expr.is_none());

	if !tail_is_block_without_value && !scan.constant(tail, 0) {
		return false;
	}

	scan.visit_expr(body.value);

	!scan.varies
}

/// Whether a local function body can panic on some reachable path, such as
/// through `assert!` or `panic!`. Branches behind a literal `if true`/`if
/// false` that cannot run, and closure bodies, are ignored.
fn body_can_panic(cx: &LateContext<'_>, local: LocalDefId, body: &Body<'_>) -> bool {
	struct PanicFinder<'b, 'tcx> {
		typeck: &'b TypeckResults<'tcx>,
		panics: bool,
	}

	impl<'hir> Visitor<'hir> for PanicFinder<'_, '_> {
		fn visit_expr(&mut self, expr: &'hir Expr<'hir>) {
			if let ExprKind::If(condition, then, otherwise) = &expr.kind
				&& let Some(taken) = bool_literal(condition)
			{
				if taken {
					self.visit_expr(then);
				} else if let Some(otherwise) = otherwise {
					self.visit_expr(otherwise);
				}

				return;
			}

			if matches!(expr.kind, ExprKind::Call(..) | ExprKind::MethodCall(..))
				&& self.typeck.expr_ty(expr).is_never()
			{
				self.panics = true;
			}

			rustc_hir::intravisit::walk_expr(self, expr);
		}
	}

	let mut finder = PanicFinder {
		typeck: cx.tcx.typeck(local),
		panics: false,
	};

	finder.visit_expr(body.value);

	finder.panics
}

/// `expr` without a leading `&`/`&mut`.
fn peel_reference<'a, 'hir>(expr: &'a Expr<'hir>) -> &'a Expr<'hir> {
	match &expr.kind {
		ExprKind::AddrOf(_, _, inner) => inner,
		_ => expr,
	}
}

/// Whether an unguarded arm with this pattern receives every value of its
/// class: `Err(_)`, `Ok(value)`, `None`, `_`, or a plain binding.
fn covers_class(pattern: &Pat<'_>) -> bool {
	match &pattern.kind {
		PatKind::TupleStruct(_, fields, _) => fields.iter().all(|field| irrefutable(field)),
		PatKind::Expr(_) => classify_pattern(pattern) != PatternClass::Both,
		_ => irrefutable(pattern),
	}
}

/// Whether `pattern` matches every value of its type.
fn irrefutable(pattern: &Pat<'_>) -> bool {
	match &pattern.kind {
		PatKind::Wild => true,
		PatKind::Binding(.., sub) => sub.is_none_or(|sub| irrefutable(sub)),
		PatKind::Tuple(fields, _) => fields.iter().all(|field| irrefutable(field)),
		PatKind::Ref(inner, ..) | PatKind::Deref(inner) => irrefutable(inner),
		_ => false,
	}
}

/// The values carried by every `break` that leaves the labeled block
/// `block`, outside nested closures.
fn breaks_to<'hir>(block: &'hir Expr<'hir>) -> std::vec::IntoIter<Option<&'hir Expr<'hir>>> {
	struct BreakFinder<'hir> {
		targets: [HirId; 2],
		values: Vec<Option<&'hir Expr<'hir>>>,
	}

	impl<'hir> Visitor<'hir> for BreakFinder<'hir> {
		fn visit_expr(&mut self, expr: &'hir Expr<'hir>) {
			if let ExprKind::Break(destination, value) = &expr.kind
				&& destination
					.target_id
					.is_ok_and(|target| self.targets.contains(&target))
			{
				self.values.push(*value);
			}

			rustc_hir::intravisit::walk_expr(self, expr);
		}
	}

	let ExprKind::Block(inner, Some(_)) = &block.kind else {
		return Vec::new().into_iter();
	};
	let mut finder = BreakFinder {
		targets: [block.hir_id, inner.hir_id],
		values: Vec::new(),
	};

	finder.visit_block(inner);

	finder.values.into_iter()
}

/// Whether a call name states pause, cap, or circuit-breaker intent.
fn names_guard(name: &str) -> bool {
	let lowercase = name.to_ascii_lowercase();

	GUARD_TERMS.iter().any(|term| lowercase.contains(term))
}

/// The final segment of a path: `check` in `limits::check(..)`.
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

/// Every local binding `expr` reads, outside nested closure bodies.
fn locals_in<'hir>(expr: &'hir Expr<'hir>) -> Vec<HirId> {
	struct LocalFinder {
		locals: Vec<HirId>,
	}

	impl<'hir> Visitor<'hir> for LocalFinder {
		fn visit_path(&mut self, path: &rustc_hir::Path<'hir>, _: HirId) {
			if let Res::Local(binding) = path.res {
				self.locals.push(binding);
			}

			rustc_hir::intravisit::walk_path(self, path);
		}
	}

	let mut finder = LocalFinder { locals: Vec::new() };

	finder.visit_expr(expr);

	finder.locals
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

		let analyzer = DrainAnalyzer::analyze(cx, cx.typeck_results(), body, vec![def_id], None);

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
					"a guard counts only when it reads a parameter-derived receiver or argument \
					 and its failure stops the handler first: `?`, `unwrap`/`expect`, or an \
					 `if`/`match`/`let ... else` whose failing branch returns `Err`/`None` or \
					 panics; a branch that returns `Ok` does not count",
				);
				diag.help(
					"a differently named local wrapper counts when it returns `Result`/`Option` \
					 and enforces a named guard in its outermost scope before any early success \
					 `return` or `break`",
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
