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
	/// Warns when sysvar-like accounts are used without asserting their sysvar identity first.
	///
	/// ### Why is this bad?
	///
	/// Spoofed sysvar accounts can distort rent, clock, and instruction-data logic.
	///
	/// ### Example
	///
	/// ```ignore
	/// // See lints/readme.md for the preferred pattern.
	/// ```
	pub REQUIRE_SYSVAR_ASSERT_BEFORE_SYSVAR_USE,
	Deny,
	"sysvar access should be preceded by `assert_sysvar()` on the same account"
}

const TARGET_NEEDLES: &[&str] = &["process", "process_instruction", "instruction", "sysvar"];
const KNOWN_SYSVAR_NAMES: &[&str] = &[
	"clock",
	"epoch_rewards",
	"epoch_schedule",
	"fees",
	"instructions",
	"last_restart_slot",
	"recent_blockhashes",
	"rent",
	"rewards",
	"slot_hashes",
	"slot_history",
	"stake_history",
];

fn terminal_identifier(value: &str) -> &str {
	value.rsplit(['.', ':']).next().unwrap_or(value)
}

fn normalized_tokens(value: &str) -> Vec<String> {
	value
		.split(|c: char| !c.is_ascii_alphanumeric())
		.filter(|token| !token.is_empty())
		.map(|token| token.to_ascii_lowercase())
		.collect()
}

fn matches_sysvar_id(receiver: &str, asserted_id: &str) -> bool {
	let expected_tokens = normalized_tokens(terminal_identifier(receiver));
	let asserted_tokens = normalized_tokens(asserted_id);
	if expected_tokens.is_empty() || asserted_tokens.is_empty() {
		return false;
	}

	let mut asserted_iter = asserted_tokens.iter();
	expected_tokens
		.iter()
		.all(|token| asserted_iter.by_ref().any(|candidate| candidate == token))
}

fn is_sysvar_receiver(name: &str) -> bool {
	let terminal = terminal_identifier(name).to_ascii_lowercase();
	KNOWN_SYSVAR_NAMES.contains(&terminal.as_str())
		|| terminal.ends_with("_sysvar")
		|| terminal.ends_with("_instructions")
}

impl<'tcx> LateLintPass<'tcx> for RequireSysvarAssertBeforeSysvarUse {
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
			|| !shared::def_path_matches(&def_path, TARGET_NEEDLES)
		{
			return;
		}

		let facts = shared::collect_function_facts(body);
		for (index, call) in facts.calls.iter().enumerate() {
			if call.method == "assert_sysvar" {
				continue;
			}

			let looks_like_sysvar_use = call.receiver.as_deref().is_some_and(is_sysvar_receiver)
				|| call.path.as_deref().is_some_and(|path| {
					let terminal = terminal_identifier(path).to_ascii_lowercase();
					matches!(
						terminal.as_str(),
						"load_current_index" | "load_instruction_at"
					) || KNOWN_SYSVAR_NAMES.contains(&terminal.as_str())
						|| terminal.ends_with("_sysvar")
						|| terminal.ends_with("_instructions")
				});

			if !looks_like_sysvar_use {
				continue;
			}

			let has_guard = call.receiver.as_deref().is_some_and(|receiver| {
				facts.calls[..index].iter().any(|prior| {
					prior.method == "assert_sysvar"
						&& prior.receiver.as_deref() == Some(receiver)
						&& prior
							.args
							.first()
							.and_then(Option::as_deref)
							.is_some_and(|arg| matches_sysvar_id(receiver, arg))
				})
			});
			if !has_guard {
				cx.lint(REQUIRE_SYSVAR_ASSERT_BEFORE_SYSVAR_USE, |diag| {
					diag.span(call.span);
					diag.primary_message(
						"sysvar access should be preceded by `assert_sysvar()` on the same account",
					);
					diag.help(
						"call `sysvar_account.assert_sysvar(&sysvar::ID)?` before reading time or \
						 rent information",
					);
				});
			}
		}
	}
}
