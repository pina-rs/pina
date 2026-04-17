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
	/// Warns when seed-heavy example code does not make its byte-string namespaces and discriminator markers obvious.
	///
	/// ### Why is this bad?
	///
	/// Explicit discriminator and seed naming patterns make Pina examples easier to audit and easier for the IDL
	/// extractor to understand.
	///
	/// ### Example
	///
	/// ```ignore
	/// const CONFIG_SEED: &[u8] = b"config";
	/// ```
	pub REQUIRE_EXPLICIT_DISCRIMINATORS_AND_SEED_NAMESPACES,
	Warn,
	"seed-based example code should use explicit byte-string namespaces and visible discriminator markers"
}

impl Default for RequireExplicitDiscriminatorsAndSeedNamespaces {
	fn default() -> Self {
		Self
	}
}

impl<'tcx> LateLintPass<'tcx> for RequireExplicitDiscriminatorsAndSeedNamespaces {
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
			|| !shared::def_path_matches(
				&def_path,
				&["process_instruction", "process", "instruction", "account"],
			) {
			return;
		}

		let facts = shared::collect_function_facts(body);
		if !facts.has_byte_string
			&& facts.calls.iter().any(|call| {
				call.method == "assert_seeds"
					|| call.method == "assert_canonical_bump"
					|| call.method == "assert_seeds_with_bump"
			}) {
			cx.lint(
				REQUIRE_EXPLICIT_DISCRIMINATORS_AND_SEED_NAMESPACES,
				|diag| {
					diag.primary_message(
						"seed-based example code should use explicit byte-string namespaces and \
						 visible discriminator markers",
					);
					diag.help(
						"use byte-string seed prefixes such as `b\"config\"` and keep \
						 `#[discriminator]` / `#[instruction(...)]` annotations explicit",
					);
				},
			);
		}
	}
}
