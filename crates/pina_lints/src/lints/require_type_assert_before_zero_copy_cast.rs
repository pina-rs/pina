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
	/// Warns when raw bytemuck casts are used in account-processing code.
	///
	/// ### Why is this bad?
	///
	/// Raw zero-copy casts can reinterpret spoofed or incorrectly sized account bytes as trusted state.
	/// A preceding `assert_type::<T>()` is only a moment-in-time validation and does not bind the
	/// later cast to the validated borrow.
	///
	/// ### Example
	///
	/// ```ignore
	/// // See lints/readme.md for the preferred pattern.
	/// ```
	pub REQUIRE_TYPE_ASSERT_BEFORE_ZERO_COPY_CAST,
	Deny,
	"raw zero-copy account casts should use a guard-backed Pina account conversion"
}

const TARGET_METHODS: &[&str] = &[
	"try_from_bytes",
	"try_from_bytes_mut",
	"cast_ref",
	"cast_mut",
];
fn is_instruction_handler(cx: &LateContext<'_>, def_id: rustc_hir::def_id::LocalDefId) -> bool {
	let def_id = def_id.to_def_id();
	if !matches!(
		cx.tcx.def_kind(def_id),
		rustc_hir::def::DefKind::Fn | rustc_hir::def::DefKind::AssocFn
	) {
		return false;
	}

	if cx.tcx.item_name(def_id).as_str() == "process_instruction" {
		return true;
	}

	let Some((trait_item, trait_id)) = cx.tcx.trait_item_of(def_id).and_then(|trait_item| {
		cx.tcx
			.trait_of_assoc(trait_item)
			.map(|trait_id| (trait_item, trait_id))
	}) else {
		return false;
	};

	cx.tcx.crate_name(trait_id.krate).as_str() == "pina"
		&& cx.tcx.item_name(trait_id).as_str() == "ProcessAccountInfos"
		&& cx.tcx.item_name(trait_item).as_str() == "process"
}

fn is_bytemuck_cast(call: &shared::CallInfo) -> bool {
	let Some(def_path) = call.def_path.as_deref() else {
		return false;
	};
	let method = def_path.rsplit("::").next().unwrap_or(def_path);

	call.def_crate.as_deref() == Some("bytemuck") && TARGET_METHODS.contains(&method)
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
		if shared::should_skip_def_path(&def_path) || !is_instruction_handler(cx, def_id) {
			return;
		}

		let facts = shared::collect_function_facts(cx, body);
		for call in &facts.calls {
			if !is_bytemuck_cast(call) {
				continue;
			}

			cx.lint(REQUIRE_TYPE_ASSERT_BEFORE_ZERO_COPY_CAST, |diag| {
				diag.span(call.span);
				diag.primary_message(
					"raw zero-copy account casts bypass guard-backed account validation",
				);
				diag.help(
					"replace the cast with `as_account::<T>()`, `as_account_mut::<T>()`, or a \
					 generated `load_pda*` method",
				);
				diag.help(
					"`assert_type::<T>()` is validation-only and does not make a later raw cast \
					 safe",
				);
			});
		}
	}
}
