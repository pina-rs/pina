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
	/// Warns when example-program entrypoints hide instruction dispatch instead of matching directly on the parsed
	/// instruction enum.
	///
	/// ### Why is this bad?
	///
	/// Pina's IDL extractor is easiest to reason about when the program's entrypoint uses an explicit `match` over
	/// the parsed instruction enum.
	///
	/// ### Example
	///
	/// ```ignore
	/// match ix {
	/// 	MyInstruction::Initialize => InitializeAccounts::try_from_account_infos(accounts)?.process(data),
	/// 	MyInstruction::Update => UpdateAccounts::try_from_account_infos(accounts)?.process(data),
	/// }
	/// ```
	pub REQUIRE_CANONICAL_INSTRUCTION_DISPATCH_FOR_IDL,
	Warn,
	"IDL-friendly instruction dispatch should be a direct `match` over the parsed instruction enum"
}

impl Default for RequireCanonicalInstructionDispatchForIdl {
	fn default() -> Self {
		Self
	}
}

impl<'tcx> LateLintPass<'tcx> for RequireCanonicalInstructionDispatchForIdl {
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
			|| !shared::def_path_matches(&def_path, &["process_instruction", "entrypoint"])
		{
			return;
		}

		let facts = shared::collect_function_facts(body);
		if !facts.has_match {
			cx.lint(REQUIRE_CANONICAL_INSTRUCTION_DISPATCH_FOR_IDL, |diag| {
				diag.primary_message(
					"IDL-friendly instruction dispatch should be a direct `match` over the parsed \
					 instruction enum",
				);
				diag.help(
					"keep the dispatch in the entrypoint itself so `pina idl` can follow the \
					 instruction routing",
				);
			});
		}
	}
}
