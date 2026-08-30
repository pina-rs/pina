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

crate::declare_late_lint! {
	/// ### What it does
	///
	/// Warns when a `CreateProgramAccount` or
	/// `CreateProgramAccountWithBump` builder is invoked without a preceding
	/// `assert_empty()` call on the target account within the same function.
	///
	/// ### Why is this bad?
	///
	/// Without an emptiness check, an attacker can reinitialize an already-
	/// initialized account, overwriting existing state. The `#[account]` macro
	/// does NOT inject reinitialization protection.
	///
	/// ### Example
	///
	/// Bad:
	/// ```ignore
	/// CreateProgramAccount { account: target, payer, owner: &ID, seeds }
	///     .invoke::<State>()?;
	/// ```
	///
	/// Good:
	/// ```ignore
	/// target.assert_empty()?;
	/// CreateProgramAccount { account: target, payer, owner: &ID, seeds }
	///     .invoke::<State>()?;
	/// ```
	pub REQUIRE_EMPTY_BEFORE_INIT,
	Deny,
	"program-account creation should be preceded by `assert_empty()` on the target"
}

const INIT_FUNCTIONS: &[&str] = &["create_program_account", "create_program_account_with_bump"];
const INIT_BUILDERS: &[&str] = &["CreateProgramAccount", "CreateProgramAccountWithBump"];

struct CallInfo {
	span: rustc_span::Span,
	name: String,
	target: Option<AccountPlace>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum AccountPlace {
	Local(HirId),
	Field(Box<Self>, rustc_span::symbol::Symbol),
}

#[derive(Clone)]
struct BuilderInfo {
	name: String,
	target: Option<AccountPlace>,
}

fn account_place(expr: &Expr<'_>) -> Option<AccountPlace> {
	match &expr.kind {
		ExprKind::Field(base, ident) => {
			Some(AccountPlace::Field(
				Box::new(account_place(base)?),
				ident.name,
			))
		}
		ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
			let Res::Local(binding) = path.res else {
				return None;
			};

			Some(AccountPlace::Local(binding))
		}
		ExprKind::DropTemps(inner) | ExprKind::AddrOf(_, _, inner) => account_place(inner),
		_ => None,
	}
}

fn path_name(path: &rustc_hir::QPath<'_>) -> Option<String> {
	match path {
		rustc_hir::QPath::Resolved(_, path) => {
			path.segments
				.last()
				.map(|segment| segment.ident.name.as_str().to_string())
		}
		_ => None,
	}
}

fn init_builder(expr: &Expr<'_>) -> Option<BuilderInfo> {
	match &expr.kind {
		ExprKind::Struct(path, fields, _) => {
			let name = path_name(path)?;
			if !INIT_BUILDERS.contains(&name.as_str()) {
				return None;
			}

			let target = fields
				.iter()
				.find(|field| field.ident.name.as_str() == "account")
				.and_then(|field| account_place(field.expr));

			Some(BuilderInfo { name, target })
		}
		ExprKind::DropTemps(inner) | ExprKind::AddrOf(_, _, inner) => init_builder(inner),
		_ => None,
	}
}

fn collect_calls(body: &rustc_hir::Body<'_>) -> Vec<CallInfo> {
	let mut calls = Vec::new();
	let mut builders = HashMap::new();
	visit_expr(body.value, &mut calls, &mut builders);
	calls
}

fn visit_expr(
	expr: &Expr<'_>,
	calls: &mut Vec<CallInfo>,
	builders: &mut HashMap<HirId, BuilderInfo>,
) {
	match &expr.kind {
		ExprKind::MethodCall(seg, receiver, args, _) => {
			visit_expr(receiver, calls, builders);
			for arg in *args {
				visit_expr(arg, calls, builders);
			}
			let method = seg.ident.name.as_str();
			if matches!(method, "invoke" | "invoke_signed") {
				let inline_builder = init_builder(receiver);
				let bound_builder = account_place(receiver).and_then(|place| {
					let AccountPlace::Local(binding) = place else {
						return None;
					};

					builders.get(&binding).cloned()
				});

				if let Some(builder) = inline_builder.or(bound_builder) {
					calls.push(CallInfo {
						span: expr.span,
						name: builder.name.clone(),
						target: builder.target.clone(),
					});
				} else {
					calls.push(CallInfo {
						span: expr.span,
						name: method.to_string(),
						target: account_place(receiver),
					});
				}
			} else {
				calls.push(CallInfo {
					span: expr.span,
					name: method.to_string(),
					target: account_place(receiver),
				});
			}
		}
		ExprKind::Call(callee, args) => {
			visit_expr(callee, calls, builders);
			for arg in *args {
				visit_expr(arg, calls, builders);
			}
			if let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = &callee.kind
				&& let Some(seg) = path.segments.last()
			{
				let target = args.first().and_then(|argument| account_place(argument));
				calls.push(CallInfo {
					span: expr.span,
					name: seg.ident.name.as_str().to_string(),
					target,
				});
			}
		}
		ExprKind::Block(block, _) => {
			let mut local_bindings = Vec::new();
			for stmt in block.stmts {
				match &stmt.kind {
					rustc_hir::StmtKind::Let(local) => {
						let binding = match local.pat.kind {
							rustc_hir::PatKind::Binding(_, binding, ..) => {
								local_bindings.push(binding);
								Some(binding)
							}
							_ => None,
						};
						if let Some(init) = local.init {
							visit_expr(init, calls, builders);
							if let Some((binding, builder)) = binding.zip(init_builder(init)) {
								builders.insert(binding, builder);
							}
						}
					}
					rustc_hir::StmtKind::Expr(e) | rustc_hir::StmtKind::Semi(e) => {
						visit_expr(e, calls, builders);
					}
					_ => {}
				}
			}
			if let Some(e) = block.expr {
				visit_expr(e, calls, builders);
			}
			for binding in local_bindings {
				builders.remove(&binding);
			}
		}
		ExprKind::Match(scrutinee, arms, _) => {
			visit_expr(scrutinee, calls, builders);
			for arm in *arms {
				visit_expr(arm.body, calls, builders);
			}
		}
		ExprKind::If(cond, then, else_opt) => {
			visit_expr(cond, calls, builders);
			visit_expr(then, calls, builders);
			if let Some(el) = else_opt {
				visit_expr(el, calls, builders);
			}
		}
		ExprKind::Unary(_, e)
		| ExprKind::Cast(e, _)
		| ExprKind::DropTemps(e)
		| ExprKind::AddrOf(_, _, e)
		| ExprKind::Field(e, _) => {
			visit_expr(e, calls, builders);
		}
		ExprKind::Binary(_, lhs, rhs) => {
			visit_expr(lhs, calls, builders);
			visit_expr(rhs, calls, builders);
		}
		ExprKind::Assign(lhs, rhs, _) => {
			visit_expr(rhs, calls, builders);
			visit_expr(lhs, calls, builders);
			if let Some(AccountPlace::Local(binding)) = account_place(lhs) {
				if let Some(builder) = init_builder(rhs) {
					builders.insert(binding, builder);
				} else {
					builders.remove(&binding);
				}
			}
		}
		ExprKind::Tup(exprs) | ExprKind::Array(exprs) => {
			for e in *exprs {
				visit_expr(e, calls, builders);
			}
		}
		_ => {}
	}
}

impl<'tcx> LateLintPass<'tcx> for RequireEmptyBeforeInit {
	fn check_fn(
		&mut self,
		cx: &LateContext<'tcx>,
		_: FnKind<'tcx>,
		_: &'tcx rustc_hir::FnDecl<'tcx>,
		body: &'tcx rustc_hir::Body<'tcx>,
		_: rustc_span::Span,
		_: rustc_hir::def_id::LocalDefId,
	) {
		let calls = collect_calls(body);

		for (i, info) in calls.iter().enumerate() {
			if !INIT_FUNCTIONS.contains(&info.name.as_str())
				&& !INIT_BUILDERS.contains(&info.name.as_str())
			{
				continue;
			}

			let has_empty_check = calls[..i].iter().any(|prev| {
				prev.name == "assert_empty" && prev.target.is_some() && prev.target == info.target
			});

			if !has_empty_check {
				cx.lint(REQUIRE_EMPTY_BEFORE_INIT, |diag| {
					diag.span(info.span);
					diag.primary_message(format!(
						"`{}` invoked without a preceding `assert_empty()` on the target account",
						info.name
					));
					diag.help(
						"add `target_account.assert_empty()?` before calling account creation to \
						 prevent reinitialization",
					);
				});
			}
		}
	}
}
