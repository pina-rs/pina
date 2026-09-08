extern crate rustc_hir;
extern crate rustc_span;

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
	/// do not require a separate assertion.
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

fn trusted_sysvar_type(
	definition_crate: Option<&str>,
	definition_path: Option<&str>,
) -> Option<&'static str> {
	if definition_crate != Some("pinocchio") {
		return None;
	}

	definition_path.and_then(|path| {
		TRUSTED_SYSVAR_TYPES.iter().find_map(|trusted| {
			(path == *trusted
				|| path
					.strip_prefix(trusted)
					.is_some_and(|suffix| suffix.starts_with("::")))
			.then_some(*trusted)
		})
	})
}

fn is_checked_loader_for(call: &shared::CallInfo, trusted_type: &str) -> bool {
	if trusted_type.ends_with("::Instructions") {
		return call.method == "try_from"
			&& call.is_type_relative
			&& call
				.def_path
				.as_deref()
				.is_some_and(|path| path.ends_with("TryFrom::try_from"));
	}

	call.method == "from_account_view"
		&& call.def_crate.as_deref() == Some("pinocchio")
		&& call
			.def_path
			.as_deref()
			.is_some_and(|path| path.starts_with(trusted_type))
}

fn is_transparent_result_adapter(call: &shared::CallInfo) -> bool {
	matches!(call.def_crate.as_deref(), Some("core" | "std"))
		&& matches!(
			call.method.as_str(),
			"branch" | "from_output" | "inspect" | "inspect_err" | "map_err"
		)
}

fn has_checked_typed_loader_at(
	facts: &shared::FunctionFacts,
	trusted_type: &str,
	receiver_binding: Option<rustc_hir::HirId>,
	receiver_span: rustc_span::Span,
	use_span: rustc_span::Span,
) -> bool {
	let named_loader = receiver_binding.is_some_and(|receiver_binding| {
		facts.calls.iter().rev().any(|prior| {
			prior.span.lo() < use_span.lo()
				&& prior.result_binding_id == Some(receiver_binding)
				&& is_checked_loader_for(prior, trusted_type)
				&& !facts.assignments.iter().any(|assignment| {
					assignment.binding == Some(receiver_binding)
						&& assignment.span.lo() > prior.span.lo()
						&& assignment.span.lo() < use_span.lo()
				})
		})
	});
	if named_loader {
		return true;
	}

	facts.calls.iter().rev().any(|prior| {
		prior.span.lo() >= receiver_span.lo()
			&& prior.span.hi() <= receiver_span.hi()
			&& is_checked_loader_for(prior, trusted_type)
			&& !facts.calls.iter().any(|later| {
				later.span.lo() > prior.span.lo()
					&& later.span.hi() <= receiver_span.hi()
					&& !is_transparent_result_adapter(later)
			})
	})
}

fn has_checked_typed_loader(facts: &shared::FunctionFacts, call: &shared::CallInfo) -> bool {
	let Some(trusted_type) =
		trusted_sysvar_type(call.def_crate.as_deref(), call.def_path.as_deref())
	else {
		return false;
	};
	let Some(receiver_span) = call.receiver_span else {
		return false;
	};

	has_checked_typed_loader_at(
		facts,
		trusted_type,
		call.receiver_binding,
		receiver_span,
		call.span,
	)
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

		let facts = shared::collect_function_facts(cx, body);
		for (index, call) in facts.calls.iter().enumerate() {
			if call.method == "assert_sysvar" || has_checked_typed_loader(&facts, call) {
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

		for field in &facts.field_accesses {
			let Some(trusted_type) = trusted_sysvar_type(
				field.receiver_type_crate.as_deref(),
				field.receiver_type_path.as_deref(),
			) else {
				continue;
			};
			if !has_checked_typed_loader_at(
				&facts,
				trusted_type,
				field.receiver_binding,
				field.receiver_span,
				field.span,
			) {
				emit_unchecked_sysvar(cx, field.span);
			}
		}
	}
}
