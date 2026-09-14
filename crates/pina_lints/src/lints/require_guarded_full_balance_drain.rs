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
	/// Warns when an instruction handler can sweep an account's entire
	/// balance to a recipient in a single call without a visible pause,
	/// circuit-breaker, or withdrawal-cap guard.
	///
	/// ### Why is this bad?
	///
	/// Ungated full-balance drains are the shape real key-compromise exploits
	/// use: once the sweep authority leaks, nothing on-chain slows the drain.
	/// A pause switch plus a per-window withdrawal cap bounds the blast
	/// radius, and a separate guardian key can halt the program without being
	/// able to move funds.
	pub REQUIRE_GUARDED_FULL_BALANCE_DRAIN,
	Warn,
	"full-balance drains should be gated by a pause or circuit-breaker guard"
}

const DRAIN_METHODS: &[&str] = &["send", "send_owned"];
const BALANCE_METHODS: &[&str] = &["lamports"];
const CLOSE_METHODS: &[&str] = &[
	"zeroed",
	"close",
	"close_with_recipient",
	"close_account_zeroed",
];
const GUARD_TERMS: &[&str] = &[
	"pause", "cap", "circuit", "halt", "guard", "limit", "throttle",
];
const TARGET_NEEDLES: &[&str] = &["process", "process_instruction", "instruction"];

/// Whether the drain call's lamport amount is the drained account's own full
/// balance, passed inline or through one `let` binding.
fn drains_full_balance(facts: &shared::FunctionFacts, drain: &shared::CallInfo) -> bool {
	let Some(amount) = drain.args.get(1).and_then(Clone::clone) else {
		return false;
	};

	facts.calls.iter().any(|balance_call| {
		BALANCE_METHODS.contains(&balance_call.method.as_str())
			&& balance_call.receiver == drain.receiver
			&& (balance_call.result_binding.as_deref() == Some(amount.as_str())
				|| (balance_call.result_binding.is_none()
					&& balance_call.receiver.as_deref() == Some(amount.as_str())))
	})
}

fn has_close_intent(facts: &shared::FunctionFacts, drain: &shared::CallInfo) -> bool {
	facts.calls.iter().any(|call| {
		CLOSE_METHODS.contains(&call.method.as_str()) && call.receiver == drain.receiver
	})
}

fn has_guard(facts: &shared::FunctionFacts) -> bool {
	facts.calls.iter().any(|call| {
		let method = call.method.to_ascii_lowercase();

		GUARD_TERMS.iter().any(|term| method.contains(term))
	})
}

impl<'tcx> LateLintPass<'tcx> for RequireGuardedFullBalanceDrain {
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

		if has_guard(&facts) {
			return;
		}

		for drain in &facts.calls {
			if !DRAIN_METHODS.contains(&drain.method.as_str()) || drain.receiver.is_none() {
				continue;
			}

			if !drains_full_balance(&facts, drain) || has_close_intent(&facts, drain) {
				continue;
			}

			cx.lint(REQUIRE_GUARDED_FULL_BALANCE_DRAIN, |diag| {
				diag.span(drain.span);
				diag.primary_message(
					"an instruction path can sweep an account's entire balance in one call",
				);
				diag.help(
					"gate full-balance sweeps behind a pause or circuit-breaker check (a pause \
					 flag plus a per-window withdrawal cap bounds a compromised key's blast \
					 radius)",
				);
				diag.help(
					"if this drain is an account-close path, use `close_account_zeroed` so \
					 `require_zeroed_before_close` covers the stale-data risk too",
				);
				diag.help(shared::CONTROL_FLOW_LIMITATION_HELP);
			});
		}
	}
}
