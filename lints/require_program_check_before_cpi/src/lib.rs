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

struct CallInfo {
	span: rustc_span::Span,
	method: String,
	receiver: Option<String>,
	program_argument: Option<String>,
	trusted_cpi_type: bool,
}

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

fn receiver_ident(expr: &Expr<'_>) -> Option<String> {
	match &expr.kind {
		ExprKind::Field(_, ident) => Some(ident.name.as_str().to_string()),
		ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
			path.segments
				.last()
				.map(|s| s.ident.name.as_str().to_string())
		}
		ExprKind::MethodCall(_, receiver, ..) => receiver_ident(receiver),
		ExprKind::DropTemps(inner) | ExprKind::AddrOf(_, _, inner) => receiver_ident(inner),
		_ => None,
	}
}

fn program_argument(method: &str, args: &[Expr<'_>]) -> Option<String> {
	let index = match method {
		"invoke_with_program" => 0,
		"invoke_signed_with_program" => 1,
		_ => return None,
	};

	args.get(index).and_then(receiver_ident)
}

fn collect_calls<'tcx>(cx: &LateContext<'tcx>, body: &'tcx rustc_hir::Body<'tcx>) -> Vec<CallInfo> {
	let mut calls = Vec::new();
	visit_expr(cx, body.value, &mut calls);
	calls
}

fn visit_expr<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>, calls: &mut Vec<CallInfo>) {
	match &expr.kind {
		ExprKind::MethodCall(seg, receiver, args, _) => {
			visit_expr(cx, receiver, calls);
			for arg in *args {
				visit_expr(cx, arg, calls);
			}
			let method = seg.ident.name.as_str();
			calls.push(CallInfo {
				span: expr.span,
				method: method.to_string(),
				receiver: receiver_ident(receiver),
				program_argument: program_argument(method, args),
				trusted_cpi_type: is_trusted_pina_cpi_type(cx, receiver),
			});
		}
		ExprKind::Call(callee, args) => {
			visit_expr(cx, callee, calls);
			for arg in *args {
				visit_expr(cx, arg, calls);
			}
		}
		ExprKind::Block(block, _) => {
			for stmt in block.stmts {
				match &stmt.kind {
					rustc_hir::StmtKind::Let(local) => {
						if let Some(init) = local.init {
							visit_expr(cx, init, calls);
						}
					}
					rustc_hir::StmtKind::Expr(e) | rustc_hir::StmtKind::Semi(e) => {
						visit_expr(cx, e, calls);
					}
					_ => {}
				}
			}
			if let Some(e) = block.expr {
				visit_expr(cx, e, calls);
			}
		}
		ExprKind::Match(scrutinee, arms, _) => {
			visit_expr(cx, scrutinee, calls);
			for arm in *arms {
				visit_expr(cx, arm.body, calls);
			}
		}
		ExprKind::If(cond, then, else_opt) => {
			visit_expr(cx, cond, calls);
			visit_expr(cx, then, calls);
			if let Some(el) = else_opt {
				visit_expr(cx, el, calls);
			}
		}
		ExprKind::Unary(_, e)
		| ExprKind::Cast(e, _)
		| ExprKind::DropTemps(e)
		| ExprKind::AddrOf(_, _, e)
		| ExprKind::Field(e, _) => {
			visit_expr(cx, e, calls);
		}
		ExprKind::Binary(_, lhs, rhs) | ExprKind::Assign(lhs, rhs, _) => {
			visit_expr(cx, lhs, calls);
			visit_expr(cx, rhs, calls);
		}
		ExprKind::Tup(exprs) | ExprKind::Array(exprs) => {
			for e in *exprs {
				visit_expr(cx, e, calls);
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
		let calls = collect_calls(cx, body);

		for (i, info) in calls.iter().enumerate() {
			if !CPI_METHODS.contains(&info.method.as_str()) || info.trusted_cpi_type {
				continue;
			}

			let has_program_check = calls[..i].iter().any(|prev| {
				if !PROGRAM_CHECK_METHODS.contains(&prev.method.as_str()) {
					return false;
				}

				if let Some(program_argument) = info.program_argument.as_ref() {
					return prev.receiver.as_ref() == Some(program_argument);
				}

				prev.receiver.as_ref().is_some_and(|receiver| {
					receiver.contains("program")
						|| receiver.contains("system")
						|| receiver.contains("token")
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
