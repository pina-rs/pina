#![feature(rustc_private)]

#[path = "../../shared.rs"]
mod shared;

extern crate rustc_hir;
extern crate rustc_span;

use std::cell::Cell;

use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;
use rustc_span::hygiene::ExpnKind;
use rustc_span::hygiene::MacroKind;

thread_local! {
	static DECLARE_ID_COUNT: Cell<usize> = const { Cell::new(0) };
	static HAS_MATCHED_ITEMS: Cell<bool> = const { Cell::new(false) };
}

dylint_linting::declare_late_lint! {
	/// ### What it does
	///
	/// Warns when IDL-oriented example crates do not appear to define exactly one program ID at the crate root.
	///
	/// ### Why is this bad?
	///
	/// Pina's IDL extractor starts from the crate root and expects a single program ID declaration so it can resolve
	/// the example program consistently.
	///
	/// ### Example
	///
	/// ```ignore
	/// declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");
	/// ```
	pub REQUIRE_IDL_ROOT_TO_DEFINE_ONE_PROGRAM_ID,
	Warn,
	"IDL-oriented example crates should define exactly one program ID at the crate root"
}

fn is_declare_id_expansion(item: &rustc_hir::Item<'_>, def_path: &str) -> bool {
	if !def_path.ends_with("::ID") || !item.span.from_expansion() {
		return false;
	}

	matches!(
		item.span.ctxt().outer_expn_data().kind,
		ExpnKind::Macro(MacroKind::Bang, macro_name) if macro_name.as_str() == "declare_id"
	)
}

impl<'tcx> LateLintPass<'tcx> for RequireIdlRootToDefineOneProgramId {
	fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx rustc_hir::Item<'tcx>) {
		let def_path = cx.tcx.def_path_str(item.owner_id.def_id.to_def_id());
		if !shared::def_path_matches(&def_path, &["examples", "security"]) {
			return;
		}

		HAS_MATCHED_ITEMS.with(|flag| flag.set(true));
		if is_declare_id_expansion(item, &def_path) {
			DECLARE_ID_COUNT.with(|count| count.set(count.get() + 1));
		}
	}

	fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
		let declare_id_count = DECLARE_ID_COUNT.with(Cell::get);
		let has_matched_items = HAS_MATCHED_ITEMS.with(Cell::get);
		DECLARE_ID_COUNT.with(|count| count.set(0));
		HAS_MATCHED_ITEMS.with(|flag| flag.set(false));

		if !has_matched_items || declare_id_count == 1 {
			return;
		}

		cx.lint(REQUIRE_IDL_ROOT_TO_DEFINE_ONE_PROGRAM_ID, |diag| {
			diag.primary_message(
				"IDL-oriented example crates should define exactly one `declare_id!` in the crate \
				 root",
			);
			diag.help(
				"keep the program id declaration in `src/lib.rs` and avoid duplicating it across \
				 modules",
			);
		});
	}
}
