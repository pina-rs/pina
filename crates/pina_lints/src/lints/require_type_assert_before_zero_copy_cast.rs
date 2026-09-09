extern crate rustc_hir;
extern crate rustc_span;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::Node;
use rustc_hir::def::DefKind;
use rustc_hir::def::Res;
use rustc_hir::intravisit::FnKind;
use rustc_hir::intravisit::Visitor;
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

/// Bytemuck entry points that can reinterpret bytes as typed values.
///
/// This includes the `checked` module: checked bit patterns still do not bind
/// the borrow to Pina's owner, exact-size, discriminator, and schema checks.
const TARGET_METHODS: &[&str] = &[
	"cast",
	"try_cast",
	"from_bytes",
	"from_bytes_mut",
	"try_from_bytes",
	"try_from_bytes_mut",
	"pod_read_unaligned",
	"try_pod_read_unaligned",
	"pod_align_to",
	"pod_align_to_mut",
	"cast_ref",
	"cast_mut",
	"try_cast_ref",
	"try_cast_mut",
	"cast_slice",
	"cast_slice_mut",
	"try_cast_slice",
	"try_cast_slice_mut",
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

fn is_bytemuck_cast(cx: &LateContext<'_>, definition: rustc_hir::def_id::DefId) -> bool {
	let def_path = cx.tcx.def_path_str(definition);
	let method = def_path.rsplit("::").next().unwrap_or(&def_path);

	cx.tcx.crate_name(definition.krate).as_str() == "bytemuck" && TARGET_METHODS.contains(&method)
}

fn path_definition(
	cx: &LateContext<'_>,
	expression: &Expr<'_>,
) -> Option<rustc_hir::def_id::DefId> {
	let ExprKind::Path(path) = &expression.kind else {
		return None;
	};
	let Res::Def(DefKind::Fn | DefKind::AssocFn, definition) =
		cx.qpath_res(path, expression.hir_id)
	else {
		return None;
	};

	Some(definition)
}

struct Analyzer<'cx, 'tcx> {
	cx: &'cx LateContext<'tcx>,
}

impl Analyzer<'_, '_> {
	fn emit(&self, span: rustc_span::Span) {
		self.cx
			.lint(REQUIRE_TYPE_ASSERT_BEFORE_ZERO_COPY_CAST, |diag| {
				diag.span(span);
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

	fn is_direct_call_callee(&self, expression: &Expr<'_>) -> bool {
		matches!(
			self.cx.tcx.parent_hir_node(expression.hir_id),
			Node::Expr(Expr {
				kind: ExprKind::Call(callee, _),
				..
			}) if callee.hir_id == expression.hir_id
		)
	}
}

impl<'tcx> Visitor<'tcx> for Analyzer<'_, 'tcx> {
	fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
		match &expression.kind {
			ExprKind::Call(callee, _) => {
				if path_definition(self.cx, callee)
					.is_some_and(|definition| is_bytemuck_cast(self.cx, definition))
				{
					self.emit(expression.span);
				}
			}
			ExprKind::Path(_) => {
				if !self.is_direct_call_callee(expression)
					&& path_definition(self.cx, expression)
						.is_some_and(|definition| is_bytemuck_cast(self.cx, definition))
				{
					self.emit(expression.span);
				}
			}
			ExprKind::Closure(closure) => {
				// Nested bodies are not visited by default. A closure can otherwise
				// hide a raw conversion and invoke it after the outer scan finishes.
				self.visit_body(self.cx.tcx.hir_body(closure.body));
				return;
			}
			_ => {}
		}

		rustc_hir::intravisit::walk_expr(self, expression);
	}
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

		Analyzer { cx }.visit_body(body);
	}
}
