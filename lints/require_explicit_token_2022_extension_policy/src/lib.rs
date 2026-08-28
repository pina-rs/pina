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
	/// Requires instruction paths that load a Token-2022-capable mint to state
	/// which extensions they accept.
	///
	/// ### Why is this bad?
	///
	/// Token-2022 extensions can change transfer and authority semantics. Reading
	/// only the legacy base fields silently treats those semantics as irrelevant.
	pub REQUIRE_EXPLICIT_TOKEN_2022_EXTENSION_POLICY,
	Deny,
	"Token-2022-capable mint loads require an explicit extension allow-list"
}

const TARGET_METHODS: &[&str] = &[
	"as_token_mint_for_program",
	"as_token_2022_mint",
	"as_token_2022_mint_checked",
];
const POLICY_METHODS: &[&str] = &["assert_extensions_allowed", "assert_no_extensions"];

fn policy_matches(load: &shared::CallInfo, policy: &shared::CallInfo) -> bool {
	if !POLICY_METHODS.contains(&policy.method.as_str()) {
		return false;
	}

	if let Some(binding) = load.result_binding.as_deref()
		&& (policy.receiver.as_deref() == Some(binding)
			|| policy.result_binding.as_deref() == Some(binding))
	{
		return true;
	}

	policy
		.receiver_span
		.is_some_and(|receiver| receiver.contains(load.span))
}

impl<'tcx> LateLintPass<'tcx> for RequireExplicitToken2022ExtensionPolicy {
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
		for (index, call) in facts.calls.iter().enumerate() {
			if !TARGET_METHODS.contains(&call.method.as_str()) {
				continue;
			}
			let next_load = facts.calls[index + 1..]
				.iter()
				.position(|next| TARGET_METHODS.contains(&next.method.as_str()))
				.map_or(facts.calls.len(), |offset| index + 1 + offset);
			let has_policy = facts.calls[index + 1..next_load]
				.iter()
				.any(|next| policy_matches(call, next));
			if has_policy {
				continue;
			}

			cx.lint(REQUIRE_EXPLICIT_TOKEN_2022_EXTENSION_POLICY, |diag| {
				diag.span(call.span);
				diag.primary_message(
					"Token-2022-capable mint loaded without an explicit extension policy",
				);
				diag.help(
					"bind the mint view and call `assert_no_extensions()` or \
					 `assert_extensions_allowed(&[...])` before using its base fields",
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
