extern crate rustc_hir;
extern crate rustc_span;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::MatchSource;
use rustc_hir::intravisit::FnKind;
use rustc_hir::intravisit::Visitor;
use rustc_hir::intravisit::walk_expr;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;

use crate::shared;

crate::declare_late_lint! {
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
	/// 	MyInstruction::Initialize => InitializeAccounts::try_from((program_id, accounts))?.process(data),
	/// 	MyInstruction::Update => UpdateAccounts::try_from((program_id, accounts))?.process(data),
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

struct DispatchVisitor<'cx, 'tcx> {
	cx: &'cx LateContext<'tcx>,
	found: bool,
}

impl DispatchVisitor<'_, '_> {
	fn is_instruction_enum(&self, scrutinee: &Expr<'_>) -> bool {
		let scrutinee_type = self.cx.typeck_results().expr_ty(scrutinee).peel_refs();
		let Some(definition) = scrutinee_type.ty_adt_def() else {
			return false;
		};

		definition.is_enum()
			&& self
				.cx
				.tcx
				.item_name(definition.did())
				.as_str()
				.to_ascii_lowercase()
				.contains("instruction")
	}
}

impl<'tcx> Visitor<'tcx> for DispatchVisitor<'_, 'tcx> {
	fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
		if self.found {
			return;
		}
		if let ExprKind::Match(scrutinee, _, MatchSource::Normal) = &expr.kind
			&& self.is_instruction_enum(scrutinee)
		{
			self.found = true;
			return;
		}

		walk_expr(self, expr);
	}
}

impl<'tcx> LateLintPass<'tcx> for RequireCanonicalInstructionDispatchForIdl {
	fn check_fn(
		&mut self,
		cx: &LateContext<'tcx>,
		_: FnKind<'tcx>,
		_: &'tcx rustc_hir::FnDecl<'tcx>,
		body: &'tcx rustc_hir::Body<'tcx>,
		span: rustc_span::Span,
		def_id: rustc_hir::def_id::LocalDefId,
	) {
		let def_path = cx.tcx.def_path_str(def_id.to_def_id());
		if shared::should_skip_def_path(&def_path)
			|| !shared::def_path_matches(&def_path, &["process_instruction", "entrypoint"])
		{
			return;
		}

		let mut visitor = DispatchVisitor { cx, found: false };
		visitor.visit_expr(body.value);
		if !visitor.found {
			cx.lint(REQUIRE_CANONICAL_INSTRUCTION_DISPATCH_FOR_IDL, |diag| {
				diag.span(span);
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
