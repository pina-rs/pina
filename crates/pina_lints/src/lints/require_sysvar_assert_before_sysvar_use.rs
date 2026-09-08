extern crate rustc_hir;
extern crate rustc_span;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::def::Res;
use rustc_hir::intravisit::FnKind;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;

use crate::shared;

crate::declare_late_lint! {
	/// ### What it does
	///
	/// Warns when raw sysvar-like accounts are read without asserting their
	/// sysvar identity first.
	///
	/// ### Why is this bad?
	///
	/// Spoofed sysvar accounts can distort rent, clock, and instruction-data
	/// logic. Pinocchio's checked `from_account_view()` and `try_from()` sysvar
	/// loaders perform the identity check while they parse the account, so they
	/// do not require a separate assertion. Known unchecked typed constructors
	/// are rejected where they are called. Deliberate raw parsing needs a local,
	/// reviewed lint allowance after `assert_sysvar()`.
	///
	/// ### Example
	///
	/// ```ignore
	/// // See lints/readme.md for the preferred pattern.
	/// ```
	pub REQUIRE_SYSVAR_ASSERT_BEFORE_SYSVAR_USE,
	Deny,
	"raw sysvar access should be preceded by `assert_sysvar()` on the same account"
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
const TRUSTED_SYSVAR_TYPES: &[&str] = &[
	"pinocchio::sysvars::clock::Clock",
	"pinocchio::sysvars::instructions::Instructions",
	"pinocchio::sysvars::rent::Rent",
	"pinocchio::sysvars::slot_hashes::SlotHashes",
];

fn terminal_identifier(value: &str) -> &str {
	value.rsplit(['.', ':']).next().unwrap_or(value)
}

fn sysvar_identifier(value: &str) -> &str {
	let terminal = terminal_identifier(value);
	terminal.strip_suffix("_account").unwrap_or(terminal)
}

fn normalized_tokens(value: &str) -> Vec<String> {
	value
		.split(|c: char| !c.is_ascii_alphanumeric())
		.filter(|token| !token.is_empty())
		.map(|token| token.to_ascii_lowercase())
		.collect()
}

fn matches_sysvar_id(receiver: &str, asserted_id: &str) -> bool {
	let expected_tokens = normalized_tokens(sysvar_identifier(receiver));
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
	let terminal = sysvar_identifier(name).to_ascii_lowercase();
	KNOWN_SYSVAR_NAMES.contains(&terminal.as_str())
		|| terminal.ends_with("_sysvar")
		|| terminal.ends_with("_instructions")
}

fn trusted_definition(definition: &shared::TypeDefinition) -> bool {
	definition.crate_name == "pinocchio" && TRUSTED_SYSVAR_TYPES.contains(&definition.path.as_str())
}

fn has_trusted_sysvar_receiver(definitions: &[shared::TypeDefinition]) -> bool {
	let Some(outer) = definitions.first() else {
		return false;
	};
	if trusted_definition(outer) {
		return true;
	}

	// Checked loaders for borrowed sysvars return an account-borrow wrapper
	// whose `Deref` target is the trusted Pinocchio type.
	outer.crate_name == "solana_account_view"
		&& matches!(
			terminal_identifier(&outer.path),
			"Ref" | "RefMut" | "MappedRef" | "MappedRefMut"
		) && definitions[1..].iter().any(trusted_definition)
}

fn is_unchecked_typed_constructor(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
	let ExprKind::Call(callee, _) = &expression.kind else {
		return false;
	};
	let ExprKind::Path(path) = &callee.kind else {
		return false;
	};
	let Res::Def(_, definition) = cx.qpath_res(path, callee.hir_id) else {
		return false;
	};
	if cx.tcx.crate_name(definition.krate).as_str() != "pinocchio"
		|| !matches!(
			cx.tcx.item_name(definition).as_str(),
			"from_bytes" | "from_bytes_unchecked"
		) {
		return false;
	}

	let path = cx.tcx.def_path_str(definition);
	TRUSTED_SYSVAR_TYPES
		.iter()
		.any(|trusted| path.starts_with(trusted))
}

fn emit_unchecked_typed_constructor(cx: &LateContext<'_>, span: rustc_span::Span) {
	cx.lint(REQUIRE_SYSVAR_ASSERT_BEFORE_SYSVAR_USE, |diag| {
		diag.span(span);
		diag.primary_message(
			"unchecked typed sysvar constructor requires a validated source account",
		);
		diag.help(
			"prefer the sysvar type's checked `from_account_view()` or `try_from()` loader; after \
			 reviewing deliberate raw parsing, use a narrow lint allowance at this constructor",
		);
	});
}

fn emit_unchecked_sysvar(cx: &LateContext<'_>, span: rustc_span::Span) {
	cx.lint(REQUIRE_SYSVAR_ASSERT_BEFORE_SYSVAR_USE, |diag| {
		diag.span(span);
		diag.primary_message(
			"raw sysvar access should be preceded by `assert_sysvar()` on the same account",
		);
		diag.help(
			"use the sysvar type's checked `from_account_view()` or `try_from()` loader, or call \
			 `sysvar_account.assert_sysvar(&sysvar::ID)?` before borrowing raw data",
		);
	});
}

impl<'tcx> LateLintPass<'tcx> for RequireSysvarAssertBeforeSysvarUse {
	fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
		if is_unchecked_typed_constructor(cx, expression) {
			emit_unchecked_typed_constructor(cx, expression.span);
		}
	}

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
		if shared::should_skip_def_path(&def_path) {
			return;
		}

		let facts = shared::collect_function_facts(cx, body);
		if !shared::def_path_matches(&def_path, TARGET_NEEDLES) {
			return;
		}

		for (index, call) in facts.calls.iter().enumerate() {
			if call.method == "assert_sysvar" {
				continue;
			}
			if has_trusted_sysvar_receiver(&call.receiver_type_definitions) {
				continue;
			}

			if matches!(call.def_crate.as_deref(), Some("alloc" | "core" | "std")) {
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
				emit_unchecked_sysvar(cx, call.span);
			}
		}
	}
}
