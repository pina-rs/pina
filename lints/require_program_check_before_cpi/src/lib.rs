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
	/// Warns when `.invoke()` or `.invoke_signed()` is called without a
	/// preceding `assert_address()`, `assert_addresses()`, or
	/// `assert_program()` call on a program account within the same function.
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
	Warn,
	"CPI invocations should be preceded by program address verification"
}

const CPI_METHODS: &[&str] = &["invoke", "invoke_signed"];

const PROGRAM_CHECK_METHODS: &[&str] = &["assert_address", "assert_addresses", "assert_program"];

struct CallInfo {
	span: rustc_span::Span,
	method: String,
	receiver: Option<String>,
}

fn receiver_ident(expr: &Expr<'_>) -> Option<String> {
	match &expr.kind {
		ExprKind::Field(_, ident) => Some(ident.name.as_str().to_string()),
		ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
			path.segments
				.last()
				.map(|s| s.ident.name.as_str().to_string())
		}
		ExprKind::MethodCall(_, receiver, ..) => receiver_ident(receiver),
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
				method: seg.ident.name.as_str().to_string(),
				receiver: receiver_ident(receiver),
			});
		}
		ExprKind::Call(callee, args) => {
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
		let calls = collect_calls(body);

		for (i, info) in calls.iter().enumerate() {
			if !CPI_METHODS.contains(&info.method.as_str()) {
				continue;
			}

			let has_program_check = calls[..i].iter().any(|prev| {
				PROGRAM_CHECK_METHODS.contains(&prev.method.as_str())
					&& prev.receiver.as_ref().is_some_and(|r| {
						r.contains("program") || r.contains("system") || r.contains("token")
					})
			});

			if !has_program_check {
				cx.lint(REQUIRE_PROGRAM_CHECK_BEFORE_CPI, |diag| {
					diag.span(info.span);
					diag.primary_message(format!(
						"`.{}()` called without a preceding program address verification",
						info.method
					));
					diag.help(
						"add `program_account.assert_address(&expected_id)?` or \
						 `program_account.assert_program(&expected_id)?` before the CPI invocation",
					);
				});
			}
		}
	}
}
