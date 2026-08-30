extern crate rustc_hir;
extern crate rustc_span;

use std::cell::Cell;
use std::cell::RefCell;

use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::Level;
use rustc_lint::LintContext;
use rustc_span::hygiene::ExpnKind;
use rustc_span::hygiene::MacroKind;

thread_local! {
	static DECLARE_ID_COUNT: Cell<usize> = const { Cell::new(0) };
	/// Definition paths, spans, and defining nodes of the found program
	/// ids, so warnings and suppression resolve against the declaration
	/// that introduced them. Entries whose definition path carries no `::`
	/// separator live at the crate root.
	static DECLARE_ID_DECLARATIONS: RefCell<Vec<(String, rustc_span::Span, rustc_hir::hir_id::HirId)>> = const { RefCell::new(Vec::new()) };
	static HAS_MATCHED_ITEMS: Cell<bool> = const { Cell::new(false) };
}

crate::declare_late_lint! {
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
	if !is_program_id_path(def_path) || !item.span.from_expansion() {
		return false;
	}

	matches!(
		item.span.ctxt().outer_expn_data().kind,
		ExpnKind::Macro(MacroKind::Bang, macro_name) if macro_name.as_str() == "declare_id"
	)
}

/// Return whether the definition path's final segment names a program ID.
///
/// Local `def_path_str` output is relative to the crate root, so root and
/// module level declarations alike end with an `ID` segment.
fn is_program_id_path(def_path: &str) -> bool {
	def_path
		.rsplit(':')
		.next()
		.is_some_and(|segment| segment == "ID")
}

/// Return whether the item's source file lives under an `examples` or
/// `security` directory of this repository.
///
/// Definition paths do not carry directory information (the crate name is
/// omitted from local `def_path_str` output), so repository scoping has to
/// look at the source file itself.
fn repository_scoped(cx: &LateContext<'_>, span: rustc_span::Span) -> bool {
	let rustc_span::FileName::Real(real) = cx.sess().source_map().span_to_filename(span) else {
		return false;
	};
	let Some(local) = real.local_path() else {
		return false;
	};
	let Some(shown) = local.to_str() else {
		return false;
	};
	shown.contains("examples") || shown.contains("security")
}

impl<'tcx> LateLintPass<'tcx> for RequireIdlRootToDefineOneProgramId {
	fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx rustc_hir::Item<'tcx>) {
		if !repository_scoped(cx, item.span) {
			return;
		}
		let def_path = cx.tcx.def_path_str(item.owner_id.def_id.to_def_id());

		HAS_MATCHED_ITEMS.with(|flag| flag.set(true));
		if is_declare_id_expansion(item, &def_path) {
			DECLARE_ID_COUNT.with(|count| count.set(count.get() + 1));
			DECLARE_ID_DECLARATIONS.with(|declarations| {
				declarations.borrow_mut().push((
					def_path.clone(),
					item.span.source_callsite(),
					item.hir_id(),
				));
			});
		}
	}

	fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
		let declare_id_count = DECLARE_ID_COUNT.with(Cell::get);
		let has_matched_items = HAS_MATCHED_ITEMS.with(Cell::get);
		let declarations = DECLARE_ID_DECLARATIONS
			.with(|declarations| std::mem::take(&mut *declarations.borrow_mut()));
		DECLARE_ID_COUNT.with(|count| count.set(0));
		HAS_MATCHED_ITEMS.with(|flag| flag.set(false));

		if !has_matched_items || declare_id_count == 1 {
			return;
		}

		// The check runs outside any item scope, so the lint level has to be
		// resolved explicitly against the declaration site; this keeps
		// module-scoped `#[allow(...)]` attributes working. Only non-root
		// declarations are reported: the crate-root program id is the
		// contract, and every additional declaration is the violation.
		let extra_declarations = declarations
			.iter()
			.filter(|(def_path, ..)| def_path.contains("::"))
			.filter(|(_, _, declaration_id)| {
				!matches!(
					cx.tcx
						.lint_level_at_node(
							REQUIRE_IDL_ROOT_TO_DEFINE_ONE_PROGRAM_ID,
							*declaration_id,
						)
						.level,
					Level::Allow,
				)
			})
			.collect::<Vec<_>>();

		if extra_declarations.is_empty() {
			return;
		}

		cx.lint(REQUIRE_IDL_ROOT_TO_DEFINE_ONE_PROGRAM_ID, |diag| {
			// Point at a discovered declaration so suppression and the lint
			// level resolve against the item that introduced the extra (or
			// missing) program id.
			if let Some((_, declaration_span, _)) = extra_declarations.first() {
				diag.span(*declaration_span);
			}
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
