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
	/// Requires custody destinations to be read both before and after a token
	/// transfer CPI.
	///
	/// ### Why is this bad?
	///
	/// Token-2022 transfer fees can make the amount received differ from the
	/// requested amount. Protocol accounting must use the observed balance delta.
	pub REQUIRE_POST_CPI_BALANCE_RELOAD,
	Deny,
	"custody deposits must account from a post-CPI token balance delta"
}

fn is_custody_account(identity: &str) -> bool {
	let name = identity.to_ascii_lowercase();
	["vault", "custody", "reserve", "pool"]
		.iter()
		.any(|part| name.contains(part))
}

fn is_transfer_constructor(cx: &LateContext<'_>, call: &shared::CallInfo) -> bool {
	// Associated constructors are lowered as type-relative HIR paths, which do
	// not retain the type name in `CallInfo`. Confirm the source-level constructor
	// before applying the custody-name heuristic below.
	if call.method != "new" || call.args.len() < 5 {
		return false;
	}

	let snippet = cx
		.sess()
		.source_map()
		.span_to_snippet(call.span)
		.unwrap_or_default();
	snippet.contains("Transfer::new(") || snippet.contains("TransferChecked::new(")
}

fn is_explicit_legacy_constructor(call: &shared::CallInfo) -> bool {
	call.def_crate.as_deref() == Some("pinocchio_token")
		&& call.def_path.as_deref().is_some_and(|path| {
			path.contains("instructions::Transfer::new")
				|| path.contains("instructions::TransferChecked::new")
		})
}

fn is_cpi_invocation(call: &shared::CallInfo) -> bool {
	matches!(
		call.method.as_str(),
		"invoke" | "invoke_signed" | "invoke_with_program" | "invoke_signed_with_program"
	)
}

fn is_dynamic_token_invocation(call: &shared::CallInfo) -> bool {
	matches!(
		call.method.as_str(),
		"invoke_with_program" | "invoke_signed_with_program"
	)
}

fn invocation_matches(constructor: &shared::CallInfo, invocation: &shared::CallInfo) -> bool {
	if !is_cpi_invocation(invocation) {
		return false;
	}

	if let Some(binding) = constructor.result_binding.as_deref()
		&& invocation.receiver.as_deref() == Some(binding)
	{
		return true;
	}

	invocation
		.receiver_span
		.is_some_and(|receiver| receiver.contains(constructor.span))
}

fn lint_balance_reload(cx: &LateContext<'_>, span: rustc_span::Span, destination: &str) {
	cx.lint(REQUIRE_POST_CPI_BALANCE_RELOAD, |diag| {
		diag.span(span);
		diag.primary_message(format!(
			"transfer into `{destination}` is not accounted from its observed balance delta"
		));
		diag.help(
			"read the destination amount before CPI, drop the borrow, invoke the transfer, reload \
			 the amount, and use `checked_sub` for the received value",
		);
	});
}

impl<'tcx> LateLintPass<'tcx> for RequirePostCpiBalanceReload {
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

		let facts = shared::collect_function_facts(cx, body);
		for (index, call) in facts.calls.iter().enumerate() {
			if !is_transfer_constructor(cx, call) {
				continue;
			}
			let Some(destination) = call.args.get(2).and_then(Option::as_deref) else {
				continue;
			};
			if !is_custody_account(destination) {
				continue;
			}

			let invocation = facts.calls[index + 1..]
				.iter()
				.position(|next| invocation_matches(call, next));
			let Some(invocation) = invocation.map(|offset| index + 1 + offset) else {
				// The transfer builder was passed through an opaque wrapper. Avoid a
				// deny-level guess when the actual invocation cannot be associated.
				continue;
			};
			if !is_dynamic_token_invocation(&facts.calls[invocation])
				&& is_explicit_legacy_constructor(call)
			{
				continue;
			}

			let reads_amount = |candidate: &shared::CallInfo| {
				candidate.method == "amount" && candidate.receiver.as_deref() == Some(destination)
			};
			let before = facts.calls[..invocation].iter().rposition(reads_amount);
			let has_before = before.is_some_and(|before| {
				!facts.calls[before + 1..invocation]
					.iter()
					.any(is_cpi_invocation)
			});
			let after = facts.calls[invocation + 1..]
				.iter()
				.position(reads_amount)
				.map(|offset| invocation + 1 + offset);
			let has_after = after.is_some_and(|after| {
				!facts.calls[invocation + 1..after]
					.iter()
					.any(is_cpi_invocation)
			});
			if has_before && has_after {
				continue;
			}

			lint_balance_reload(cx, facts.calls[invocation].span, destination);
		}
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn ui() {
		dylint_testing::ui_test(
			env!("CARGO_PKG_NAME"),
			concat!(env!("CARGO_MANIFEST_DIR"), "/ui"),
		);
	}
}
