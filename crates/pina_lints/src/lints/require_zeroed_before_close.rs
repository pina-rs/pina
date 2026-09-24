extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashMap;
use std::collections::HashSet;

use rustc_ast::LitKind;
use rustc_hir::BinOpKind;
use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::HirId;
use rustc_hir::MatchSource;
use rustc_hir::Mutability;
use rustc_hir::UnOp;
use rustc_hir::attrs::lang_items::LangItem;
use rustc_hir::intravisit::FnKind;
use rustc_hir::intravisit::Visitor;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_span::Span;
use rustc_span::Symbol;

use crate::diagnostics;
use crate::shared;

crate::declare_late_lint! {
	/// ### What it does
	///
	/// Warns when an instruction path closes an account with `close()` or
	/// `close_with_recipient()` without first zeroing that same account's data.
	///
	/// ### Why is this bad?
	///
	/// Closing an account without clearing its bytes first can leave stale data
	/// readable during the same transaction window.
	///
	/// ### Example
	///
	/// ```ignore
	/// // Preferred: zero the data and close in one operation.
	/// state.close_account_zeroed(&ID, recipient)?;
	///
	/// // When the close must stay separate, clear the whole buffer first.
	/// state.try_borrow_mut()?.fill(0);
	/// state.close_with_recipient(&ID, recipient)?;
	/// ```
	///
	/// The zeroing proof is a `core` slice `fill(0)` over the entire buffer
	/// returned by `AccountView::try_borrow_mut()?` — chained directly, through
	/// `[..]`, or through a `let` binding of that buffer — on the same account,
	/// on every path to the close, with no later mutable borrow of that account
	/// or use of that buffer before the close. "Same account" is decided by
	/// resolving both receivers to a local binding plus a field path. A `let`
	/// alias is followed only when its initializer is a plain place (`x`,
	/// `&mut x`, `&mut *x`, `*x`, `x.field`); a binding initialized any other
	/// way is its own account. Receivers reached through indexing, method or
	/// function calls, and bindings that may change after their `let`
	/// (assigned, mutably borrowed as a slot, or captured by a closure) have no
	/// identity, so their closes are always flagged.
	/// `close_account_zeroed()` and the `CloseAccountZeroed` builder zero before
	/// closing, so they are never flagged.
	pub REQUIRE_ZEROED_BEFORE_CLOSE,
	Deny,
	"account close should be preceded by zeroing the same account's data"
}

const TARGET_METHODS: &[&str] = &["close_with_recipient", "close"];
const TARGET_NEEDLES: &[&str] = &["process", "process_instruction", "instruction"];

impl<'tcx> LateLintPass<'tcx> for RequireZeroedBeforeClose {
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

		let mut collector = Collector::new(cx);
		collector.visit_body(body);
		let analysis = collector.finish();

		for close in &analysis.closes {
			if analysis.is_zeroed_before(close) {
				continue;
			}

			diagnostics::emit(cx, REQUIRE_ZEROED_BEFORE_CLOSE, |diag| {
				diag.span(close.span);
				diag.primary_message(
					"account close should be preceded by zeroing the same account's data",
				);
				diag.help(
					"close with `account.close_account_zeroed(&ID, recipient)?` (or the \
					 `CloseAccountZeroed` builder), which zeroes the data and closes in one step",
				);
				diag.help(
					"to keep a separate close, first clear the whole data buffer of the same \
					 account with `account.try_borrow_mut()?.fill(0);`",
				);
				diag.help(shared::CONTROL_FLOW_LIMITATION_HELP);
			});
		}
	}
}

/// An account identity: a root local binding plus the fields read from it.
#[derive(Debug, PartialEq, Eq)]
struct AccountPlace {
	root: HirId,
	fields: Vec<Symbol>,
}

/// A whole-buffer `fill(0)` of an account's data.
struct ZeroFill<'tcx> {
	order: usize,
	/// The account expression whose `try_borrow_mut()` produced the buffer.
	account: &'tcx Expr<'tcx>,
	/// The `let` binding holding the buffer, when the fill went through one.
	buffer: Option<HirId>,
	branches: Vec<HirId>,
}

/// A `close()` or `close_with_recipient()` call.
struct Close<'tcx> {
	order: usize,
	span: Span,
	receiver: &'tcx Expr<'tcx>,
	branches: Vec<HirId>,
}

/// A mutable borrow of an account's data through `AccountView::try_borrow_mut`.
struct Borrow<'tcx> {
	order: usize,
	account: &'tcx Expr<'tcx>,
}

/// A use of a buffer binding, which may write to the zeroed bytes.
struct BufferUse {
	order: usize,
	buffer: HirId,
	/// The path expression, to recognize it as the argument of `drop`.
	path: HirId,
	/// Whether the use hands the buffer to `core::mem::drop`, which ends
	/// the borrow without writing.
	is_drop: bool,
}

/// Facts about one function body, resolved once the whole body is visited.
struct Analysis<'tcx> {
	/// `let` bindings whose initializer is a plain place expression.
	aliases: HashMap<HirId, &'tcx Expr<'tcx>>,
	/// Bindings that may hold a different value after their `let`.
	unstable: HashSet<HirId>,
	zero_fills: Vec<ZeroFill<'tcx>>,
	closes: Vec<Close<'tcx>>,
	borrows: Vec<Borrow<'tcx>>,
	buffer_uses: Vec<BufferUse>,
}

impl<'tcx> Analysis<'tcx> {
	/// Whether an earlier zero fill proves `close`'s account zeroed.
	///
	/// The fill must name the same account, run on every path that reaches
	/// the close (no branch the close is not also inside), and stay the last
	/// write: a later mutable borrow of the account or use of the fill's
	/// buffer binding (other than `drop`) before the close voids it.
	fn is_zeroed_before(&self, close: &Close<'tcx>) -> bool {
		let Some(closed) = self.place_of(close.receiver) else {
			return false;
		};
		let is_between =
			|order: usize, fill: &ZeroFill<'tcx>| fill.order < order && order < close.order;

		self.zero_fills.iter().any(|fill| {
			fill.order < close.order
				&& close.branches.starts_with(&fill.branches)
				&& fill
					.buffer
					.is_none_or(|buffer| !self.unstable.contains(&buffer))
				&& self.place_of(fill.account).as_ref() == Some(&closed)
				&& !self.borrows.iter().any(|borrow| {
					is_between(borrow.order, fill)
						&& self.place_of(borrow.account).as_ref() == Some(&closed)
				}) && !self.buffer_uses.iter().any(|buffer_use| {
				is_between(buffer_use.order, fill)
					&& Some(buffer_use.buffer) == fill.buffer
					&& !buffer_use.is_drop
			})
		})
	}

	/// Resolve an account expression to its identity.
	///
	/// Derefs and borrows name the same account; field reads extend the path.
	/// Anything else — indexing, method or function calls, non-local paths —
	/// has no identity.
	fn place_of(&self, expr: &Expr<'_>) -> Option<AccountPlace> {
		self.place_of_inner(expr, &mut HashSet::new())
	}

	fn place_of_inner(
		&self,
		expr: &Expr<'_>,
		visited: &mut HashSet<HirId>,
	) -> Option<AccountPlace> {
		match expr.kind {
			ExprKind::Unary(UnOp::Deref, inner) | ExprKind::AddrOf(_, _, inner) => {
				self.place_of_inner(inner, visited)
			}
			ExprKind::Field(base, field) => {
				let mut place = self.place_of_inner(base, visited)?;
				place.fields.push(field.name);
				Some(place)
			}
			_ => {
				let binding = local_path_binding(expr)?;
				if self.unstable.contains(&binding) || !visited.insert(binding) {
					return None;
				}

				match self.aliases.get(&binding) {
					Some(initializer) => self.place_of_inner(initializer, visited),
					None => {
						Some(AccountPlace {
							root: binding,
							fields: Vec::new(),
						})
					}
				}
			}
		}
	}
}

/// Walks one function body in evaluation order, recording events after
/// their operands so `order` follows execution.
struct Collector<'cx, 'tcx> {
	cx: &'cx LateContext<'tcx>,
	order: usize,
	/// The conditional branches enclosing the expression being visited.
	branches: Vec<HirId>,
	/// Buffer bindings, mapped to the account expression they borrow.
	buffers: HashMap<HirId, &'tcx Expr<'tcx>>,
	/// `let r = &mut x;` initializers, which alias `x` rather than escape it.
	alias_borrows: HashSet<HirId>,
	/// `(r, x)` pairs for `let r = &mut x;`: `x` is exposed through `r`.
	exposures: Vec<(HirId, HirId)>,
	/// Path expressions used only to read a field or call a method.
	projection_uses: HashSet<HirId>,
	/// Every local path expression, as `(binding, path expression)`.
	local_uses: Vec<(HirId, HirId)>,
	/// Bindings written by an assignment, before alias expansion.
	assigned: Vec<HirId>,
	analysis: Analysis<'tcx>,
}

impl<'cx, 'tcx> Collector<'cx, 'tcx> {
	fn new(cx: &'cx LateContext<'tcx>) -> Self {
		Self {
			cx,
			order: 0,
			branches: Vec::new(),
			buffers: HashMap::new(),
			alias_borrows: HashSet::new(),
			exposures: Vec::new(),
			projection_uses: HashSet::new(),
			local_uses: Vec::new(),
			assigned: Vec::new(),
			analysis: Analysis {
				aliases: HashMap::new(),
				unstable: HashSet::new(),
				zero_fills: Vec::new(),
				closes: Vec::new(),
				borrows: Vec::new(),
				buffer_uses: Vec::new(),
			},
		}
	}

	/// Resolve the facts that depend on the whole body.
	fn finish(mut self) -> Analysis<'tcx> {
		// A binding exposed through `let r = &mut x;` may be rewritten
		// through `r` unless `r` is only used to read fields or call methods.
		for (alias, exposed) in &self.exposures {
			let escapes = self
				.local_uses
				.iter()
				.any(|(binding, path)| binding == alias && !self.projection_uses.contains(path));
			if escapes {
				self.analysis.unstable.insert(*exposed);
			}
		}

		// An assignment through an alias (`*r = ..`) rewrites what the alias
		// points at, so every binding on its alias chain becomes unstable.
		for binding in std::mem::take(&mut self.assigned) {
			let mut current = Some(binding);
			let mut visited = HashSet::new();
			while let Some(binding) = current
				&& visited.insert(binding)
			{
				self.analysis.unstable.insert(binding);
				current = self
					.analysis
					.aliases
					.get(&binding)
					.and_then(|initializer| place_root_binding(initializer));
			}
		}

		self.analysis
	}

	fn next_order(&mut self) -> usize {
		self.order += 1;
		self.order
	}

	fn visit_branch(&mut self, expr: &'tcx Expr<'tcx>) {
		self.branches.push(expr.hir_id);
		self.visit_expr(expr);
		self.branches.pop();
	}

	/// Classify `expr` after its operands have been visited.
	fn record(&mut self, expr: &'tcx Expr<'tcx>) {
		match expr.kind {
			ExprKind::Assign(target, ..) | ExprKind::AssignOp(_, target, _) => {
				if let Some(binding) = assignment_base_binding(target) {
					self.assigned.push(binding);
				}
			}
			ExprKind::AddrOf(_, Mutability::Mut, inner)
				if !self.alias_borrows.contains(&expr.hir_id) =>
			{
				if let Some(binding) = slot_binding(inner) {
					self.analysis.unstable.insert(binding);
				}
			}
			ExprKind::Closure(closure) => {
				if let Some(upvars) = self.cx.tcx.upvars_mentioned(closure.def_id) {
					self.analysis.unstable.extend(upvars.keys().copied());
				}
			}
			ExprKind::Field(base, _) => self.note_projection(base),
			ExprKind::Call(callee, [argument]) if is_drop(self.cx, callee) => {
				for buffer_use in &mut self.analysis.buffer_uses {
					if buffer_use.path == argument.hir_id {
						buffer_use.is_drop = true;
					}
				}
			}
			ExprKind::MethodCall(segment, receiver, arguments, _) => {
				self.note_projection(receiver);
				self.record_method_call(expr, segment.ident.name, receiver, arguments);
			}
			_ => {
				if let Some(binding) = local_path_binding(expr) {
					self.local_uses.push((binding, expr.hir_id));
					if self.buffers.contains_key(&binding) {
						let order = self.next_order();
						self.analysis.buffer_uses.push(BufferUse {
							order,
							buffer: binding,
							path: expr.hir_id,
							is_drop: false,
						});
					}
				}
			}
		}
	}

	fn record_method_call(
		&mut self,
		expr: &'tcx Expr<'tcx>,
		method: Symbol,
		receiver: &'tcx Expr<'tcx>,
		arguments: &'tcx [Expr<'tcx>],
	) {
		if let Some(account) = account_borrow(self.cx, expr) {
			let order = self.next_order();
			self.analysis.borrows.push(Borrow { order, account });
			return;
		}

		if TARGET_METHODS.contains(&method.as_str()) {
			let order = self.next_order();
			self.analysis.closes.push(Close {
				order,
				span: expr.span,
				receiver,
				branches: self.branches.clone(),
			});
			return;
		}

		let [value] = arguments else {
			return;
		};
		if method.as_str() != "fill" || !is_literal_zero(value) || !is_slice_fill(self.cx, expr) {
			return;
		}

		let buffer = whole_buffer(self.cx, receiver);
		let zeroed = match local_path_binding(buffer) {
			Some(binding) => {
				self.buffers
					.get(&binding)
					.map(|account| (*account, Some(binding)))
			}
			None => borrowed_account(self.cx, buffer).map(|account| (account, None)),
		};
		if let Some((account, buffer)) = zeroed {
			let order = self.next_order();
			self.analysis.zero_fills.push(ZeroFill {
				order,
				account,
				buffer,
				branches: self.branches.clone(),
			});
		}
	}

	/// Mark a path used as a method receiver or field base as a projection.
	fn note_projection(&mut self, expr: &Expr<'_>) {
		let mut expr = expr;
		while let ExprKind::Unary(UnOp::Deref, inner) = expr.kind {
			expr = inner;
		}
		if local_path_binding(expr).is_some() {
			self.projection_uses.insert(expr.hir_id);
		}
	}
}

impl<'tcx> Visitor<'tcx> for Collector<'_, 'tcx> {
	fn visit_local(&mut self, local: &'tcx rustc_hir::LetStmt<'tcx>) {
		if let rustc_hir::PatKind::Binding(_, binding, _, None) = local.pat.kind
			&& let Some(initializer) = local.init
		{
			if is_plain_place(initializer) {
				self.analysis.aliases.insert(binding, initializer);
				if let ExprKind::AddrOf(_, Mutability::Mut, inner) = initializer.kind
					&& let Some(exposed) = slot_binding(inner)
				{
					self.alias_borrows.insert(initializer.hir_id);
					self.exposures.push((binding, exposed));
				}
			} else if let Some(account) = borrowed_account(self.cx, initializer) {
				self.buffers.insert(binding, account);
			}
		}
		rustc_hir::intravisit::walk_local(self, local);
	}

	fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
		match expr.kind {
			ExprKind::If(condition, then, otherwise) => {
				self.visit_expr(condition);
				self.visit_branch(then);
				if let Some(otherwise) = otherwise {
					self.visit_branch(otherwise);
				}
			}
			ExprKind::Match(scrutinee, arms, source)
				if !matches!(source, MatchSource::TryDesugar(_)) =>
			{
				self.visit_expr(scrutinee);
				for arm in arms {
					self.branches.push(arm.hir_id);
					self.visit_arm(arm);
					self.branches.pop();
				}
			}
			ExprKind::Binary(operator, left, right)
				if matches!(operator.node, BinOpKind::And | BinOpKind::Or) =>
			{
				self.visit_expr(left);
				self.visit_branch(right);
			}
			_ => rustc_hir::intravisit::walk_expr(self, expr),
		}
		self.record(expr);
	}
}

/// Whether `expr` is a place expression: a local, a field of one, or a
/// borrow or dereference of one. Only these initializers make a `let` an
/// alias; a call, method call, or index produces a value of its own.
fn is_plain_place(expr: &Expr<'_>) -> bool {
	match expr.kind {
		ExprKind::Unary(UnOp::Deref, inner)
		| ExprKind::AddrOf(_, _, inner)
		| ExprKind::Field(inner, _) => is_plain_place(inner),
		_ => local_path_binding(expr).is_some(),
	}
}

/// The local binding at the root of a place expression.
fn place_root_binding(expr: &Expr<'_>) -> Option<HirId> {
	match expr.kind {
		ExprKind::Unary(UnOp::Deref, inner)
		| ExprKind::AddrOf(_, _, inner)
		| ExprKind::Field(inner, _) => place_root_binding(inner),
		_ => local_path_binding(expr),
	}
}

/// The local binding an assignment target writes into or through.
fn assignment_base_binding(expr: &Expr<'_>) -> Option<HirId> {
	match expr.kind {
		ExprKind::Unary(UnOp::Deref, inner)
		| ExprKind::Field(inner, _)
		| ExprKind::Index(inner, ..) => assignment_base_binding(inner),
		_ => local_path_binding(expr),
	}
}

/// The local binding whose own storage `&mut <expr>` borrows.
///
/// `&mut x` and `&mut x.field` lend the binding's slot, so the borrower can
/// replace what it holds. `&mut *x` lends the value `x` points at instead,
/// which leaves `x` itself unchanged.
fn slot_binding(expr: &Expr<'_>) -> Option<HirId> {
	match expr.kind {
		ExprKind::Field(inner, _) | ExprKind::Index(inner, ..) => slot_binding(inner),
		_ => local_path_binding(expr),
	}
}

fn local_path_binding(expr: &Expr<'_>) -> Option<HirId> {
	let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = expr.kind else {
		return None;
	};
	let rustc_hir::def::Res::Local(binding) = path.res else {
		return None;
	};

	Some(binding)
}

/// The account expression of a `try_borrow_mut()` call resolved to
/// `solana_account_view`'s `AccountView`, which Pina and Pinocchio re-export.
fn account_borrow<'tcx>(
	cx: &LateContext<'tcx>,
	expr: &'tcx Expr<'tcx>,
) -> Option<&'tcx Expr<'tcx>> {
	let ExprKind::MethodCall(segment, account, [], _) = expr.kind else {
		return None;
	};
	if segment.ident.name.as_str() != "try_borrow_mut" {
		return None;
	}

	let definition = cx.typeck_results().type_dependent_def_id(expr.hir_id)?;
	(cx.tcx.crate_name(definition.krate).as_str() == "solana_account_view").then_some(account)
}

/// The account expression of `account.try_borrow_mut()?`.
fn borrowed_account<'tcx>(
	cx: &LateContext<'tcx>,
	expr: &'tcx Expr<'tcx>,
) -> Option<&'tcx Expr<'tcx>> {
	let ExprKind::Match(scrutinee, _, MatchSource::TryDesugar(_)) = expr.kind else {
		return None;
	};

	account_borrow(cx, shared::try_branch_argument(cx, scrutinee)?)
}

/// Strip derefs and full-range indexing (`buffer[..]`), which address the
/// whole buffer. Any other index narrows it and is kept.
fn whole_buffer<'tcx>(cx: &LateContext<'tcx>, mut expr: &'tcx Expr<'tcx>) -> &'tcx Expr<'tcx> {
	loop {
		match expr.kind {
			ExprKind::Unary(UnOp::Deref, inner) => expr = inner,
			ExprKind::Index(inner, index, _) if is_range_full(cx, index) => expr = inner,
			_ => return expr,
		}
	}
}

fn is_range_full(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
	cx.typeck_results()
		.expr_ty(expr)
		.ty_adt_def()
		.is_some_and(|definition| cx.tcx.is_lang_item(definition.did(), LangItem::RangeFull))
}

/// Whether `expr` resolves to `core`'s slice `fill`.
fn is_slice_fill(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
	cx.typeck_results()
		.type_dependent_def_id(expr.hir_id)
		.is_some_and(|definition| {
			cx.tcx.crate_name(definition.krate).as_str() == "core"
				&& cx.tcx.item_name(definition).as_str() == "fill"
		})
}

fn is_literal_zero(expr: &Expr<'_>) -> bool {
	let ExprKind::Lit(literal) = expr.kind else {
		return false;
	};

	matches!(literal.node, LitKind::Int(value, _) if value.get() == 0)
}

/// Whether `callee` is `core::mem::drop`.
fn is_drop(cx: &LateContext<'_>, callee: &Expr<'_>) -> bool {
	let ExprKind::Path(ref path) = callee.kind else {
		return false;
	};
	let rustc_hir::def::Res::Def(_, definition) = cx.qpath_res(path, callee.hir_id) else {
		return false;
	};

	cx.tcx.def_path_str(definition) == "std::mem::drop"
		|| cx.tcx.def_path_str(definition) == "core::mem::drop"
}
