#![feature(rustc_private)]

extern crate rustc_hir;
extern crate rustc_span;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::intravisit::FnKind;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;

dylint_linting::declare_late_lint! {
	/// ### What it does
	///
	/// Warns when `create_program_account()` or
	/// `create_program_account_with_bump()` is called without a preceding
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
	/// create_program_account::<State>(target, payer, &ID, seeds)?;
	/// ```
	///
	/// Good:
	/// ```ignore
	/// target.assert_empty()?;
	/// create_program_account::<State>(target, payer, &ID, seeds)?;
	/// ```
	pub REQUIRE_EMPTY_BEFORE_INIT,
	Warn,
	"calls to `create_program_account*()` should be preceded by `assert_empty()` on the target"
}

const INIT_FUNCTIONS: &[&str] = &["create_program_account", "create_program_account_with_bump"];

struct CallInfo {
	span: rustc_span::Span,
	name: String,
	target: Option<String>,
}

fn receiver_name(expr: &Expr<'_>) -> Option<String> {
	match &expr.kind {
		ExprKind::Field(_, ident) => Some(ident.name.as_str().to_string()),
		ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
			path.segments
				.last()
				.map(|s| s.ident.name.as_str().to_string())
		}
		ExprKind::MethodCall(_, receiver, ..) => receiver_name(receiver),
		_ => None,
	}
}

fn collect_calls(body: &rustc_hir::Body<'_>) -> Vec<CallInfo> {
	let mut calls = Vec::new();
	visit_expr(body.value, &mut calls);
	calls
}

fn visit_expr(expr: &Expr<'_>, calls: &mut Vec<CallInfo>) {
	match &expr.kind {
		ExprKind::MethodCall(seg, receiver, args, _) => {
			visit_expr(receiver, calls);
			for arg in *args {
				visit_expr(arg, calls);
			}
			calls.push(CallInfo {
				span: expr.span,
				name: seg.ident.name.as_str().to_string(),
				target: receiver_name(receiver),
			});
		}
		ExprKind::Call(callee, args) => {
			if let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = &callee.kind {
				if let Some(seg) = path.segments.last() {
					let target = args.first().and_then(receiver_name);
					calls.push(CallInfo {
						span: expr.span,
						name: seg.ident.name.as_str().to_string(),
						target,
					});
				}
			}
			visit_expr(callee, calls);
			for arg in *args {
				visit_expr(arg, calls);
			}
		}
		ExprKind::Block(block, _) => {
			for stmt in block.stmts {
				match &stmt.kind {
					rustc_hir::StmtKind::Let(local) => {
						if let Some(init) = local.init {
							visit_expr(init, calls);
						}
					}
					rustc_hir::StmtKind::Expr(e) | rustc_hir::StmtKind::Semi(e) => {
						visit_expr(e, calls);
					}
					_ => {}
				}
			}
			if let Some(e) = block.expr {
				visit_expr(e, calls);
			}
		}
		ExprKind::Match(scrutinee, arms, _) => {
			visit_expr(scrutinee, calls);
			for arm in *arms {
				visit_expr(arm.body, calls);
			}
		}
		ExprKind::If(cond, then, else_opt) => {
			visit_expr(cond, calls);
			visit_expr(then, calls);
			if let Some(el) = else_opt {
				visit_expr(el, calls);
			}
		}
		ExprKind::Unary(_, e)
		| ExprKind::Cast(e, _)
		| ExprKind::DropTemps(e)
		| ExprKind::AddrOf(_, _, e)
		| ExprKind::Field(e, _) => {
			visit_expr(e, calls);
		}
		ExprKind::Binary(_, lhs, rhs) | ExprKind::Assign(lhs, rhs, _) => {
			visit_expr(lhs, calls);
			visit_expr(rhs, calls);
		}
		ExprKind::Tup(exprs) | ExprKind::Array(exprs) => {
			for e in *exprs {
				visit_expr(e, calls);
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
			if !INIT_FUNCTIONS.contains(&info.name.as_str()) {
				continue;
			}

			let has_empty_check = calls[..i].iter().any(|prev| {
				prev.name == "assert_empty" && prev.target.is_some() && prev.target == info.target
			});

			if !has_empty_check {
				cx.lint(REQUIRE_EMPTY_BEFORE_INIT, |diag| {
					diag.span(info.span);
					diag.primary_message(format!(
						"`{}()` called without a preceding `assert_empty()` on the target account",
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
