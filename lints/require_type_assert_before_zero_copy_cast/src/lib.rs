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
	/// Warns when raw bytemuck casts are used on account data without a type assertion.
	///
	/// ### Why is this bad?
	///
	/// Raw zero-copy casts can reinterpret spoofed or incorrectly sized account bytes as trusted state.
	///
	/// ### Example
	///
	/// ```ignore
	/// // See lints/readme.md for the preferred pattern.
	/// ```
	pub REQUIRE_TYPE_ASSERT_BEFORE_ZERO_COPY_CAST,
	Deny,
	"raw zero-copy casts should be preceded by `assert_type::<T>()` or a safe Pina account conversion"
}

const TARGET_METHODS: &[&str] = &[
	"try_from_bytes",
	"try_from_bytes_mut",
	"cast_ref",
	"cast_mut",
];
const TARGET_NEEDLES: &[&str] = &["process", "process_instruction", "instruction", "account"];
const TARGET_PATHS: &[&str] = &[
	"bytemuck::try_from_bytes",
	"bytemuck::try_from_bytes_mut",
	"bytemuck::cast_ref",
	"bytemuck::cast_mut",
];
const REQUIRED_METHODS: &[&str] = &["assert_type", "as_account", "as_account_mut"];
const BORROW_METHODS: &[&str] = &["try_borrow", "try_borrow_mut"];

fn prior_borrow_receiver(calls: &[shared::CallInfo], index: usize) -> Option<String> {
	calls[..index]
		.iter()
		.rev()
		.find(|call| BORROW_METHODS.contains(&call.method.as_str()))
		.and_then(|call| call.receiver.clone())
}

impl<'tcx> LateLintPass<'tcx> for RequireTypeAssertBeforeZeroCopyCast {
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
			if !TARGET_METHODS.contains(&call.method.as_str())
				&& !call
					.path
					.as_deref()
					.is_some_and(|path| TARGET_PATHS.iter().any(|needle| path.contains(needle)))
			{
				continue;
			}

			let guard_receiver =
				prior_borrow_receiver(&facts.calls, index).or_else(|| call.receiver.clone());
			let has_guard = guard_receiver.as_ref().is_some_and(|receiver| {
				shared::has_prior_method_with_receiver_match(
					&facts.calls,
					index,
					REQUIRED_METHODS,
					&Some(receiver.clone()),
				)
			});

			if !has_guard {
				cx.lint(REQUIRE_TYPE_ASSERT_BEFORE_ZERO_COPY_CAST, |diag| {
					diag.span(call.span);
					diag.primary_message(
						"raw zero-copy casts should be preceded by `assert_type::<T>()` or a safe \
						 Pina account conversion",
					);
					diag.help(
						"prefer `assert_type::<T>()` or `as_account::<T>()` over raw \
						 `bytemuck::try_from_bytes` casts",
					);
					diag.help(shared::CONTROL_FLOW_LIMITATION_HELP);
				});
			}
		}
	}
}
