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
	/// Warns when raw sysvar-like accounts are read without successfully
	/// asserting their identity through Pina and the matching canonical sysvar
	/// ID first. Success-side `Result` callbacks do not establish a proof because
	/// they can replace the asserted binding before execution continues.
	///
	/// ### Why is this bad?
	///
	/// Spoofed sysvar accounts can distort rent, clock, and instruction-data
	/// logic. Pinocchio's checked `from_account_view()` and `try_from()` sysvar
	/// loaders perform the identity check while they parse the account, so they
	/// do not require a separate assertion. Known unchecked typed constructors
	/// are rejected where they are called. Deliberate raw parsing needs a local,
	/// reviewed lint allowance after `assert_sysvar()`.
	///
	/// ### Example
	///
	/// ```ignore
	/// // See lints/readme.md for the preferred pattern.
	/// ```
	pub REQUIRE_SYSVAR_ASSERT_BEFORE_SYSVAR_USE,
	Deny,
	"raw sysvar access should be preceded by `assert_sysvar()` on the same account"
}

const TARGET_NEEDLES: &[&str] = &["process", "process_instruction", "instruction", "sysvar"];
const KNOWN_SYSVAR_NAMES: &[&str] = &[
	"clock",
	"epoch_rewards",
	"epoch_schedule",
	"fees",
	"instructions",
	"last_restart_slot",
	"recent_blockhashes",
	"rent",
	"rewards",
	"slot_hashes",
	"slot_history",
	"stake_history",
];
const TRUSTED_SYSVAR_TYPES: &[&str] = &[
	"pinocchio::sysvars::clock::Clock",
	"pinocchio::sysvars::instructions::Instructions",
	"pinocchio::sysvars::rent::Rent",
	"pinocchio::sysvars::slot_hashes::SlotHashes",
];

fn terminal_identifier(value: &str) -> &str {
	value.rsplit(['.', ':']).next().unwrap_or(value)
}

fn sysvar_identifier(value: &str) -> &str {
	let mut terminal = terminal_identifier(value);
	for suffix in ["_account", "_sysvar", "_instructions"] {
		terminal = terminal.strip_suffix(suffix).unwrap_or(terminal);
	}
	terminal
}

fn receiver_sysvar_name(receiver: &str) -> Option<&'static str> {
	let name = sysvar_identifier(receiver);
	KNOWN_SYSVAR_NAMES
		.iter()
		.copied()
		.find(|known| name.eq_ignore_ascii_case(known))
}

fn is_sysvar_receiver(name: &str) -> bool {
	let terminal = terminal_identifier(name).to_ascii_lowercase();
	let normalized = sysvar_identifier(&terminal);
	KNOWN_SYSVAR_NAMES.contains(&normalized)
		|| terminal.ends_with("_sysvar")
		|| terminal.ends_with("_instructions")
}

fn expression_definition(
	cx: &LateContext<'_>,
	expression: &Expr<'_>,
) -> Option<rustc_hir::def_id::DefId> {
	match &expression.kind {
		ExprKind::Path(path) => {
			let Res::Def(_, definition) = cx.qpath_res(path, expression.hir_id) else {
				return None;
			};
			Some(definition)
		}
		ExprKind::Unary(_, inner)
		| ExprKind::Use(inner, _)
		| ExprKind::Type(inner, _)
		| ExprKind::DropTemps(inner)
		| ExprKind::AddrOf(_, _, inner) => expression_definition(cx, inner),
		_ => None,
	}
}

fn is_trusted_sysvar_definition(
	cx: &LateContext<'_>,
	definition: rustc_hir::def_id::DefId,
) -> bool {
	cx.tcx.crate_name(definition.krate).as_str() == "pinocchio"
		&& TRUSTED_SYSVAR_TYPES
			.iter()
			.any(|trusted| cx.tcx.def_path_str(definition).starts_with(trusted))
}

fn canonical_sysvar_name(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<&'static str> {
	let definition = expression_definition(cx, expression)?;
	if cx.tcx.crate_name(definition.krate).as_str() != "pina_sdk_ids" {
		return None;
	}

	let path = cx.tcx.def_path_str(definition);
	let path = path.strip_prefix("pina_sdk_ids::sysvar::")?;
	let (name, item) = path.rsplit_once("::")?;
	if item != "ID" {
		return None;
	}

	KNOWN_SYSVAR_NAMES
		.iter()
		.copied()
		.find(|known| *known == name)
}

#[derive(Clone, Copy)]
struct UncheckedTypedConstructor {
	method: &'static str,
	type_name: &'static str,
}

fn unchecked_typed_constructor(
	cx: &LateContext<'_>,
	expression: &Expr<'_>,
) -> Option<UncheckedTypedConstructor> {
	let ExprKind::Path(path) = &expression.kind else {
		return None;
	};
	let Res::Def(DefKind::AssocFn, definition) = cx.qpath_res(path, expression.hir_id) else {
		return None;
	};
	if cx.tcx.crate_name(definition.krate).as_str() != "pinocchio" {
		return None;
	}

	let definition_path = cx.tcx.def_path_str(definition);
	let method = match cx.tcx.item_name(definition).as_str() {
		"from_bytes" => "from_bytes",
		"from_bytes_unchecked" => "from_bytes_unchecked",
		"new" => "new",
		"new_unchecked" => "new_unchecked",
		_ => return None,
	};
	let type_name = TRUSTED_SYSVAR_TYPES
		.iter()
		.find(|trusted| definition_path.starts_with(**trusted))?
		.rsplit("::")
		.next()?;
	let is_unchecked = match type_name {
		"Clock" | "Rent" => matches!(method, "from_bytes" | "from_bytes_unchecked"),
		"Instructions" => method == "new_unchecked",
		"SlotHashes" => matches!(method, "new" | "new_unchecked"),
		_ => false,
	};

	is_unchecked.then_some(UncheckedTypedConstructor { method, type_name })
}

fn emit_unchecked_typed_constructor(cx: &LateContext<'_>, span: rustc_span::Span) {
	cx.lint(REQUIRE_SYSVAR_ASSERT_BEFORE_SYSVAR_USE, |diag| {
		diag.span(span);
		diag.primary_message(
			"unchecked typed sysvar constructor requires a validated source account",
		);
		diag.help(
			"prefer the sysvar type's checked `from_account_view()` or `try_from()` loader; after \
			 reviewing deliberate raw parsing, use a narrow lint allowance at this constructor",
		);
	});
}

fn emit_unchecked_typed_constructor_value(
	cx: &LateContext<'_>,
	span: rustc_span::Span,
	constructor: UncheckedTypedConstructor,
) {
	cx.lint(REQUIRE_SYSVAR_ASSERT_BEFORE_SYSVAR_USE, |diag| {
		diag.span(span);
		diag.primary_message(format!(
			"`{}::{}` cannot be used as a function value",
			constructor.type_name, constructor.method
		));
		diag.help(
			"call the constructor directly so its unvalidated source is visible, or prefer the \
			 sysvar type's checked account-view loader",
		);
	});
}

fn emit_unchecked_sysvar(cx: &LateContext<'_>, span: rustc_span::Span) {
	cx.lint(REQUIRE_SYSVAR_ASSERT_BEFORE_SYSVAR_USE, |diag| {
		diag.span(span);
		diag.primary_message(
			"raw sysvar access should be preceded by `assert_sysvar()` on the same account",
		);
		diag.help(
			"use the sysvar type's checked `from_account_view()` or `try_from()` loader, or call \
			 `sysvar_account.assert_sysvar(&sysvar::ID)?` before borrowing raw data",
		);
	});
}

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

#[derive(Clone, Debug, Default)]
struct ValidationState {
	places: HashSet<Place>,
}

impl ValidationState {
	fn contains(&self, place: &Place) -> bool {
		self.places.contains(place)
	}

	fn insert(&mut self, place: Place) {
		self.places.insert(place);
	}
}

fn intersect_states(states: &[ValidationState]) -> ValidationState {
	let Some(first) = states.first() else {
		return ValidationState::default();
	};
	let mut intersection = first.clone();
	intersection
		.places
		.retain(|place| states[1..].iter().all(|state| state.contains(place)));
	intersection
}

struct Analyzer<'cx, 'tcx> {
	cx: &'cx LateContext<'tcx>,
}

impl<'tcx> Analyzer<'_, 'tcx> {
	fn place_identity(&self, expression: &Expr<'_>) -> Option<Place> {
		match &expression.kind {
			ExprKind::Field(base, identifier) => {
				Some(Place::Field(
					Box::new(self.place_identity(base)?),
					identifier.name,
				))
			}
			ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
				let Res::Local(binding) = path.res else {
					return None;
				};
				Some(Place::Local(binding))
			}
			ExprKind::MethodCall(_, receiver, ..)
				if shared::is_pina_method(self.cx, expression, &["assert_sysvar"])
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
			ExprKind::Unary(_, inner)
			| ExprKind::Use(inner, _)
			| ExprKind::Type(inner, _)
			| ExprKind::DropTemps(inner)
			| ExprKind::AddrOf(_, _, inner) => self.place_identity(inner),
			_ => None,
		}
	}

	fn expression_can_continue(&self, expression: &Expr<'_>) -> bool {
		!self.cx.typeck_results().expr_ty(expression).is_never()
	}

	fn invalidate(&self, state: &mut ValidationState, assigned: &Place) {
		state
			.places
			.retain(|place| !place.is_same_or_descendant_of(assigned));
	}

	fn checked_sysvar_guard(
		&self,
		expression: &Expr<'_>,
		receiver: &Expr<'_>,
		arguments: &[Expr<'_>],
	) -> Option<Place> {
		if !shared::is_pina_method(self.cx, expression, &["assert_sysvar"])
			|| !shared::result_success_is_required(self.cx, expression)
		{
			return None;
		}

		// Security: matching source spelling would let a local method or ID
		// constant forge a sysvar proof.
		let receiver_name = shared::receiver_name(receiver)?;
		let asserted = canonical_sysvar_name(self.cx, arguments.first()?)?;
		let matches_receiver = receiver_sysvar_name(&receiver_name).map_or_else(
			|| is_sysvar_receiver(&receiver_name),
			|expected| expected == asserted,
		);
		matches_receiver
			.then(|| self.place_identity(receiver))
			.flatten()
	}

	fn method_is_trusted(&self, expression: &Expr<'_>) -> bool {
		self.cx
			.typeck_results()
			.type_dependent_def_id(expression.hir_id)
			.is_some_and(|definition| is_trusted_sysvar_definition(self.cx, definition))
	}

	fn definition_is_ignored(&self, definition: rustc_hir::def_id::DefId) -> bool {
		matches!(
			self.cx.tcx.crate_name(definition.krate).as_str(),
			"alloc" | "core" | "std"
		)
	}

	fn visit_block(&self, block: &'tcx rustc_hir::Block<'tcx>, state: &mut ValidationState) {
		for statement in block.stmts {
			match &statement.kind {
				rustc_hir::StmtKind::Let(local) => {
					if let Some(initializer) = local.init {
						self.visit_expr(initializer, state);
						if let rustc_hir::PatKind::Binding(_, binding, _, None) = local.pat.kind
							&& self
								.place_identity(initializer)
								.is_some_and(|source| state.contains(&source))
						{
							state.insert(Place::Local(binding));
						}
					}
					if let Some(else_block) = local.els {
						let mut else_state = state.clone();
						self.visit_block(else_block, &mut else_state);
					}
				}
				rustc_hir::StmtKind::Expr(expression) | rustc_hir::StmtKind::Semi(expression) => {
					self.visit_expr(expression, state)
				}
				_ => {}
			}
		}

		if let Some(expression) = block.expr {
			self.visit_expr(expression, state);
		}
	}

	fn visit_expr(&self, expression: &'tcx Expr<'tcx>, state: &mut ValidationState) {
		match &expression.kind {
			ExprKind::MethodCall(segment, receiver, arguments, _) => {
				self.visit_expr(receiver, state);
				for argument in *arguments {
					self.visit_expr(argument, state);
				}

				let method = segment.ident.name.as_str();
				if method == "assert_sysvar" {
					if let Some(place) = self.checked_sysvar_guard(expression, receiver, arguments)
					{
						state.insert(place);
					}
					return;
				}
				if self.method_is_trusted(expression) {
					return;
				}
				if self
					.cx
					.typeck_results()
					.type_dependent_def_id(expression.hir_id)
					.is_some_and(|definition| self.definition_is_ignored(definition))
				{
					return;
				}

				let Some(receiver_name) = shared::receiver_name(receiver) else {
					return;
				};
				if is_sysvar_receiver(&receiver_name)
					&& !self
						.place_identity(receiver)
						.is_some_and(|place| state.contains(&place))
				{
					emit_unchecked_sysvar(self.cx, expression.span);
				}
			}
			ExprKind::Call(callee, arguments) => {
				self.visit_expr(callee, state);
				for argument in *arguments {
					self.visit_expr(argument, state);
				}

				let Some(definition) = expression_definition(self.cx, callee) else {
					return;
				};
				if is_trusted_sysvar_definition(self.cx, definition)
					|| self.definition_is_ignored(definition)
				{
					return;
				}
				let method_name = self.cx.tcx.item_name(definition);
				let method = method_name.as_str();
				let looks_like_raw_access =
					matches!(method, "load_current_index" | "load_instruction_at")
						|| KNOWN_SYSVAR_NAMES.contains(&method)
						|| method.ends_with("_sysvar")
						|| method.ends_with("_instructions");
				if looks_like_raw_access
					&& !arguments
						.first()
						.and_then(|argument| self.place_identity(argument))
						.is_some_and(|place| state.contains(&place))
				{
					emit_unchecked_sysvar(self.cx, expression.span);
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
			ExprKind::If(condition, then, else_expression) => {
				self.visit_expr(condition, state);
				let base = state.clone();
				let mut branches = Vec::with_capacity(2);
				let mut then_state = base.clone();
				self.visit_expr(then, &mut then_state);
				if self.expression_can_continue(then) {
					branches.push(then_state);
				}

				if let Some(else_expression) = else_expression {
					let mut else_state = base;
					self.visit_expr(else_expression, &mut else_state);
					if self.expression_can_continue(else_expression) {
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
			ExprKind::Binary(operation, left, right) => {
				self.visit_expr(left, state);
				if matches!(
					operation.node,
					rustc_hir::BinOpKind::And | rustc_hir::BinOpKind::Or
				) {
					let base = state.clone();
					let mut conditional = state.clone();
					self.visit_expr(right, &mut conditional);
					*state = intersect_states(&[base, conditional]);
				} else {
					self.visit_expr(right, state);
				}
			}
			ExprKind::Assign(left, right, _) | ExprKind::AssignOp(_, left, right) => {
				self.visit_expr(left, state);
				self.visit_expr(right, state);
				if let Some(place) = self.place_identity(left) {
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
			ExprKind::Let(let_expression) => self.visit_expr(let_expression.init, state),
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

impl<'tcx> LateLintPass<'tcx> for RequireSysvarAssertBeforeSysvarUse {
	fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
		let Some(constructor) = unchecked_typed_constructor(cx, expression) else {
			return;
		};
		match cx.tcx.parent_hir_node(expression.hir_id) {
			Node::Expr(
				parent @ Expr {
					kind: ExprKind::Call(callee, _),
					..
				},
			) if callee.hir_id == expression.hir_id => {
				emit_unchecked_typed_constructor(cx, parent.span);
			}
			_ => emit_unchecked_typed_constructor_value(cx, expression.span, constructor),
		}
	}

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
		if shared::should_skip_def_path(&def_path) {
			return;
		}

		if !shared::def_path_matches(&def_path, TARGET_NEEDLES) {
			return;
		}

		Analyzer { cx }.visit_expr(body.value, &mut ValidationState::default());
	}
}
