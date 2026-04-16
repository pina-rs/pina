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
	/// Warns when heap-heavy allocation patterns appear in on-chain instruction handlers.
	///
	/// ### Why is this bad?
	///
	/// On-chain code should prefer borrowed slices, stack-backed buffers, and fixed-size POD types to avoid unnecessary CU and allocation overhead.
	///
	/// ### Example
	///
	/// ```ignore
	/// // See lints/readme.md for the preferred pattern.
	/// ```
	pub DENY_HEAP_ALLOCATIONS_IN_ONCHAIN_INSTRUCTION_HANDLERS,
	Warn,
	"heap allocation patterns should be avoided in on-chain instruction handlers"
}

const TARGET_METHODS: &[&str] = &["collect", "to_vec", "to_string", "clone"];
const TARGET_NEEDLES: &[&str] = &[
	"process",
	"process_instruction",
	"try_from_account_infos",
	"instruction",
];
const TARGET_PATHS: &[&str] = &[
	"format",
	"Vec::new",
	"Vec::with_capacity",
	"String::new",
	"String::from",
];

impl<'tcx> LateLintPass<'tcx> for DenyHeapAllocationsInOnchainInstructionHandlers {
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
		for call in &facts.calls {
			let matches_heap_pattern = TARGET_METHODS.contains(&call.method.as_str())
				|| call
					.path
					.as_deref()
					.is_some_and(|path| TARGET_PATHS.iter().any(|needle| path.contains(needle)));

			if !matches_heap_pattern {
				continue;
			}

			cx.lint(
				DENY_HEAP_ALLOCATIONS_IN_ONCHAIN_INSTRUCTION_HANDLERS,
				|diag| {
					diag.span(call.span);
					diag.primary_message(
						"heap allocation patterns should be avoided in on-chain instruction \
						 handlers",
					);
					diag.help(
						"prefer borrowed slices, stack-backed buffers, and fixed-size POD types \
						 in instruction paths",
					);
				},
			);
		}
	}
}
