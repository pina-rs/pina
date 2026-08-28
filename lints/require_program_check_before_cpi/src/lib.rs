#![feature(rustc_private)]

extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::HirId;
use rustc_hir::def::Res;
use rustc_hir::intravisit::FnKind;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;

dylint_linting::declare_late_lint! {
	/// ### What it does
	///
	/// Warns when `.invoke()`, `.invoke_signed()`, `.invoke_with_program()`, or
	/// `.invoke_signed_with_program()` is called without a preceding
	/// `assert_address()`, `assert_addresses()`, or `assert_program()` call on a
	/// program account within the same function.
	///
	/// ### Why is this bad?
	///
	/// Without verifying the target program's address, an attacker can
	/// substitute a malicious program that executes arbitrary logic with the
	/// authority and accounts passed to the CPI.
	///
	/// ### Example
	///
	/// Bad:
	/// ```ignore
	/// system::instructions::Transfer { from, to, lamports }.invoke()?;
	/// ```
	///
	/// Good:
	/// ```ignore
	/// system_program.assert_address(&system::ID)?;
	/// system::instructions::Transfer { from, to, lamports }.invoke()?;
	/// ```
	pub REQUIRE_PROGRAM_CHECK_BEFORE_CPI,
	Deny,
	"CPI invocations should be preceded by program address verification"
}

const CPI_METHODS: &[&str] = &[
	"invoke",
	"invoke_signed",
	"invoke_with_program",
	"invoke_signed_with_program",
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

#[derive(Clone, Debug)]
struct PlaceIdentity {
	place: Place,
	name: String,
}

type ValidationState = HashMap<Place, String>;

const TRUSTED_PINA_CPI_TYPES: &[&str] = &[
	"AllocateAccount",
	"AllocateAccountWithBump",
	"CloseAccount",
	"CloseAccountZeroed",
	"CpiContext",
	"CreateAccount",
	"CreateProgramAccount",
	"CreateProgramAccountWithBump",
	"ReallocAccount",
	"ReallocAccountZeroed",
];

fn is_trusted_pina_cpi_type(cx: &LateContext<'_>, receiver: &Expr<'_>) -> bool {
	let receiver_type = cx.typeck_results().expr_ty(receiver).peel_refs();
	let Some(definition) = receiver_type.ty_adt_def() else {
		return false;
	};
	let path = cx.tcx.def_path_str(definition.did());
	is_trusted_pina_cpi_type_path(&path)
}

fn is_trusted_pina_cpi_type_path(path: &str) -> bool {
	path.strip_prefix("pina::cpi::")
		.is_some_and(|name| TRUSTED_PINA_CPI_TYPES.contains(&name))
}

fn place_identity(expr: &Expr<'_>) -> Option<PlaceIdentity> {
	match &expr.kind {
		ExprKind::Field(base, ident) => {
			let base = place_identity(base)?;
			Some(PlaceIdentity {
				place: Place::Field(Box::new(base.place), ident.name),
				name: ident.name.as_str().to_string(),
			})
		}
		ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
			let Res::Local(binding) = path.res else {
				return None;
			};
			let name = path.segments.last()?.ident.name.as_str().to_string();

			Some(PlaceIdentity {
				place: Place::Local(binding),
				name,
			})
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

fn program_argument(method: &str, args: &[Expr<'_>]) -> Option<PlaceIdentity> {
	let index = match method {
		"invoke_with_program" => 0,
		"invoke_signed_with_program" => 1,
		_ => return None,
	};

	args.get(index).and_then(place_identity)
}

fn intersect_states(states: &[ValidationState]) -> ValidationState {
	let Some(first) = states.first() else {
		return ValidationState::new();
	};
	let mut intersection = first.clone();
	intersection.retain(|place, _| states[1..].iter().all(|state| state.contains_key(place)));
	intersection
}

struct Analyzer<'cx, 'tcx> {
	cx: &'cx LateContext<'tcx>,
}

impl<'tcx> Analyzer<'_, 'tcx> {
	fn invalidate(&self, state: &mut ValidationState, assigned: &Place) {
		state.retain(|place, _| !place.is_same_or_descendant_of(assigned));
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
						if let rustc_hir::PatKind::Binding(_, binding, ident, None) = local.pat.kind
							&& let Some(source) = place_identity(init)
							&& state.contains_key(&source.place)
						{
							state.insert(Place::Local(binding), ident.name.as_str().to_string());
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
					if let Some(identity) = place_identity(receiver) {
						state.insert(identity.place, identity.name);
					}
					return;
				}

				if !CPI_METHODS.contains(&method) || is_trusted_pina_cpi_type(self.cx, receiver) {
					return;
				}

				let target = program_argument(method, args);
				let validated = target.as_ref().map_or_else(
					|| {
						state.values().any(|name| {
							name.contains("program")
								|| name.contains("system")
								|| name.contains("token")
						})
					},
					|identity| state.contains_key(&identity.place),
				);

				if !validated {
					self.lint_unchecked_cpi(expr, method);
				}
			}
			ExprKind::Call(callee, args) => {
				self.visit_expr(callee, state);
				for argument in *args {
					self.visit_expr(argument, state);
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
				if let Some(identity) = place_identity(lhs) {
					self.invalidate(state, &identity.place);
				}
			}
			ExprKind::AddrOf(_, rustc_hir::Mutability::Mut, inner) => {
				self.visit_expr(inner, state);
				if let Some(identity) = place_identity(inner) {
					self.invalidate(state, &identity.place);
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

#[cfg(test)]
mod tests {
	#[test]
	fn ui() {
		dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
	}

	#[test]
	fn trusted_pina_cpi_type_path_is_exact() {
		assert!(super::is_trusted_pina_cpi_type_path(
			"pina::cpi::CreateAccount"
		));
		assert!(super::is_trusted_pina_cpi_type_path(
			"pina::cpi::CpiContext"
		));
		assert!(!super::is_trusted_pina_cpi_type_path(
			"pina::cpi::custom::CreateAccount"
		));
		assert!(!super::is_trusted_pina_cpi_type_path(
			"another_crate::pina::cpi::CreateAccount"
		));
	}
}
