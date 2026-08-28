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
		let has_named_seed_constant = facts.paths.iter().any(|path| {
			path.rsplit("::")
				.next()
				.is_some_and(|name| name == "SEED" || name.ends_with("_SEED"))
		});
		let uses_generated_seed_builder = facts.paths.iter().any(|path| path == "seeds")
			|| facts.calls.iter().any(|call| {
				call.receiver.is_none()
					&& matches!(
						call.method.as_str(),
						"assert_seeds" | "assert_canonical_bump" | "assert_seeds_with_bump"
					)
			});
		let seed_assertion = facts.calls.iter().find(|call| {
			call.method == "assert_seeds"
				|| call.method == "assert_canonical_bump"
				|| call.method == "assert_seeds_with_bump"
		});
		if !facts.has_byte_string
			&& !has_named_seed_constant
			&& !uses_generated_seed_builder
			&& let Some(seed_assertion) = seed_assertion
		{
			cx.lint(
				REQUIRE_EXPLICIT_DISCRIMINATORS_AND_SEED_NAMESPACES,
				|diag| {
					diag.span(seed_assertion.span);
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

#[cfg(test)]
mod tests {
	#[test]
	fn ui() {
		dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
	}
}
