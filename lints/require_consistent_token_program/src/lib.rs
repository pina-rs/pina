#![feature(rustc_private)]

#[path = "../../shared.rs"]
mod shared;

extern crate rustc_hir;
extern crate rustc_span;

use rustc_hir::intravisit::FnKind;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;

dylint_linting::declare_late_lint! {
	/// ### What it does
	///
	/// Requires token parsing, ATA derivation, and dynamic token CPI calls in one
	/// instruction path to use one token-program identity.
	///
	/// ### Why is this bad?
	///
	/// Mixing program identities can validate an account under one token program
	/// and invoke another, invalidating ownership and address assumptions.
	pub REQUIRE_CONSISTENT_TOKEN_PROGRAM,
	Deny,
	"token operations in one instruction must share one program identity"
}

fn program_argument(call: &shared::CallInfo) -> Option<&str> {
	let index = match call.method.as_str() {
		"as_token_mint_for_program" | "as_token_account_for_program" => 0,
		"as_associated_token_account"
		| "as_associated_token_account_checked"
		| "assert_associated_token_address" => 2,
		"invoke_with_program" => 0,
		"invoke_signed_with_program" => 1,
		_ => return None,
	};

	call.args.get(index).and_then(Option::as_deref)
}

impl<'tcx> LateLintPass<'tcx> for RequireConsistentTokenProgram {
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
		if shared::should_skip_def_path(&def_path)
			|| !shared::def_path_matches(&def_path, &["process", "instruction"])
		{
			return;
		}

		let facts = shared::collect_function_facts(body);
		let mut expected = None;
		for call in &facts.calls {
			let Some(argument) = program_argument(call) else {
				continue;
			};
			let identity = argument;
			let Some(first) = expected else {
				expected = Some(identity);
				continue;
			};

			if identity == first {
				continue;
			}

			cx.lint(REQUIRE_CONSISTENT_TOKEN_PROGRAM, |diag| {
				diag.span(call.span);
				diag.primary_message(format!(
					"token operation uses `{identity}` after the instruction established `{first}`"
				));
				diag.help(
					"validate the token-program account once, copy its address, and pass that \
					 same value to token parsing, ATA checks, and CPI",
				);
			});
		}
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn ui() {
		dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
	}
}
