extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashMap;
use std::collections::HashSet;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::HirId;
use rustc_hir::def::DefKind;
use rustc_hir::def::Res;
use rustc_hir::intravisit::FnKind;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;

crate::declare_late_lint! {
	/// ### What it does
	///
	/// Warns when `.invoke_with_program()` or `.invoke_signed_with_program()` is
	/// called with a dynamic program address that has not passed
	/// `assert_address()`, `assert_addresses()`, or `assert_program()` on the
	/// same account within the same function.
	///
	/// ### Why is this bad?
	///
	/// A dynamic program argument controls the CPI target. Without verifying
	/// that exact argument, an attacker can substitute a malicious program.
	/// Static `.invoke()` and `.invoke_signed()` builders encode their target in
	/// the builder and do not accept a replaceable program argument.
	///
	/// ### Example
	///
	/// Bad:
	/// ```ignore
	/// transfer.invoke_with_program(token_program.address())?;
	/// ```
	///
	/// Good:
	/// ```ignore
	/// token_program.assert_program(&token::ID)?;
	/// transfer.invoke_with_program(token_program.address())?;
	/// ```
	pub REQUIRE_PROGRAM_CHECK_BEFORE_CPI,
	Deny,
	"dynamic CPI targets should be validated before invocation"
}

const DYNAMIC_CPI_METHODS: &[&str] = &["invoke_with_program", "invoke_signed_with_program"];

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
			Self::Invoke => "invoke_with_program",
			Self::InvokeSigned => "invoke_signed_with_program",
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
	cpi_aliases: HashMap<HirId, DynamicCpiMethod>,
}

impl ValidationState {
	fn contains(&self, place: &Place) -> bool {
		self.places.contains(place)
	}

	fn insert(&mut self, place: Place) {
		self.places.insert(place);
	}

	fn new() -> Self {
		Self::default()
	}
}

fn place_identity(expr: &Expr<'_>) -> Option<Place> {
	match &expr.kind {
		ExprKind::Field(base, ident) => {
			let base = place_identity(base)?;
			Some(Place::Field(Box::new(base), ident.name))
		}
		ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
			let Res::Local(binding) = path.res else {
				return None;
			};

			Some(Place::Local(binding))
		}
		ExprKind::MethodCall(segment, receiver, ..) if segment.ident.name.as_str() == "address" => {
			place_identity(receiver)
		}
		ExprKind::Unary(rustc_hir::UnOp::Deref, inner) => place_identity(inner),
		ExprKind::Use(inner, _)
		| ExprKind::Type(inner, _)
		| ExprKind::DropTemps(inner)
		| ExprKind::AddrOf(_, _, inner) => place_identity(inner),
		_ => None,
	}
}

fn program_argument<'a>(method: &str, args: &'a [Expr<'a>]) -> Option<&'a Expr<'a>> {
	let index = match method {
		"invoke_with_program" => 0,
		"invoke_signed_with_program" => 1,
		_ => return None,
	};

	args.get(index)
}

fn dynamic_cpi_method(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<DynamicCpiMethod> {
	let ExprKind::Path(path) = &expr.kind else {
		return None;
	};
	let Res::Def(DefKind::AssocFn, definition) = cx.qpath_res(path, expr.hir_id) else {
		return None;
	};
	let method = cx.tcx.item_name(definition);
	match method.as_str() {
		"invoke_with_program" => Some(DynamicCpiMethod::Invoke),
		"invoke_signed_with_program" => Some(DynamicCpiMethod::InvokeSigned),
		_ => None,
	}
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
	intersection.cpi_aliases.retain(|binding, method| {
		states[1..]
			.iter()
			.all(|state| state.cpi_aliases.get(binding) == Some(method))
	});
	intersection
}

struct Analyzer<'cx, 'tcx> {
	cx: &'cx LateContext<'tcx>,
}

impl<'tcx> Analyzer<'_, 'tcx> {
	fn invalidate(&self, state: &mut ValidationState, assigned: &Place) {
		state
			.places
			.retain(|place| !place.is_same_or_descendant_of(assigned));
		if let Place::Local(binding) = assigned {
			state.cpi_aliases.remove(binding);
		}
	}

	fn lint_unchecked_cpi(&self, expr: &Expr<'_>, method: &str) {
		self.cx.lint(REQUIRE_PROGRAM_CHECK_BEFORE_CPI, |diag| {
			diag.span(expr.span);
			diag.primary_message(format!(
				"`.{}()` called without a preceding program address verification",
				method
			));
			diag.help(
				"add `program_account.assert_address(&expected_id)?` or \
				 `program_account.assert_program(&expected_id)?` before the CPI invocation",
			);
		});
	}

	fn visit_block(&self, block: &'tcx rustc_hir::Block<'tcx>, state: &mut ValidationState) {
		for stmt in block.stmts {
			match &stmt.kind {
				rustc_hir::StmtKind::Let(local) => {
					if let Some(init) = local.init {
						self.visit_expr(init, state);

						// Preserve a proven program identity when an immutable local is
						// derived from the checked account (for example,
						// `let token_program = *account.address()`). The new HIR binding
						// remains independent, so a later assignment invalidates it
						// without affecting the source account's validation.
						if let rustc_hir::PatKind::Binding(_, binding, _, None) = local.pat.kind {
							let inherits_identity = is_static_address(init)
								|| place_identity(init)
									.is_some_and(|source| state.contains(&source));
							if inherits_identity {
								state.insert(Place::Local(binding));
							}

							let cpi_method = dynamic_cpi_method(self.cx, init).or_else(|| {
								let Place::Local(source) = place_identity(init)? else {
									return None;
								};
								state.cpi_aliases.get(&source).copied()
							});
							if let Some(method) = cpi_method {
								state.cpi_aliases.insert(binding, method);
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
					if let Some(place) = place_identity(receiver) {
						state.insert(place);
					}
					return;
				}

				if !DYNAMIC_CPI_METHODS.contains(&method) {
					return;
				}

				let validated = program_argument(method, args).is_some_and(|target| {
					is_static_address(target)
						|| place_identity(target).is_some_and(|place| state.contains(&place))
				});

				if !validated {
					self.lint_unchecked_cpi(expr, method);
				}
			}
			ExprKind::Call(callee, args) => {
				self.visit_expr(callee, state);
				for argument in *args {
					self.visit_expr(argument, state);
				}

				let method = dynamic_cpi_method(self.cx, callee).or_else(|| {
					let ExprKind::Path(path) = &callee.kind else {
						return None;
					};
					let Res::Local(binding) = self.cx.qpath_res(path, callee.hir_id) else {
						return None;
					};
					state.cpi_aliases.get(&binding).copied()
				});
				let Some(method) = method else {
					return;
				};

				let validated = args.get(method.program_index()).is_some_and(|target| {
					is_static_address(target)
						|| place_identity(target).is_some_and(|place| state.contains(&place))
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
					branches.push(branch);
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
				let mut then_state = base.clone();
				self.visit_expr(then, &mut then_state);

				let mut else_state = base;
				if let Some(else_expr) = else_opt {
					self.visit_expr(else_expr, &mut else_state);
				}

				*state = intersect_states(&[then_state, else_state]);
			}
			ExprKind::Loop(block, ..) => {
				let entry = state.clone();
				let mut body_state = entry.clone();
				self.visit_block(block, &mut body_state);
				*state = intersect_states(&[entry, body_state]);
			}
			ExprKind::Binary(operation, lhs, rhs) => {
				self.visit_expr(lhs, state);
				if matches!(
					operation.node,
					rustc_hir::BinOpKind::And | rustc_hir::BinOpKind::Or
				) {
					let mut conditional = state.clone();
					self.visit_expr(rhs, &mut conditional);
				} else {
					self.visit_expr(rhs, state);
				}
			}
			ExprKind::Assign(lhs, rhs, _) | ExprKind::AssignOp(_, lhs, rhs) => {
				self.visit_expr(lhs, state);
				self.visit_expr(rhs, state);
				if let Some(place) = place_identity(lhs) {
					self.invalidate(state, &place);
				}
			}
			ExprKind::AddrOf(_, rustc_hir::Mutability::Mut, inner) => {
				self.visit_expr(inner, state);
				if let Some(place) = place_identity(inner) {
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
