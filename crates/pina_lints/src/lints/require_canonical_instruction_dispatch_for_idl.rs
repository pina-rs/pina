extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashSet;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::MatchSource;
use rustc_hir::def::DefKind;
use rustc_hir::def::Res;
use rustc_hir::intravisit::FnKind;
use rustc_hir::intravisit::Visitor;
use rustc_hir::intravisit::walk_expr;
use rustc_hir::intravisit::walk_stmt;
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
	parsed_instruction_bindings: &'cx HashSet<rustc_hir::HirId>,
	found: bool,
}

impl<'tcx> DispatchVisitor<'_, 'tcx> {
	fn is_parsed_instruction_enum(&self, scrutinee: &'tcx Expr<'tcx>) -> bool {
		let scrutinee_type = self.cx.typeck_results().expr_ty(scrutinee).peel_refs();
		let Some(definition) = scrutinee_type.ty_adt_def() else {
			return false;
		};

		definition.is_enum()
			&& expression_is_parsed_instruction(
				self.cx,
				scrutinee,
				self.parsed_instruction_bindings,
			)
	}
}

impl<'tcx> Visitor<'tcx> for DispatchVisitor<'_, 'tcx> {
	fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
		if self.found {
			return;
		}
		if let ExprKind::Match(scrutinee, _, MatchSource::Normal) = &expr.kind
			&& self.is_parsed_instruction_enum(scrutinee)
		{
			self.found = true;
			return;
		}

		walk_expr(self, expr);
	}
}

fn is_parse_instruction_call(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
	let ExprKind::Call(callee, _) = &expr.kind else {
		return false;
	};
	let ExprKind::Path(path) = &callee.kind else {
		return false;
	};
	let Res::Def(DefKind::Fn, def_id) = cx.qpath_res(path, callee.hir_id) else {
		return false;
	};

	cx.tcx.crate_name(def_id.krate).as_str() == "pina"
		&& cx.tcx.item_name(def_id).as_str() == "parse_instruction"
}

fn expression_is_parsed_instruction<'tcx>(
	cx: &LateContext<'tcx>,
	expr: &'tcx Expr<'tcx>,
	parsed_instruction_bindings: &HashSet<rustc_hir::HirId>,
) -> bool {
	if is_parse_instruction_call(cx, expr) {
		return true;
	}

	match &expr.kind {
		ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
			matches!(path.res, Res::Local(binding) if parsed_instruction_bindings.contains(&binding))
		}
		ExprKind::Call(callee, [argument]) => {
			let ExprKind::Path(path) = &callee.kind else {
				return false;
			};
			let Res::Def(DefKind::AssocFn, def_id) = cx.qpath_res(path, callee.hir_id) else {
				return false;
			};

			cx.tcx.crate_name(def_id.krate).as_str() == "core"
				&& cx.tcx.item_name(def_id).as_str() == "branch"
				&& expression_is_parsed_instruction(cx, argument, parsed_instruction_bindings)
		}
		ExprKind::Match(scrutinee, _, MatchSource::TryDesugar(_)) => {
			expression_is_parsed_instruction(cx, scrutinee, parsed_instruction_bindings)
		}
		ExprKind::Block(block, _) => {
			block.expr.is_some_and(|tail| {
				expression_is_parsed_instruction(cx, tail, parsed_instruction_bindings)
			})
		}
		_ => false,
	}
}

struct ParsedInstructionBindingCollector<'cx, 'tcx> {
	cx: &'cx LateContext<'tcx>,
	bindings: HashSet<rustc_hir::HirId>,
}

impl<'tcx> Visitor<'tcx> for ParsedInstructionBindingCollector<'_, 'tcx> {
	fn visit_stmt(&mut self, statement: &'tcx rustc_hir::Stmt<'tcx>) {
		if let rustc_hir::StmtKind::Let(local) = &statement.kind
			&& let Some(initializer) = local.init
			&& let rustc_hir::PatKind::Binding(_, binding, _, None) = local.pat.kind
			&& expression_is_parsed_instruction(self.cx, initializer, &self.bindings)
		{
			self.bindings.insert(binding);
		}

		walk_stmt(self, statement);
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
		let function_name = def_path.rsplit("::").next().unwrap_or_default();
		let is_entrypoint = matches!(function_name, "process_instruction" | "entrypoint")
			|| function_name.starts_with("entrypoint_");
		if shared::should_skip_def_path(&def_path) || !is_entrypoint {
			return;
		}

		let mut collector = ParsedInstructionBindingCollector {
			cx,
			bindings: HashSet::new(),
		};
		collector.visit_expr(body.value);

		let mut visitor = DispatchVisitor {
			cx,
			parsed_instruction_bindings: &collector.bindings,
			found: false,
		};
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
