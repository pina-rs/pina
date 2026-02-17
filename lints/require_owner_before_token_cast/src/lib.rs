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
	/// Warns when `as_token_mint()`, `as_token_account()`, `as_token_2022_mint()`,
	/// or `as_token_2022_account()` is called without a preceding `assert_owner()`
	/// or `assert_owners()` call on the same receiver within the same function.
	///
	/// ### Why is this bad?
	///
	/// These methods perform layout casts without ownership verification. An
	/// attacker can create a fake account with arbitrary token data owned by a
	/// different program. Without an owner check, the program trusts spoofed data.
	///
	/// ### Example
	///
	/// Bad:
	/// ```ignore
	/// let token = account.as_token_account()?;
	/// ```
	///
	/// Good:
	/// ```ignore
	/// account.assert_owners(&SPL_PROGRAM_IDS)?;
	/// let token = account.as_token_account()?;
	/// ```
	pub REQUIRE_OWNER_BEFORE_TOKEN_CAST,
	Warn,
	"calls to `as_token_*()` methods should be preceded by `assert_owner()` or `assert_owners()`"
}

const TOKEN_CAST_METHODS: &[&str] = &[
	"as_token_mint",
	"as_token_account",
	"as_token_2022_mint",
	"as_token_2022_account",
];

const OWNER_CHECK_METHODS: &[&str] = &["assert_owner", "assert_owners"];

/// Extracts the "root" field identifier from a receiver expression chain.
fn receiver_ident_name(expr: &Expr<'_>) -> Option<rustc_span::Symbol> {
	match &expr.kind {
		ExprKind::Field(_, ident) => Some(ident.name),
		ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
			path.segments.last().map(|seg| seg.ident.name)
		}
		ExprKind::MethodCall(_, receiver, ..) => receiver_ident_name(receiver),
		_ => None,
	}
}

/// Collected call info: (span, method_name, receiver_ident).
struct CallInfo {
	span: rustc_span::Span,
	method: String,
	receiver: Option<rustc_span::Symbol>,
}

fn collect_method_calls(body: &rustc_hir::Body<'_>) -> Vec<CallInfo> {
	let mut calls = Vec::new();
	collect_from_expr(body.value, &mut calls);
	calls
}

fn collect_from_expr(expr: &Expr<'_>, calls: &mut Vec<CallInfo>) {
	match &expr.kind {
		ExprKind::MethodCall(path_segment, receiver, args, _) => {
			collect_from_expr(receiver, calls);
			for arg in *args {
				collect_from_expr(arg, calls);
			}
			calls.push(CallInfo {
				span: expr.span,
				method: path_segment.ident.name.as_str().to_string(),
				receiver: receiver_ident_name(receiver),
			});
		}
		ExprKind::Block(block, _) => {
			for stmt in block.stmts {
				match &stmt.kind {
					rustc_hir::StmtKind::Let(local) => {
						if let Some(init) = local.init {
							collect_from_expr(init, calls);
						}
					}
					rustc_hir::StmtKind::Expr(e) | rustc_hir::StmtKind::Semi(e) => {
						collect_from_expr(e, calls);
					}
					_ => {}
				}
			}
			if let Some(expr) = block.expr {
				collect_from_expr(expr, calls);
			}
		}
		ExprKind::Call(callee, args) => {
			collect_from_expr(callee, calls);
			for arg in *args {
				collect_from_expr(arg, calls);
			}
		}
		ExprKind::Match(scrutinee, arms, _) => {
			collect_from_expr(scrutinee, calls);
			for arm in *arms {
				collect_from_expr(arm.body, calls);
			}
		}
		ExprKind::If(cond, then, else_opt) => {
			collect_from_expr(cond, calls);
			collect_from_expr(then, calls);
			if let Some(el) = else_opt {
				collect_from_expr(el, calls);
			}
		}
		ExprKind::Unary(_, e)
		| ExprKind::Cast(e, _)
		| ExprKind::DropTemps(e)
		| ExprKind::AddrOf(_, _, e)
		| ExprKind::Field(e, _) => {
			collect_from_expr(e, calls);
		}
		ExprKind::Binary(_, lhs, rhs) | ExprKind::Assign(lhs, rhs, _) => {
			collect_from_expr(lhs, calls);
			collect_from_expr(rhs, calls);
		}
		ExprKind::Tup(exprs) | ExprKind::Array(exprs) => {
			for e in *exprs {
				collect_from_expr(e, calls);
			}
		}
		_ => {}
	}
}

impl<'tcx> LateLintPass<'tcx> for RequireOwnerBeforeTokenCast {
	fn check_fn(
		&mut self,
		cx: &LateContext<'tcx>,
		_: FnKind<'tcx>,
		_: &'tcx rustc_hir::FnDecl<'tcx>,
		body: &'tcx rustc_hir::Body<'tcx>,
		_: rustc_span::Span,
		_: rustc_hir::def_id::LocalDefId,
	) {
		let calls = collect_method_calls(body);

		for (i, info) in calls.iter().enumerate() {
			if !TOKEN_CAST_METHODS.contains(&info.method.as_str()) {
				continue;
			}

			let has_owner_check = calls[..i].iter().any(|prev| {
				OWNER_CHECK_METHODS.contains(&prev.method.as_str())
					&& prev.receiver.is_some()
					&& prev.receiver == info.receiver
			});

			if !has_owner_check {
				cx.lint(REQUIRE_OWNER_BEFORE_TOKEN_CAST, |diag| {
					diag.span(info.span);
					diag.primary_message(format!(
						"`{}()` called without a preceding `assert_owner()` or `assert_owners()` \
						 on the same account",
						info.method
					));
					diag.help(format!(
						"add `account.assert_owner(&expected_owner)?` or \
						 `account.assert_owners(&PROGRAM_IDS)?` before calling `{}()`",
						info.method
					));
				});
			}
		}
	}
}
