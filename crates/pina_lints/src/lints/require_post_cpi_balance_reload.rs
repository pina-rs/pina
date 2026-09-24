extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashMap;
use std::collections::HashSet;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::HirId;
use rustc_hir::MatchSource;
use rustc_hir::Node;
use rustc_hir::Pat;
use rustc_hir::PatKind;
use rustc_hir::def::DefKind;
use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::intravisit::Visitor;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;
use rustc_middle::ty::Ty;
use rustc_middle::ty::TyKind;
use rustc_span::Span;
use rustc_span::sym;

use crate::diagnostics;
use crate::shared;

crate::declare_late_lint! {
	/// ### What it does
	///
	/// Requires the destination of every value-moving token CPI (`Transfer`,
	/// `TransferChecked`, `MintTo`, and `MintToChecked` builders) to be
	/// re-read after the CPI when an integer snapshot of its balance taken
	/// before the CPI is used afterwards: the snapshot may only be combined
	/// with that reload (as in `after.checked_sub(before)`) or compared with a
	/// constant. Destinations named like protocol custody (`vault`, `custody`,
	/// `reserve`, or `pool`) must additionally be read both before and after
	/// every transfer, with no other CPI in between.
	///
	/// ### Why is this bad?
	///
	/// Token-2022 transfer fees can make the amount received differ from the
	/// requested amount, so a balance read before the CPI no longer describes
	/// the account after it. Accounting must use the balance observed after the
	/// CPI, never a snapshot taken before it.
	pub REQUIRE_POST_CPI_BALANCE_RELOAD,
	Deny,
	"token balances snapshotted before a value-moving CPI must be reloaded after it"
}

/// Crates whose `Transfer` builders move lamports, not tokens.
const SYSTEM_PROGRAM_CRATES: &[&str] = &["pinocchio_system", "solana_system_interface"];

/// Pina loaders that parse an account's data into a token view without
/// changing which account is read.
const TOKEN_VIEW_LOADERS: &[&str] = &[
	"as_account",
	"as_associated_token_account",
	"as_token_2022_account",
	"as_token_account",
	"as_token_account_for_program",
];

/// A token-program instruction that increases its destination's balance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TokenCpiKind {
	Transfer,
	TransferChecked,
	MintTo,
	MintToChecked,
}

impl TokenCpiKind {
	/// Matches on the suffix so wrappers such as `SplTransfer` and the real
	/// `transfer_checked::TransferChecked` resolve alike.
	fn from_type_name(name: &str) -> Option<Self> {
		if name.ends_with("TransferChecked") {
			Some(Self::TransferChecked)
		} else if name.ends_with("MintToChecked") {
			Some(Self::MintToChecked)
		} else if name.ends_with("Transfer") {
			Some(Self::Transfer)
		} else if name.ends_with("MintTo") {
			Some(Self::MintTo)
		} else {
			None
		}
	}

	/// Position of the credited account, given how many account parameters
	/// lead the constructor.
	///
	/// Transfers take `(from, to, authority)` or, when the mint is checked,
	/// `(from, mint, to, authority)`; mints take `(mint, account, authority)`.
	fn destination_index(self, leading_accounts: usize) -> usize {
		if self.is_transfer() && leading_accounts >= 4 {
			2
		} else {
			1
		}
	}

	/// Transfers can be charged a Token-2022 fee on arrival; mints cannot.
	fn is_transfer(self) -> bool {
		matches!(self, Self::Transfer | Self::TransferChecked)
	}

	fn verb(self) -> &'static str {
		if self.is_transfer() {
			"transfer"
		} else {
			"mint"
		}
	}
}

/// A constructor call whose resolved result type is a value-moving token
/// instruction builder.
struct TokenCpiConstructor {
	kind: TokenCpiKind,
	destination_index: usize,
	/// The credited account's key, or `None` when it cannot be named
	/// precisely (a dynamic index, an arbitrary call).
	destination: Option<String>,
	builder: DefId,
	/// The builder's program type parameter is the legacy SPL Token program.
	/// A static `invoke()` of such a builder targets `Tokenkeg...`, which has
	/// no transfer-fee extension, so the requested amount is exactly what
	/// arrives.
	legacy_program: bool,
}

/// A `.invoke*()` call and the builder type it is called on.
struct Invocation {
	hir_id: HirId,
	receiver: Option<DefId>,
}

/// Which account an expression names, and whether it is a parsed token view
/// of that account rather than the account handle itself.
struct AccountKey {
	path: String,
	view: bool,
}

/// A read of a token balance outside any closure: `.amount()` or
/// `Type::amount(account)`.
struct AmountRead {
	hir_id: HirId,
	span: Span,
	account: String,
}

/// A binding or assignment that gives integer locals a new value.
struct Definition {
	bindings: Vec<HirId>,
	/// The whole `let` statement or assignment expression.
	span: Span,
	/// The expression providing the value.
	value: Span,
	/// The local copied verbatim, for `let snapshot = before;`.
	copy_of: Option<HirId>,
}

/// A read of a local variable.
struct LocalUse {
	binding: HirId,
	hir_id: HirId,
	span: Span,
}

/// Collects the type-resolved facts the analysis needs in one pass.
struct Analyzer<'cx, 'tcx> {
	cx: &'cx LateContext<'tcx>,
	let_initializers: HashMap<HirId, &'tcx Expr<'tcx>>,
	constructors: HashMap<Span, TokenCpiConstructor>,
	invocations: HashMap<Span, Invocation>,
	amount_reads: Vec<AmountRead>,
	definitions: Vec<Definition>,
	local_uses: Vec<LocalUse>,
	closure_depth: usize,
}

fn local_path(expr: &Expr<'_>) -> Option<HirId> {
	match expr.kind {
		ExprKind::Path(rustc_hir::QPath::Resolved(None, path)) => {
			match path.res {
				Res::Local(binding) => Some(binding),
				_ => None,
			}
		}
		ExprKind::DropTemps(inner) => local_path(inner),
		_ => None,
	}
}

/// Whether `earlier` ends before `later` starts in the source the user wrote.
///
/// Macro and desugaring spans are mapped to their call site first so `?` and
/// assertion macros order like the code around them.
fn precedes(earlier: Span, later: Span) -> bool {
	earlier.source_callsite().hi() <= later.source_callsite().lo()
}

fn encloses(outer: Span, inner: Span) -> bool {
	outer.source_callsite().contains(inner.source_callsite())
}

fn node_span(node: Node<'_>) -> Option<Span> {
	match node {
		Node::Expr(expr) => Some(expr.span),
		Node::Block(block) => Some(block.span),
		_ => None,
	}
}

fn is_cpi_method(method: &str) -> bool {
	matches!(
		method,
		"invoke"
			| "invoke_signed"
			| "invoke_with_program"
			| "invoke_signed_with_program"
			| "invoke_with_unverified_program"
			| "invoke_signed_with_unverified_program"
	)
}

impl<'tcx> Analyzer<'_, 'tcx> {
	fn crate_name(&self, definition: DefId) -> String {
		self.cx.tcx.crate_name(definition.krate).as_str().to_owned()
	}

	/// Peels references, `Result`, and `Option` to the ADT underneath.
	fn core_adt(&self, mut ty: Ty<'tcx>) -> Option<DefId> {
		loop {
			ty = ty.peel_refs();
			let TyKind::Adt(definition, generics) = ty.kind() else {
				return None;
			};
			if self
				.cx
				.tcx
				.is_diagnostic_item(sym::Result, definition.did())
				|| self
					.cx
					.tcx
					.is_diagnostic_item(sym::Option, definition.did())
			{
				ty = generics.type_at(0);
				continue;
			}
			return Some(definition.did());
		}
	}

	/// Whether an argument is fixed at compile time: a literal, `()`, or a
	/// path to a constant or static, optionally borrowed or negated.
	fn is_constant(&self, expr: &Expr<'_>) -> bool {
		match &expr.kind {
			ExprKind::Lit(_) => true,
			ExprKind::Tup([]) => true,
			ExprKind::Unary(rustc_hir::UnOp::Neg, inner)
			| ExprKind::AddrOf(_, _, inner)
			| ExprKind::Cast(inner, _)
			| ExprKind::DropTemps(inner) => self.is_constant(inner),
			ExprKind::Path(path) => {
				matches!(
					self.cx.qpath_res(path, expr.hir_id),
					Res::Def(
						DefKind::Const
							| DefKind::AssocConst | DefKind::ConstParam
							| DefKind::Static { .. },
						_
					)
				)
			}
			_ => false,
		}
	}

	/// Names the account an expression refers to.
	///
	/// The key is the root binding plus the full field path, so
	/// `ctx.user_ata` and `ctx.fee_ata` never collapse onto `ctx`. Only a
	/// fixed allow-list of steps is looked through: `let` aliases, `&`, `*`,
	/// `?`, Pina's token-view loaders, the `.base` field of a loaded view, and
	/// methods whose result has the receiver's type and whose arguments are
	/// constant (assertion chains, `ok_or(())`). A method with constant
	/// arguments that changes the type (`get(2)`) becomes part of the key.
	/// Anything else is unknown, and unknown matches no account.
	fn account_key(&self, expr: &'tcx Expr<'tcx>) -> Option<AccountKey> {
		match &expr.kind {
			ExprKind::Path(rustc_hir::QPath::Resolved(None, path)) => {
				if let Res::Local(binding) = path.res
					&& let Some(initializer) = self.let_initializers.get(&binding)
				{
					return self.account_key(initializer);
				}

				Some(AccountKey {
					path: path
						.segments
						.iter()
						.map(|segment| segment.ident.name.as_str())
						.collect::<Vec<_>>()
						.join("::"),
					view: false,
				})
			}
			ExprKind::Field(base, field) => {
				let base = self.account_key(base)?;
				if base.view {
					// Token-2022's `StateWithExtensions` keeps the token account in
					// `.base`; any other field of a view is not an account.
					return (field.name.as_str() == "base").then_some(base);
				}

				Some(AccountKey {
					path: format!("{}.{field}", base.path),
					view: false,
				})
			}
			ExprKind::Index(base, index, _) => {
				let base = self.account_key(base)?;
				if base.view || !matches!(index.kind, ExprKind::Lit(_)) {
					return None;
				}
				let index = self
					.cx
					.sess()
					.source_map()
					.span_to_snippet(index.span)
					.ok()?;

				Some(AccountKey {
					path: format!("{}[{index}]", base.path),
					view: false,
				})
			}
			ExprKind::Unary(rustc_hir::UnOp::Deref, inner)
			| ExprKind::AddrOf(_, _, inner)
			| ExprKind::DropTemps(inner)
			| ExprKind::Type(inner, _)
			| ExprKind::Match(inner, _, MatchSource::TryDesugar(_)) => self.account_key(inner),
			ExprKind::Call(..) => {
				let argument = shared::try_branch_argument(self.cx, expr)?;
				self.account_key(argument)
			}
			ExprKind::MethodCall(segment, receiver, arguments, _) => {
				let receiver_key = self.account_key(receiver)?;
				let method = segment.ident.name.as_str();
				if TOKEN_VIEW_LOADERS.contains(&method) && !receiver_key.view {
					return Some(AccountKey {
						path: receiver_key.path,
						view: true,
					});
				}
				if !arguments.iter().all(|argument| self.is_constant(argument)) {
					return None;
				}

				let typeck = self.cx.typeck_results();
				let preserves_type = self.core_adt(typeck.expr_ty(receiver)).is_some()
					&& self.core_adt(typeck.expr_ty(receiver))
						== self.core_adt(typeck.expr_ty(expr));
				if preserves_type {
					return Some(receiver_key);
				}

				let source_map = self.cx.sess().source_map();
				let arguments = arguments
					.iter()
					.map(|argument| source_map.span_to_snippet(argument.span).ok())
					.collect::<Option<Vec<_>>>()?
					.join(", ");
				Some(AccountKey {
					path: format!("{}.{method}({arguments})", receiver_key.path),
					view: false,
				})
			}
			ExprKind::Block(block, _) => block.expr.and_then(|tail| self.account_key(tail)),
			_ => None,
		}
	}

	fn account_path(&self, expr: &'tcx Expr<'tcx>) -> Option<String> {
		self.account_key(expr).map(|key| key.path)
	}

	/// Classifies a call by the type it returns and by its signature.
	///
	/// The resolved builder type (after unwrapping `Result`/`Option`) must end
	/// in a token instruction name, and the constructor must have the token
	/// builder shape: at least three leading reference parameters (source or
	/// mint, destination, authority) followed by an integer amount. Types
	/// such as `AuthorityTransfer::new(config, new_authority, signer)` that
	/// move no amount therefore do not count.
	fn token_cpi_constructor(
		&self,
		expr: &'tcx Expr<'tcx>,
		callee: &'tcx Expr<'tcx>,
		args: &'tcx [Expr<'tcx>],
	) -> Option<TokenCpiConstructor> {
		let ExprKind::Path(path) = &callee.kind else {
			return None;
		};
		let Res::Def(DefKind::AssocFn, function) = self.cx.qpath_res(path, callee.hir_id) else {
			return None;
		};
		if !matches!(
			self.cx.tcx.item_name(function).as_str(),
			"new" | "with_multisig_signers"
		) {
			return None;
		}

		let mut ty = self.cx.typeck_results().expr_ty(expr);
		while let TyKind::Adt(wrapper, generics) = ty.kind()
			&& (self.cx.tcx.is_diagnostic_item(sym::Result, wrapper.did())
				|| self.cx.tcx.is_diagnostic_item(sym::Option, wrapper.did()))
		{
			ty = generics.type_at(0);
		}
		let TyKind::Adt(builder, generics) = ty.kind() else {
			return None;
		};
		if SYSTEM_PROGRAM_CRATES.contains(&self.crate_name(builder.did()).as_str()) {
			return None;
		}
		let kind = TokenCpiKind::from_type_name(self.cx.tcx.item_name(builder.did()).as_str())?;

		let signature = self
			.cx
			.tcx
			.fn_sig(function)
			.instantiate_identity()
			.skip_binder();
		let parameters = signature.inputs();
		let leading_accounts = parameters
			.iter()
			.take_while(|parameter| {
				matches!(parameter.kind(), TyKind::Ref(_, inner, _)
					if matches!(inner.kind(), TyKind::Adt(..) | TyKind::Param(_)))
			})
			.count();
		let has_amount = parameters
			.get(leading_accounts)
			.is_some_and(|amount| amount.is_integral());
		if leading_accounts < 3 || !has_amount {
			return None;
		}
		let destination_index = kind.destination_index(leading_accounts);

		// `pina::token_2022` re-exports these builders as aliases of the
		// `pinocchio_token` structs with a `Token2022Program` parameter, so the
		// crate alone does not identify the legacy program: the program type
		// parameter must be `pinocchio_token::TokenProgram` too.
		let legacy_program = self.crate_name(builder.did()) == "pinocchio_token"
			&& generics.types().last().is_none_or(|program| {
				program.ty_adt_def().is_some_and(|program| {
					self.crate_name(program.did()) == "pinocchio_token"
						&& self.cx.tcx.item_name(program.did()).as_str() == "TokenProgram"
				})
			});

		Some(TokenCpiConstructor {
			kind,
			destination_index,
			destination: args
				.get(destination_index)
				.and_then(|destination| self.account_path(destination)),
			builder: builder.did(),
			legacy_program,
		})
	}

	/// Records integer bindings introduced by `pattern = value`, pairing tuple
	/// patterns with tuple expressions element by element.
	fn record_definitions(&mut self, pattern: &Pat<'_>, value: &'tcx Expr<'tcx>, span: Span) {
		if let (PatKind::Tuple(patterns, rest), ExprKind::Tup(values)) =
			(&pattern.kind, &value.kind)
			&& rest.as_opt_usize().is_none()
			&& patterns.len() == values.len()
		{
			for (pattern, value) in patterns.iter().zip(values.iter()) {
				self.record_definitions(pattern, value, span);
			}
			return;
		}

		let typeck = self.cx.typeck_results();
		let mut bindings = Vec::new();
		pattern.each_binding(|_, binding, _, _| {
			if typeck.node_type(binding).is_integral() {
				bindings.push(binding);
			}
		});
		if bindings.is_empty() {
			return;
		}

		self.definitions.push(Definition {
			bindings,
			span,
			value: value.span,
			copy_of: local_path(value),
		});
	}

	fn reads_of<'a>(&'a self, destination: &'a str) -> impl Iterator<Item = &'a AmountRead> {
		self.amount_reads
			.iter()
			.filter(move |read| read.account == destination)
	}

	/// Definitions whose value contains a balance read of `destination`
	/// selected by `read_filter`, or that copy such a definition verbatim.
	fn balance_definitions(
		&self,
		destination: &str,
		read_filter: impl Fn(&AmountRead) -> bool,
	) -> Vec<&Definition> {
		let mut found: Vec<&Definition> = Vec::new();

		for definition in &self.definitions {
			let direct = self
				.reads_of(destination)
				.filter(|read| read_filter(read))
				.any(|read| encloses(definition.value, read.span));
			let copied = definition.copy_of.is_some_and(|source| {
				found
					.iter()
					.any(|earlier| earlier.bindings.contains(&source))
			});
			if direct || copied {
				found.push(definition);
			}
		}

		found
	}

	/// The lexically latest definition of `binding` before `point`.
	fn reaching_definition(&self, binding: HirId, point: Span) -> Option<&Definition> {
		self.definitions
			.iter()
			.filter(|definition| {
				definition.bindings.contains(&binding) && precedes(definition.span, point)
			})
			.max_by_key(|definition| definition.span.source_callsite().lo())
	}

	fn diverges(&self, expr: &Expr<'_>) -> bool {
		match expr.kind {
			ExprKind::Ret(_)
			| ExprKind::Break(..)
			| ExprKind::Continue(_)
			| ExprKind::Become(_) => true,
			ExprKind::Block(..) | ExprKind::Loop(..) => {
				self.cx.typeck_results().expr_ty(expr).is_never()
			}
			_ => false,
		}
	}

	/// Whether control can flow from the CPI at `invocation` to `target`.
	///
	/// A CPI inside a block that always diverges (`return`, `break`, ...)
	/// cannot reach code after that block, and a CPI in one `if`/`match` arm
	/// cannot reach a sibling arm.
	fn reaches(&self, invocation: HirId, target: Span) -> bool {
		let mut child = invocation;

		for (id, node) in self.cx.tcx.hir_parent_iter(invocation) {
			if node_span(node).is_some_and(|span| encloses(span, target)) {
				return match node {
					Node::Expr(Expr {
						kind: ExprKind::If(condition, ..),
						..
					}) => condition.hir_id == child,
					Node::Expr(Expr {
						kind: ExprKind::Match(_, arms, _),
						..
					}) => !arms.iter().any(|arm| arm.hir_id == child),
					_ => true,
				};
			}
			if let Node::Expr(expr) = node
				&& self.diverges(expr)
			{
				return false;
			}
			child = id;
		}

		true
	}

	/// Whether `read` runs on every path that reaches `target`: it may not sit
	/// in a conditional arm, a loop body, a closure, or the short-circuited
	/// operand of `&&`/`||` that does not also contain `target`. A reload in
	/// the same loop iteration as the use still dominates it.
	fn dominates(&self, read: HirId, target: Span) -> bool {
		let mut child = read;

		for (id, node) in self.cx.tcx.hir_parent_iter(read) {
			let conditional = match node {
				Node::Arm(_) => true,
				Node::Expr(expr) => {
					match expr.kind {
						ExprKind::If(condition, ..) => condition.hir_id != child,
						ExprKind::Loop(..) | ExprKind::Closure(..) => true,
						ExprKind::Binary(operator, _, right) => {
							matches!(
								operator.node,
								rustc_hir::BinOpKind::And | rustc_hir::BinOpKind::Or
							) && right.hir_id == child
						}
						_ => false,
					}
				}
				_ => false,
			};
			if conditional {
				return false;
			}
			if node_span(node).is_some_and(|span| encloses(span, target)) {
				return true;
			}
			child = id;
		}

		false
	}

	/// The outermost expression around `usage` within its statement, `let`,
	/// block tail, or match arm.
	fn statement_expression(&self, usage: HirId) -> Span {
		let mut span = self.cx.tcx.hir_span(usage);

		for (_, node) in self.cx.tcx.hir_parent_iter(usage) {
			let Node::Expr(expr) = node else {
				break;
			};
			span = expr.span;
		}

		span
	}

	/// Comparing a pre-CPI snapshot against a constant (`if prior == 0`,
	/// `before as u128 >= CAP`) records a fact about the account before the
	/// CPI, which stays true.
	fn is_constant_comparison(&self, usage: &LocalUse) -> bool {
		let mut operand = usage.hir_id;
		let parent = loop {
			let Node::Expr(parent) = self.cx.tcx.parent_hir_node(operand) else {
				return false;
			};
			if matches!(parent.kind, ExprKind::Cast(..) | ExprKind::DropTemps(_)) {
				operand = parent.hir_id;
				continue;
			}
			break parent;
		};
		let ExprKind::Binary(operator, left, right) = parent.kind else {
			return false;
		};
		let other = if left.hir_id == operand { right } else { left };

		operator.node.is_comparison() && self.is_constant(other)
	}

	/// Whether the statement expression holding a stale use also uses a
	/// dominating post-CPI reload of `destination`, as in
	/// `after.checked_sub(before)` or `before.checked_add(after - before)`.
	fn is_combined_with_reload(
		&self,
		destination: &str,
		invocation: Span,
		usage: &LocalUse,
	) -> bool {
		let statement = self.statement_expression(usage.hir_id);
		let dominating_reload = |read: &AmountRead| {
			precedes(invocation, read.span) && self.dominates(read.hir_id, usage.span)
		};

		let direct = self
			.reads_of(destination)
			.any(|read| encloses(statement, read.span) && dominating_reload(read));
		if direct {
			return true;
		}

		let reloads = self.balance_definitions(destination, |read| {
			dominating_reload(read) && precedes(read.span, statement)
		});
		let reload_bindings = reloads
			.iter()
			.filter(|definition| precedes(invocation, definition.span))
			.flat_map(|definition| definition.bindings.iter().copied())
			.collect::<HashSet<_>>();

		self.local_uses.iter().any(|reload_use| {
			encloses(statement, reload_use.span)
				&& reload_bindings.contains(&reload_use.binding)
				&& self
					.reaching_definition(reload_use.binding, reload_use.span)
					.is_some_and(|definition| precedes(invocation, definition.span))
		})
	}

	/// The snapshot tier, which applies to every destination: returns a use
	/// of a pre-CPI balance snapshot of `destination` that the CPI can reach
	/// and that neither compares it with a constant nor combines it with a
	/// dominating post-CPI reload.
	fn stale_snapshot_use(
		&self,
		destination: &str,
		invocation: HirId,
		invocation_span: Span,
	) -> Option<Span> {
		let snapshots = self.balance_definitions(destination, |_| true);

		self.local_uses
			.iter()
			.filter(|usage| precedes(invocation_span, usage.span))
			.filter(|usage| {
				self.reaching_definition(usage.binding, usage.span)
					.is_some_and(|definition| {
						precedes(definition.span, invocation_span)
							&& snapshots
								.iter()
								.any(|snapshot| std::ptr::eq(*snapshot, definition))
					})
			})
			.filter(|usage| self.reaches(invocation, usage.span))
			.filter(|usage| !self.is_constant_comparison(usage))
			.find(|usage| !self.is_combined_with_reload(destination, invocation_span, usage))
			.map(|usage| usage.span)
	}

	/// The custody tier: a transfer into a custody-named account must be
	/// bracketed by destination reads with no other CPI in between.
	fn custody_transfer_is_unaccounted(
		&self,
		destination: Option<&str>,
		invocation: Span,
		cpi_spans: &[Span],
	) -> bool {
		let Some(destination) = destination else {
			return true;
		};
		let cpi_between = |start: Span, end: Span| {
			cpi_spans
				.iter()
				.any(|cpi| precedes(start, *cpi) && precedes(*cpi, end))
		};
		let before = self
			.reads_of(destination)
			.filter(|read| precedes(read.span, invocation))
			.max_by_key(|read| read.span.source_callsite().lo());
		let after = self
			.reads_of(destination)
			.filter(|read| precedes(invocation, read.span))
			.min_by_key(|read| read.span.source_callsite().lo());
		let has_before = before.is_some_and(|read| !cpi_between(read.span, invocation));
		let has_after = after.is_some_and(|read| !cpi_between(invocation, read.span));

		!(has_before && has_after)
	}

	fn record_amount_read(&mut self, expr: &Expr<'_>, account: &'tcx Expr<'tcx>) {
		// A read inside a closure only happens if and when the closure runs,
		// so it cannot bracket or reload a CPI.
		if self.closure_depth > 0 {
			return;
		}
		if let Some(account) = self.account_path(account) {
			self.amount_reads.push(AmountRead {
				hir_id: expr.hir_id,
				span: expr.span,
				account,
			});
		}
	}
}

impl<'tcx> Visitor<'tcx> for Analyzer<'_, 'tcx> {
	fn visit_local(&mut self, local: &'tcx rustc_hir::LetStmt<'tcx>) {
		if let Some(initializer) = local.init {
			if let PatKind::Binding(_, binding, _, None) = local.pat.kind {
				self.let_initializers.insert(binding, initializer);
			}
			self.record_definitions(local.pat, initializer, local.span);
		}

		rustc_hir::intravisit::walk_local(self, local);
	}

	fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
		match &expr.kind {
			ExprKind::Call(callee, args) => {
				if let Some(constructor) = self.token_cpi_constructor(expr, callee, args) {
					self.constructors.insert(expr.span, constructor);
				}
				if let (ExprKind::Path(path), [account]) = (&callee.kind, args) {
					let is_amount = match self.cx.qpath_res(path, callee.hir_id) {
						Res::Def(DefKind::AssocFn | DefKind::Fn, function) => {
							self.cx.tcx.item_name(function).as_str() == "amount"
						}
						_ => false,
					};
					if is_amount {
						self.record_amount_read(expr, account);
					}
				}
			}
			ExprKind::MethodCall(segment, receiver, arguments, _) => {
				let method = segment.ident.name.as_str();
				if is_cpi_method(method) {
					let receiver_ty = self.cx.typeck_results().expr_ty(receiver).peel_refs();
					self.invocations.insert(
						expr.span,
						Invocation {
							hir_id: expr.hir_id,
							receiver: receiver_ty.ty_adt_def().map(|definition| definition.did()),
						},
					);
				}
				if method == "amount" && arguments.is_empty() {
					self.record_amount_read(expr, receiver);
				}
			}
			ExprKind::Assign(target, value, _) => {
				if let Some(binding) = local_path(target)
					&& self.cx.typeck_results().expr_ty(target).is_integral()
				{
					self.definitions.push(Definition {
						bindings: vec![binding],
						span: expr.span,
						value: value.span,
						copy_of: local_path(value),
					});
					// Writing a local is not a read of its previous value.
					self.visit_expr(value);
					return;
				}
			}
			ExprKind::Path(rustc_hir::QPath::Resolved(None, path)) => {
				if let Res::Local(binding) = path.res {
					self.local_uses.push(LocalUse {
						binding,
						hir_id: expr.hir_id,
						span: expr.span,
					});
				}
			}
			ExprKind::Closure(closure) => {
				// A snapshot captured by a closure is still used where the
				// closure runs; nested bodies are not visited by default.
				self.closure_depth += 1;
				self.visit_body(self.cx.tcx.hir_body(closure.body));
				self.closure_depth -= 1;
			}
			_ => {}
		}

		rustc_hir::intravisit::walk_expr(self, expr);
	}
}

fn invocation_matches(constructor: &shared::CallInfo, invocation: &shared::CallInfo) -> bool {
	if !is_cpi_method(&invocation.method) {
		return false;
	}

	if let Some(binding) = constructor.result_binding.as_deref()
		&& invocation.receiver.as_deref() == Some(binding)
	{
		return true;
	}

	invocation
		.receiver_span
		.is_some_and(|receiver| receiver.contains(constructor.span))
}

fn is_custody_account(identity: &str) -> bool {
	let name = identity.to_ascii_lowercase();
	["vault", "custody", "reserve", "pool"]
		.iter()
		.any(|part| name.contains(part))
}

fn lint_custody_transfer(cx: &LateContext<'_>, invocation: Span, destination: &str) {
	diagnostics::emit(cx, REQUIRE_POST_CPI_BALANCE_RELOAD, |diag| {
		diag.span(invocation);
		diag.primary_message(format!(
			"transfer into `{destination}` is not accounted from its observed balance delta"
		));
		diag.help(
			"read the destination amount before CPI, release the destination borrow (drop or \
			 scope it), invoke the transfer, reload the amount, and use `checked_sub` for the \
			 received value",
		);
	});
}

fn lint_stale_snapshot(
	cx: &LateContext<'_>,
	invocation: Span,
	stale_use: Span,
	kind: TokenCpiKind,
	destination: &str,
) {
	diagnostics::emit(cx, REQUIRE_POST_CPI_BALANCE_RELOAD, |diag| {
		diag.span(invocation);
		diag.primary_message(format!(
			"{} into `{destination}` makes an earlier read of its balance stale",
			kind.verb()
		));
		diag.span_note(stale_use, "the pre-CPI balance snapshot is used here");
		diag.help(
			"reload the destination amount after the CPI and account from \
			 `after.checked_sub(before)` instead of trusting the snapshot",
		);
	});
}

impl<'tcx> LateLintPass<'tcx> for RequirePostCpiBalanceReload {
	fn check_fn(
		&mut self,
		cx: &LateContext<'tcx>,
		_: FnKind<'tcx>,
		_: &'tcx rustc_hir::FnDecl<'tcx>,
		body: &'tcx rustc_hir::Body<'tcx>,
		_: Span,
		def_id: rustc_hir::def_id::LocalDefId,
	) {
		let def_path = cx.tcx.def_path_str(def_id.to_def_id());
		if shared::should_skip_def_path(&def_path)
			|| !shared::def_path_matches(&def_path, &["process", "instruction"])
		{
			return;
		}

		let facts = shared::collect_function_facts(cx, body);
		let mut analyzer = Analyzer {
			cx,
			let_initializers: HashMap::new(),
			constructors: HashMap::new(),
			invocations: HashMap::new(),
			amount_reads: Vec::new(),
			definitions: Vec::new(),
			local_uses: Vec::new(),
			closure_depth: 0,
		};
		analyzer.visit_body(body);
		analyzer
			.definitions
			.sort_by_key(|definition| definition.span.source_callsite().lo());

		let cpi_spans = facts
			.calls
			.iter()
			.filter(|call| is_cpi_method(&call.method))
			.map(|call| call.span)
			.collect::<Vec<_>>();

		for (index, call) in facts.calls.iter().enumerate() {
			let Some(constructor) = analyzer.constructors.get(&call.span) else {
				continue;
			};
			let Some(invocation_call) = facts.calls[index + 1..]
				.iter()
				.find(|next| invocation_matches(call, next))
			else {
				// The builder was passed through an opaque wrapper. Avoid a
				// deny-level guess when the actual invocation cannot be associated.
				continue;
			};
			let Some(invocation) = analyzer.invocations.get(&invocation_call.span) else {
				continue;
			};

			// A static `invoke()`/`invoke_signed()` called on the legacy-program
			// builder itself targets SPL Token, which cannot deduct a fee, so the
			// requested amount is exactly what arrives. A wrapper's `invoke()`
			// could target any program and is not exempt.
			let is_static_builder_invoke =
				matches!(invocation_call.method.as_str(), "invoke" | "invoke_signed")
					&& invocation.receiver == Some(constructor.builder);
			if constructor.legacy_program && is_static_builder_invoke {
				continue;
			}

			let written_destination = call
				.args
				.get(constructor.destination_index)
				.and_then(Option::as_deref)
				.unwrap_or_default();
			let destination = constructor.destination.as_deref();
			let is_custody = constructor.kind.is_transfer()
				&& (is_custody_account(written_destination)
					|| destination.is_some_and(is_custody_account));
			if is_custody
				&& analyzer.custody_transfer_is_unaccounted(
					destination,
					invocation_call.span,
					&cpi_spans,
				) {
				lint_custody_transfer(cx, invocation_call.span, written_destination);
				continue;
			}

			let Some(destination) = destination else {
				continue;
			};
			if let Some(stale_use) =
				analyzer.stale_snapshot_use(destination, invocation.hir_id, invocation_call.span)
			{
				lint_stale_snapshot(
					cx,
					invocation_call.span,
					stale_use,
					constructor.kind,
					destination,
				);
			}
		}
	}
}
