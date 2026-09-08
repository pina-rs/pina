extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashSet;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::HirId;
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
	/// Warns when `.invoke_with_unverified_program()` or
	/// `.invoke_signed_with_unverified_program()` is called with a dynamic
	/// program address that has not passed
	/// `assert_address()`, `assert_addresses()`, or `assert_program()` on the
	/// same account within the same function against a compile-time program ID.
	/// The resolved Pina assertion must succeed on every continuing path without
	/// a success-side callback. The lint also rejects taking either unverified CPI
	/// method as a function value.
	///
	/// ### Why is this bad?
	///
	/// A dynamic program argument controls the CPI target. Without verifying
	/// that exact argument, an attacker can substitute a malicious program.
	/// Discarding the assertion `Result`, inspecting failure, or checking only
	/// one branch does not establish a proof. Success-side adapters such as
	/// `map()`, `and_then()`, and `inspect()` do not establish a proof because
	/// their callbacks can replace the validated binding before execution
	/// continues. An instruction argument is not a trusted expected ID: comparing
	/// two attacker-controlled values proves consistency, not authenticity.
	/// Static `.invoke()` and `.invoke_signed()` builders encode their target in
	/// the builder and do not accept a replaceable program argument. Restricting
	/// unverified calls to direct method or UFCS syntax keeps the target proof
	/// local and reviewable.
	///
	/// ### Example
	///
	/// Bad:
	/// ```ignore
	/// transfer.invoke_with_unverified_program(token_program.address())?;
	/// ```
	///
	/// Good:
	/// ```ignore
	/// transfer.invoke_with_program(token_program.address())?;
	/// // Or, when deliberately using the unchecked API:
	/// token_program.assert_program(&token::ID)?;
	/// transfer.invoke_with_unverified_program(token_program.address())?;
	/// ```
	pub REQUIRE_PROGRAM_CHECK_BEFORE_CPI,
	Deny,
	"dynamic CPI targets should be validated before invocation"
}

const DYNAMIC_CPI_METHODS: &[&str] = &[
	"invoke_with_unverified_program",
	"invoke_signed_with_unverified_program",
];

const PROGRAM_CHECK_METHODS: &[&str] = &["assert_address", "assert_addresses", "assert_program"];

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Place {
	Local(HirId),
	Field(Box<Self>, rustc_span::Symbol),
}

impl Place {
	fn is_same_or_descendant_of(&self, other: &Self) -> bool {
		self == other
			|| match self {
				Self::Field(base, _) => base.is_same_or_descendant_of(other),
				Self::Local(_) => false,
			}
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DynamicCpiMethod {
	Invoke,
	InvokeSigned,
}

impl DynamicCpiMethod {
	const fn name(self) -> &'static str {
		match self {
			Self::Invoke => "invoke_with_unverified_program",
			Self::InvokeSigned => "invoke_signed_with_unverified_program",
		}
	}

	const fn program_index(self) -> usize {
		match self {
			Self::Invoke => 1,
			Self::InvokeSigned => 2,
		}
	}
}

#[derive(Clone, Debug, Default)]
struct ValidationState {
	places: HashSet<Place>,
	trusted_ids: HashSet<Place>,
}

impl ValidationState {
	fn contains(&self, place: &Place) -> bool {
		self.places.contains(place)
	}

	fn insert(&mut self, place: Place) {
		self.places.insert(place);
	}

	fn contains_trusted_id(&self, place: &Place) -> bool {
		self.trusted_ids.contains(place)
	}

	fn insert_trusted_id(&mut self, place: Place) {
		self.trusted_ids.insert(place);
	}

	fn new() -> Self {
		Self::default()
	}
}

fn program_argument<'a>(method: &str, args: &'a [Expr<'a>]) -> Option<&'a Expr<'a>> {
	let index = match method {
		"invoke_with_unverified_program" => 0,
		"invoke_signed_with_unverified_program" => 1,
		_ => return None,
	};

	args.get(index)
}

fn is_pinocchio_token_crate(cx: &LateContext<'_>, definition: rustc_hir::def_id::DefId) -> bool {
	matches!(
		cx.tcx.crate_name(definition.krate).as_str(),
		"pinocchio_token" | "pinocchio_token_2022"
	)
}

fn dynamic_cpi_method_from_definition(
	cx: &LateContext<'_>,
	definition: rustc_hir::def_id::DefId,
) -> Option<DynamicCpiMethod> {
	if !is_pinocchio_token_crate(cx, definition) {
		return None;
	}

	match cx.tcx.item_name(definition).as_str() {
		"invoke_with_unverified_program" => Some(DynamicCpiMethod::Invoke),
		"invoke_signed_with_unverified_program" => Some(DynamicCpiMethod::InvokeSigned),
		_ => None,
	}
}

fn dynamic_cpi_method(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<DynamicCpiMethod> {
	let ExprKind::Path(path) = &expr.kind else {
		return None;
	};
	let Res::Def(DefKind::AssocFn, definition) = cx.qpath_res(path, expr.hir_id) else {
		return None;
	};
	dynamic_cpi_method_from_definition(cx, definition)
}

fn is_static_address(expr: &Expr<'_>) -> bool {
	match &expr.kind {
		ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
			match path.res {
				Res::Def(DefKind::Const | DefKind::AssocConst, _) => true,
				Res::Def(DefKind::Static { mutability, .. }, _) => {
					mutability == rustc_hir::Mutability::Not
				}
				_ => false,
			}
		}
		ExprKind::Unary(_, inner)
		| ExprKind::Use(inner, _)
		| ExprKind::Type(inner, _)
		| ExprKind::DropTemps(inner)
		| ExprKind::AddrOf(_, _, inner) => is_static_address(inner),
		_ => false,
	}
}

fn intersect_states(states: &[ValidationState]) -> ValidationState {
	let Some(first) = states.first() else {
		return ValidationState::new();
	};
	let mut intersection = first.clone();
	intersection
		.places
		.retain(|place| states[1..].iter().all(|state| state.contains(place)));
	intersection.trusted_ids.retain(|place| {
		states[1..]
			.iter()
			.all(|state| state.contains_trusted_id(place))
	});
	intersection
}

struct Analyzer<'cx, 'tcx> {
	cx: &'cx LateContext<'tcx>,
}

impl<'tcx> Analyzer<'_, 'tcx> {
	fn place_identity(&self, expression: &Expr<'_>) -> Option<Place> {
		match &expression.kind {
			ExprKind::Field(base, identifier) => {
				let base = self.place_identity(base)?;
				Some(Place::Field(Box::new(base), identifier.name))
			}
			ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
				let Res::Local(binding) = path.res else {
					return None;
				};

				Some(Place::Local(binding))
			}
			ExprKind::MethodCall(segment, receiver, ..)
				if segment.ident.name.as_str() == "address"
					|| shared::is_pina_method(self.cx, expression, PROGRAM_CHECK_METHODS)
					|| shared::is_result_method(
						self.cx,
						expression,
						&["expect", "inspect_err", "map_err", "unwrap"],
					) =>
			{
				self.place_identity(receiver)
			}
			ExprKind::Match(scrutinee, _, rustc_hir::MatchSource::TryDesugar(_)) => {
				self.place_identity(scrutinee)
			}
			ExprKind::Call(..) => {
				self.place_identity(shared::try_branch_argument(self.cx, expression)?)
			}
			ExprKind::Unary(rustc_hir::UnOp::Deref, inner) => self.place_identity(inner),
			ExprKind::Use(inner, _)
			| ExprKind::Type(inner, _)
			| ExprKind::DropTemps(inner)
			| ExprKind::AddrOf(_, _, inner) => self.place_identity(inner),
			_ => None,
		}
	}

	fn invalidate(&self, state: &mut ValidationState, assigned: &Place) {
		state
			.places
			.retain(|place| !place.is_same_or_descendant_of(assigned));
		state
			.trusted_ids
			.retain(|place| !place.is_same_or_descendant_of(assigned));
	}

	fn is_trusted_program_id(&self, expression: &Expr<'_>, state: &ValidationState) -> bool {
		if is_static_address(expression)
			|| self
				.place_identity(expression)
				.is_some_and(|place| state.contains_trusted_id(&place))
		{
			return true;
		}

		match &expression.kind {
			ExprKind::Array(expressions) => {
				expressions
					.iter()
					.all(|expression| self.is_trusted_program_id(expression, state))
			}
			ExprKind::Repeat(inner, _) => self.is_trusted_program_id(inner, state),
			ExprKind::Unary(_, inner)
			| ExprKind::Use(inner, _)
			| ExprKind::Type(inner, _)
			| ExprKind::DropTemps(inner)
			| ExprKind::AddrOf(_, _, inner) => self.is_trusted_program_id(inner, state),
			_ => false,
		}
	}

	fn assertion_uses_trusted_id(&self, arguments: &[Expr<'_>], state: &ValidationState) -> bool {
		arguments
			.first()
			.is_some_and(|expected| self.is_trusted_program_id(expected, state))
	}

	fn lint_unchecked_cpi(&self, expr: &Expr<'_>, method: &str) {
		let verified_method = match method {
			"invoke_signed_with_unverified_program" => "invoke_signed_with_program",
			_ => "invoke_with_program",
		};
		self.cx.lint(REQUIRE_PROGRAM_CHECK_BEFORE_CPI, |diag| {
			diag.span(expr.span);
			diag.primary_message(format!(
				"`.{}()` called without a preceding program address verification",
				method
			));
			diag.help(format!(
				"use the verified `{verified_method}` variant, or call \
				 `program_account.assert_program(&trusted_program_id)?` before the unverified CPI \
				 invocation; the expected ID must come from a const or immutable static"
			));
		});
	}

	fn lint_unverified_function_value(&self, expr: &Expr<'_>, method: DynamicCpiMethod) {
		let verified_method = match method {
			DynamicCpiMethod::Invoke => "invoke_with_program",
			DynamicCpiMethod::InvokeSigned => "invoke_signed_with_program",
		};
		self.cx.lint(REQUIRE_PROGRAM_CHECK_BEFORE_CPI, |diag| {
			diag.span(expr.span);
			diag.primary_message(format!(
				"`{}` cannot be used as a function value",
				method.name()
			));
			diag.help(format!(
				"invoke the unverified CPI function directly so the lint can prove its exact \
				 program target, or use the verified `{verified_method}` variant"
			));
		});
	}

	fn expression_can_continue(&self, expr: &Expr<'_>) -> bool {
		!self.cx.typeck_results().expr_ty(expr).is_never()
	}

	fn visit_block(&self, block: &'tcx rustc_hir::Block<'tcx>, state: &mut ValidationState) {
		for stmt in block.stmts {
			match &stmt.kind {
				rustc_hir::StmtKind::Let(local) => {
					if let Some(init) = local.init {
						self.visit_expr(init, state);

						// Preserve authenticated account aliases separately from aliases
						// whose values originate at compile time. Conflating the two would
						// let an authenticated account become trusted expected-ID provenance.
						if let rustc_hir::PatKind::Binding(_, binding, _, None) = local.pat.kind {
							let source = self.place_identity(init);
							let inherits_identity = is_static_address(init)
								|| source.as_ref().is_some_and(|source| state.contains(source));
							if inherits_identity {
								state.insert(Place::Local(binding));
							}
							if self.is_trusted_program_id(init, state) {
								state.insert_trusted_id(Place::Local(binding));
							}
						}
					}
					if let Some(else_block) = local.els {
						let mut else_state = state.clone();
						self.visit_block(else_block, &mut else_state);
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

	fn visit_expr(&self, expr: &'tcx Expr<'tcx>, state: &mut ValidationState) {
		match &expr.kind {
			ExprKind::MethodCall(segment, receiver, args, _) => {
				self.visit_expr(receiver, state);
				for argument in *args {
					self.visit_expr(argument, state);
				}

				let method = segment.ident.name.as_str();
				if PROGRAM_CHECK_METHODS.contains(&method) {
					if shared::is_pina_method(self.cx, expr, PROGRAM_CHECK_METHODS)
						&& shared::result_success_is_required(self.cx, expr)
						&& self.assertion_uses_trusted_id(args, state)
						&& let Some(place) = self.place_identity(receiver)
					{
						state.insert(place);
					}
					return;
				}

				if !DYNAMIC_CPI_METHODS.contains(&method) {
					return;
				}
				let Some(definition) = self.cx.typeck_results().type_dependent_def_id(expr.hir_id)
				else {
					return;
				};
				if dynamic_cpi_method_from_definition(self.cx, definition).is_none() {
					return;
				}

				let validated = program_argument(method, args).is_some_and(|target| {
					is_static_address(target)
						|| self
							.place_identity(target)
							.is_some_and(|place| state.contains(&place))
				});

				if !validated {
					self.lint_unchecked_cpi(expr, method);
				}
			}
			ExprKind::Call(callee, args) => {
				let direct_method = dynamic_cpi_method(self.cx, callee);
				if direct_method.is_none() {
					self.visit_expr(callee, state);
				}
				for argument in *args {
					self.visit_expr(argument, state);
				}

				let Some(method) = direct_method else {
					return;
				};
				let validated = args.get(method.program_index()).is_some_and(|target| {
					is_static_address(target)
						|| self
							.place_identity(target)
							.is_some_and(|place| state.contains(&place))
				});
				if !validated {
					self.lint_unchecked_cpi(expr, method.name());
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
					if self.expression_can_continue(arm.body) {
						branches.push(branch);
					}
				}

				*state = if branches.is_empty() {
					base
				} else {
					intersect_states(&branches)
				};
			}
			ExprKind::If(condition, then, else_opt) => {
				self.visit_expr(condition, state);
				let base = state.clone();
				let mut branches = Vec::with_capacity(2);
				let mut then_state = base.clone();
				self.visit_expr(then, &mut then_state);
				if self.expression_can_continue(then) {
					branches.push(then_state);
				}

				if let Some(else_expr) = else_opt {
					let mut else_state = base;
					self.visit_expr(else_expr, &mut else_state);
					if self.expression_can_continue(else_expr) {
						branches.push(else_state);
					}
				} else {
					branches.push(base);
				}

				if !branches.is_empty() {
					*state = intersect_states(&branches);
				}
			}
			ExprKind::Loop(block, ..) => {
				let entry = state.clone();
				let mut body_state = entry.clone();
				self.visit_block(block, &mut body_state);
				*state = intersect_states(&[entry, body_state]);
			}
			ExprKind::Closure(closure) => {
				// A closure can run after this point and replace a captured binding.
				// Keep only proofs that survive both the no-call and called paths;
				// proofs established inside the closure cannot escape either.
				let entry = state.clone();
				let mut body_state = entry.clone();
				let body = self.cx.tcx.hir_body(closure.body);
				self.visit_expr(body.value, &mut body_state);
				*state = intersect_states(&[entry, body_state]);
			}
			ExprKind::Binary(operation, lhs, rhs) => {
				self.visit_expr(lhs, state);
				if matches!(
					operation.node,
					rustc_hir::BinOpKind::And | rustc_hir::BinOpKind::Or
				) {
					let base = state.clone();
					let mut conditional = state.clone();
					self.visit_expr(rhs, &mut conditional);
					*state = intersect_states(&[base, conditional]);
				} else {
					self.visit_expr(rhs, state);
				}
			}
			ExprKind::Assign(lhs, rhs, _) | ExprKind::AssignOp(_, lhs, rhs) => {
				self.visit_expr(lhs, state);
				self.visit_expr(rhs, state);
				if let Some(place) = self.place_identity(lhs) {
					self.invalidate(state, &place);
				}
			}
			ExprKind::AddrOf(_, rustc_hir::Mutability::Mut, inner) => {
				self.visit_expr(inner, state);
				if let Some(place) = self.place_identity(inner) {
					self.invalidate(state, &place);
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
			| ExprKind::UnsafeBinderCast(_, inner, _) => self.visit_expr(inner, state),
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
			ExprKind::Break(_, value) | ExprKind::Ret(value) => {
				if let Some(value) = value {
					self.visit_expr(value, state);
				}
			}
			_ => {}
		}
	}
}

impl<'tcx> LateLintPass<'tcx> for RequireProgramCheckBeforeCpi {
	fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
		let Some(method) = dynamic_cpi_method(cx, expr) else {
			return;
		};
		let is_direct_callee = matches!(
			cx.tcx.parent_hir_node(expr.hir_id),
			Node::Expr(Expr {
				kind: ExprKind::Call(callee, _),
				..
			}) if callee.hir_id == expr.hir_id
		);
		if !is_direct_callee {
			Analyzer { cx }.lint_unverified_function_value(expr, method);
		}
	}

	fn check_fn(
		&mut self,
		cx: &LateContext<'tcx>,
		_: FnKind<'tcx>,
		_: &'tcx rustc_hir::FnDecl<'tcx>,
		body: &'tcx rustc_hir::Body<'tcx>,
		_: rustc_span::Span,
		_: rustc_hir::def_id::LocalDefId,
	) {
		Analyzer { cx }.visit_expr(body.value, &mut ValidationState::new());
	}
}
