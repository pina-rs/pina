extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashMap;
use std::collections::HashSet;

use rustc_hir::BinOpKind;
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
	/// re-read after the CPI before a balance snapshot taken before it is
	/// trusted. After the CPI, a snapshot-derived value may only be compared
	/// with a reload-derived value or a constant, subtracted with a reload to
	/// form a delta (`after.checked_sub(before)`), or added to such a delta.
	/// Destinations named like protocol custody (`vault`, `custody`,
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

/// Crates whose builders are token-program instructions by provenance.
const TOKEN_PROGRAM_CRATES: &[&str] = &[
	"pina",
	"pinocchio_token",
	"pinocchio_token_2022",
	"spl_token",
	"spl_token_2022",
	"spl_token_interface",
];

/// Crates whose `Transfer` builders move lamports, not tokens.
const SYSTEM_PROGRAM_CRATES: &[&str] = &["pinocchio_system", "solana_system_interface"];

/// Pina methods that parse an account's data into a token view without
/// changing which account is read.
const TOKEN_VIEW_METHODS: &[&str] = &[
	"as_account",
	"as_associated_token_account",
	"as_token_2022_account",
	"as_token_account",
	"as_token_account_for_program",
];

/// Associated functions of the token crates' state types (`TokenAccount`,
/// `Mint`, and their Token-2022 equivalents) that parse the account passed
/// as their first argument.
const TOKEN_VIEW_FUNCTIONS: &[&str] = &[
	"from_account_info",
	"from_account_info_unchecked",
	"from_account_view",
	"from_account_view_unchecked",
];

/// `Option`/`Result` adaptors that pass the success value through.
const UNWRAPPING_METHODS: &[&str] = &["expect", "map_err", "ok_or", "ok_or_else", "unwrap"];

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

	/// The fewest leading account parameters a builder must take.
	///
	/// A builder from a token crate is a token instruction by provenance. A
	/// builder defined anywhere else must also name what only a token
	/// instruction has: a transfer must take the mint (`from, mint, to,
	/// authority`), which a lamport transfer never does; a mint takes the
	/// mint first by definition.
	fn minimum_accounts(self, from_token_crate: bool) -> usize {
		if self.is_transfer() && !from_token_crate {
			4
		} else {
			3
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
	/// The destination argument as written, for diagnostics and the custody
	/// name check.
	display: String,
	/// The credited account's key, or `None` when it names no account
	/// precisely.
	destination: Option<AccountKey>,
	builder: DefId,
}

/// A `.invoke*()` call and the builder type it is called on.
struct Invocation {
	hir_id: HirId,
	receiver: Option<DefId>,
	/// The receiver's full type is a builder bound to the legacy SPL Token
	/// program.
	receiver_is_legacy_builder: bool,
}

/// Which account an expression names.
#[derive(Clone)]
struct AccountKey {
	/// Unique within the function: locals are keyed by their binding
	/// `HirId`, and stateful calls by their call site.
	id: String,
	/// The human-readable path, used for the custody name check.
	text: String,
	/// The expression is a parsed token view of the account rather than the
	/// account handle itself.
	view: bool,
}

/// A read of a token balance outside any closure: `.amount()` or
/// `Type::amount(account)`.
struct AmountRead {
	hir_id: HirId,
	span: Span,
	/// `None` when the receiver names no account precisely.
	account: Option<String>,
}

/// A binding or assignment that gives integer locals a new value.
struct Definition<'tcx> {
	bindings: Vec<HirId>,
	/// The whole `let` statement or assignment expression.
	span: Span,
	/// The expression providing the value.
	value: &'tcx Expr<'tcx>,
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
	definitions: Vec<Definition<'tcx>>,
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

/// What a value is judged against: the CPI's destination, the CPI, and the
/// use a reload must dominate.
#[derive(Clone, Copy)]
struct ValueContext<'a> {
	destination: &'a str,
	invocation: Span,
	target: Span,
}

fn is_comparison(operation: &str) -> bool {
	matches!(
		operation,
		"eq" | "ne" | "lt" | "le" | "gt" | "ge" | "cmp" | "partial_cmp"
	)
}

fn is_subtraction_like(operation: &str) -> bool {
	matches!(
		operation,
		"sub" | "checked_sub" | "saturating_sub" | "wrapping_sub" | "overflowing_sub" | "abs_diff"
	)
}

fn is_addition_like(operation: &str) -> bool {
	matches!(
		operation,
		"add" | "checked_add" | "saturating_add" | "wrapping_add" | "overflowing_add"
	)
}

impl<'tcx> Analyzer<'_, 'tcx> {
	fn crate_name(&self, definition: DefId) -> String {
		self.cx.tcx.crate_name(definition.krate).as_str().to_owned()
	}

	fn snippet(&self, span: Span) -> Option<String> {
		self.cx.sess().source_map().span_to_snippet(span).ok()
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

	/// Whether `ty` is a `pinocchio_token` builder bound to the legacy
	/// `TokenProgram`. Such a builder's static `invoke()` calls
	/// `invoke_with_unverified_program(&TokenProgram::ID)`, the legacy SPL
	/// Token program, which has no transfer-fee extension.
	fn is_legacy_builder(&self, ty: Ty<'tcx>) -> bool {
		let TyKind::Adt(builder, generics) = ty.peel_refs().kind() else {
			return false;
		};

		self.crate_name(builder.did()) == "pinocchio_token"
			&& generics.types().last().is_none_or(|program| {
				program.ty_adt_def().is_some_and(|program| {
					self.crate_name(program.did()) == "pinocchio_token"
						&& self.cx.tcx.item_name(program.did()).as_str() == "TokenProgram"
				})
			})
	}

	/// Whether an argument is fixed at compile time: a literal, `()`, or a
	/// path to a constant or static, optionally borrowed, negated, or cast.
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

	/// Whether a method call advances a cursor, so each call returns the next
	/// account: `Iterator::{next, nth}`, `DoubleEndedIterator::{next_back,
	/// nth_back}`, and Pina's `AccountsCursor::next*`. Other `&mut self`
	/// methods, such as accessors, return the same account on every call.
	fn is_advancing_method(&self, call: &Expr<'_>) -> bool {
		let Some(method) = self.cx.typeck_results().type_dependent_def_id(call.hir_id) else {
			return false;
		};
		let name = self.cx.tcx.item_name(method);
		let name = name.as_str();

		if let Some(trait_id) = self.cx.tcx.trait_of_assoc(method) {
			let trait_name = self.cx.tcx.item_name(trait_id);
			return self.crate_name(trait_id) == "core"
				&& matches!(trait_name.as_str(), "Iterator" | "DoubleEndedIterator")
				&& matches!(name, "next" | "nth" | "next_back" | "nth_back");
		}

		self.crate_name(method) == "pina"
			&& name.starts_with("next")
			&& self
				.cx
				.tcx
				.impl_of_assoc(method)
				.and_then(|implementation| {
					self.cx
						.tcx
						.type_of(implementation)
						.instantiate_identity()
						// Only the ADT's identity is read, which normalization cannot
						// change.
						.skip_normalization()
						.ty_adt_def()
				})
				.is_some_and(|cursor| {
					self.cx.tcx.item_name(cursor.did()).as_str() == "AccountsCursor"
				})
	}

	/// Whether a method returns its receiver's value unchanged: `Option` and
	/// `Result` adaptors that pass the success value through, and Pina's
	/// `assert_*` checks, which return the account they checked.
	fn passes_receiver_through(&self, call: &Expr<'_>, method: &str) -> bool {
		let Some(definition) = self.cx.typeck_results().type_dependent_def_id(call.hir_id) else {
			return false;
		};

		match self.crate_name(definition).as_str() {
			"core" => UNWRAPPING_METHODS.contains(&method),
			"pina" => method.starts_with("assert_"),
			_ => false,
		}
	}

	/// Whether `callee` resolves to a token crate's state loader such as
	/// `TokenAccount::from_account_view`.
	fn is_token_view_function(&self, callee: &Expr<'_>) -> bool {
		let ExprKind::Path(path) = &callee.kind else {
			return false;
		};
		let Res::Def(DefKind::AssocFn, function) = self.cx.qpath_res(path, callee.hir_id) else {
			return false;
		};

		TOKEN_VIEW_FUNCTIONS.contains(&self.cx.tcx.item_name(function).as_str())
			&& TOKEN_PROGRAM_CRATES.contains(&self.crate_name(function).as_str())
	}

	/// Names the account an expression refers to.
	///
	/// Every local is keyed by its binding `HirId`, and fields extend the key
	/// with their full path, so `ctx.user_ata` and `ctx.fee_ata` never
	/// collapse onto `ctx`, and shadowed or pattern-bound locals with the same
	/// name stay distinct. Only a fixed allow-list of steps is looked through:
	/// `let` aliases, `&`, `*`, `?`, Pina's token-view methods, the token
	/// crates' `from_account_view`-style loaders, the `.base` field of a
	/// loaded view, and methods with constant arguments that return the
	/// receiver's type. A stateful method (`&mut self`, such as `next()`)
	/// yields a key unique to its call site. A method with constant arguments
	/// that returns another type becomes part of the key (`get(2)`).
	/// Anything else names no account and matches nothing.
	fn account_key(&self, expr: &'tcx Expr<'tcx>) -> Option<AccountKey> {
		match &expr.kind {
			ExprKind::Path(rustc_hir::QPath::Resolved(None, path)) => {
				let name = path
					.segments
					.iter()
					.map(|segment| segment.ident.name.as_str())
					.collect::<Vec<_>>()
					.join("::");
				let Res::Local(binding) = path.res else {
					return Some(AccountKey {
						id: name.clone(),
						text: name,
						view: false,
					});
				};
				if let Some(initializer) = self.let_initializers.get(&binding) {
					return self.account_key(initializer);
				}

				Some(AccountKey {
					id: format!("{name}#{}", binding.local_id.as_u32()),
					text: name,
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
					id: format!("{}.{field}", base.id),
					text: format!("{}.{field}", base.text),
					view: false,
				})
			}
			ExprKind::Index(base, index, _) => {
				let base = self.account_key(base)?;
				if base.view || !matches!(index.kind, ExprKind::Lit(_)) {
					return None;
				}
				let index = self.snippet(index.span)?;

				Some(AccountKey {
					id: format!("{}[{index}]", base.id),
					text: format!("{}[{index}]", base.text),
					view: false,
				})
			}
			ExprKind::Unary(rustc_hir::UnOp::Deref, inner)
			| ExprKind::AddrOf(_, _, inner)
			| ExprKind::DropTemps(inner)
			| ExprKind::Type(inner, _)
			| ExprKind::Match(inner, _, MatchSource::TryDesugar(_)) => self.account_key(inner),
			ExprKind::Call(callee, arguments) => {
				if let Some(argument) = shared::try_branch_argument(self.cx, expr) {
					return self.account_key(argument);
				}
				let [account, ..] = arguments else {
					return None;
				};
				if !self.is_token_view_function(callee) {
					return None;
				}
				let account = self.account_key(account)?;

				Some(AccountKey {
					view: true,
					..account
				})
			}
			ExprKind::MethodCall(segment, receiver, arguments, _) => {
				let receiver_key = self.account_key(receiver)?;
				let method = segment.ident.name.as_str();
				if TOKEN_VIEW_METHODS.contains(&method) && !receiver_key.view {
					return Some(AccountKey {
						view: true,
						..receiver_key
					});
				}
				if self.is_advancing_method(expr) {
					return Some(AccountKey {
						id: format!(
							"{}.{method}@{}",
							receiver_key.id,
							expr.hir_id.local_id.as_u32()
						),
						text: format!("{}.{method}()", receiver_key.text),
						view: false,
					});
				}
				if self.passes_receiver_through(expr, method) {
					return Some(receiver_key);
				}
				if !arguments.iter().all(|argument| self.is_constant(argument)) {
					return None;
				}

				let definition = self
					.cx
					.typeck_results()
					.type_dependent_def_id(expr.hir_id)?;
				let arguments = arguments
					.iter()
					.map(|argument| self.snippet(argument.span))
					.collect::<Option<Vec<_>>>()?
					.join(", ");
				Some(AccountKey {
					id: format!("{}.{definition:?}({arguments})", receiver_key.id),
					text: format!("{}.{method}({arguments})", receiver_key.text),
					view: false,
				})
			}
			ExprKind::Block(block, _) => block.expr.and_then(|tail| self.account_key(tail)),
			_ => None,
		}
	}

	/// Classifies a call by the type it returns and by its signature.
	///
	/// The resolved builder type (after unwrapping `Result`/`Option`) must end
	/// in a token instruction name, and the constructor must have the token
	/// builder shape: leading parameters that are references to a struct or
	/// generic type (see [`TokenCpiKind::minimum_accounts`]) followed by an
	/// integer amount.
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

		let builder = self.core_adt(self.cx.typeck_results().expr_ty(expr))?;
		let builder_crate = self.crate_name(builder);
		if SYSTEM_PROGRAM_CRATES.contains(&builder_crate.as_str()) {
			return None;
		}
		let kind = TokenCpiKind::from_type_name(self.cx.tcx.item_name(builder).as_str())?;

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
		let from_token_crate = TOKEN_PROGRAM_CRATES.contains(&builder_crate.as_str());
		if leading_accounts < kind.minimum_accounts(from_token_crate) || !has_amount {
			return None;
		}

		let destination_index = kind.destination_index(leading_accounts);
		let destination = args.get(destination_index)?;
		let display = self.snippet(destination.span)?;
		let display = display
			.trim_start_matches("&mut ")
			.trim_start_matches('&')
			.to_owned();

		Some(TokenCpiConstructor {
			kind,
			destination_index,
			display,
			destination: self.account_key(destination),
			builder,
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
			value,
		});
	}

	fn reads_of<'a>(&'a self, destination: &'a str) -> impl Iterator<Item = &'a AmountRead> {
		self.amount_reads
			.iter()
			.filter(move |read| read.account.as_deref() == Some(destination))
	}

	/// Definitions made before the CPI whose value is snapshot-derived.
	fn snapshot_definitions(&self, value: &ValueContext<'_>) -> Vec<&Definition<'tcx>> {
		self.definitions
			.iter()
			.filter(|definition| precedes(definition.span, value.invocation))
			.filter(|definition| self.is_snapshot_derived(definition.value, value, 0))
			.collect()
	}

	/// The lexically latest definition of `binding` before `point`.
	fn reaching_definition(&self, binding: HirId, point: Span) -> Option<&Definition<'tcx>> {
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
							matches!(operator.node, BinOpKind::And | BinOpKind::Or)
								&& right.hir_id == child
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

	/// Peels expressions that pass a value through unchanged: casts, `?`,
	/// and `Option`/`Result` unwrapping adaptors.
	fn peel_value(&self, mut expr: &'tcx Expr<'tcx>) -> &'tcx Expr<'tcx> {
		loop {
			expr = match &expr.kind {
				ExprKind::DropTemps(inner)
				| ExprKind::Cast(inner, _)
				| ExprKind::Type(inner, _)
				| ExprKind::Match(inner, _, MatchSource::TryDesugar(_)) => inner,
				ExprKind::Call(..) => {
					match shared::try_branch_argument(self.cx, expr) {
						Some(inner) => inner,
						None => return expr,
					}
				}
				ExprKind::MethodCall(segment, receiver, ..)
					if UNWRAPPING_METHODS.contains(&segment.ident.name.as_str()) =>
				{
					receiver
				}
				_ => return expr,
			};
		}
	}

	/// Splits integer arithmetic into its operation name and operands:
	/// `a - b` is `("sub", [a, b])`, `a.checked_sub(b)` and
	/// `u64::checked_sub(a, b)` are `("checked_sub", [a, b])`.
	fn arithmetic_parts(&self, expr: &'tcx Expr<'tcx>) -> Option<(String, Vec<&'tcx Expr<'tcx>>)> {
		match &expr.kind {
			ExprKind::Binary(operator, left, right) => {
				let name = match operator.node {
					BinOpKind::Add => "add",
					BinOpKind::Sub => "sub",
					BinOpKind::Mul => "mul",
					BinOpKind::Div => "div",
					BinOpKind::Rem => "rem",
					BinOpKind::Eq => "eq",
					BinOpKind::Ne => "ne",
					BinOpKind::Lt => "lt",
					BinOpKind::Le => "le",
					BinOpKind::Gt => "gt",
					BinOpKind::Ge => "ge",
					BinOpKind::BitAnd => "bitand",
					BinOpKind::BitOr => "bitor",
					BinOpKind::BitXor => "bitxor",
					BinOpKind::Shl => "shl",
					BinOpKind::Shr => "shr",
					BinOpKind::And | BinOpKind::Or => return None,
				};
				Some((name.to_owned(), vec![*left, *right]))
			}
			ExprKind::MethodCall(segment, receiver, arguments, _) => {
				let integer_receiver = self
					.cx
					.typeck_results()
					.expr_ty(receiver)
					.peel_refs()
					.is_integral();
				integer_receiver.then(|| {
					let mut operands = vec![*receiver];
					operands.extend(arguments.iter());
					(segment.ident.name.as_str().to_owned(), operands)
				})
			}
			ExprKind::Call(callee, arguments) => {
				let ExprKind::Path(path) = &callee.kind else {
					return None;
				};
				let Res::Def(DefKind::AssocFn, function) = self.cx.qpath_res(path, callee.hir_id)
				else {
					return None;
				};
				let integer_first = arguments.first().is_some_and(|first| {
					self.cx
						.typeck_results()
						.expr_ty(first)
						.peel_refs()
						.is_integral()
				});
				integer_first.then(|| {
					(
						self.cx.tcx.item_name(function).as_str().to_owned(),
						arguments.iter().collect(),
					)
				})
			}
			_ => None,
		}
	}

	/// The reaching definition of a local used in `expr`, when `expr` is a
	/// local path.
	fn local_definition(&self, expr: &'tcx Expr<'tcx>) -> Option<&Definition<'tcx>> {
		let binding = local_path(expr)?;
		self.reaching_definition(binding, expr.span)
	}

	/// A post-CPI balance of the destination: a read after the CPI that
	/// dominates `value.target`, a local defined as one, or a cast of one.
	/// Arithmetic over a reload is not itself a reload.
	fn is_reload(&self, expr: &'tcx Expr<'tcx>, value: &ValueContext<'_>, depth: usize) -> bool {
		if depth > 16 {
			return false;
		}
		let expr = self.peel_value(expr);
		if self.amount_reads.iter().any(|read| {
			read.hir_id == expr.hir_id
				&& read.account.as_deref() == Some(value.destination)
				&& precedes(value.invocation, read.span)
				&& self.dominates(read.hir_id, value.target)
		}) {
			return true;
		}

		self.local_definition(expr).is_some_and(|definition| {
			precedes(value.invocation, definition.span)
				&& self.is_reload(definition.value, value, depth + 1)
		})
	}

	/// A delta: a subtraction-like operation (`-`, `checked_sub`,
	/// `saturating_sub`, `wrapping_sub`, `overflowing_sub`, `abs_diff`, in
	/// method or `u64::checked_sub(a, b)` form) between a snapshot-derived
	/// operand and a reload, or a local defined as one after the CPI.
	fn is_delta(&self, expr: &'tcx Expr<'tcx>, value: &ValueContext<'_>, depth: usize) -> bool {
		if depth > 16 {
			return false;
		}
		let expr = self.peel_value(expr);
		if let Some(definition) = self.local_definition(expr) {
			return precedes(value.invocation, definition.span)
				&& self.is_delta(definition.value, value, depth + 1);
		}
		let Some((operation, operands)) = self.arithmetic_parts(expr) else {
			return false;
		};
		let [left, right] = operands[..] else {
			return false;
		};

		is_subtraction_like(&operation)
			&& ((self.is_snapshot_derived(left, value, depth + 1)
				&& self.is_reload(right, value, depth + 1))
				|| (self.is_reload(left, value, depth + 1)
					&& self.is_snapshot_derived(right, value, depth + 1)))
	}

	/// A reload-derived value: a reload, a delta, or a local, cast, or
	/// arithmetic result computed from one.
	fn is_reload_derived(
		&self,
		expr: &'tcx Expr<'tcx>,
		value: &ValueContext<'_>,
		depth: usize,
	) -> bool {
		if depth > 16 {
			return false;
		}
		if self.is_reload(expr, value, depth + 1) || self.is_delta(expr, value, depth + 1) {
			return true;
		}
		let expr = self.peel_value(expr);
		if let Some(definition) = self.local_definition(expr) {
			return precedes(value.invocation, definition.span)
				&& self.is_reload_derived(definition.value, value, depth + 1);
		}

		self.arithmetic_parts(expr).is_some_and(|(_, operands)| {
			operands
				.iter()
				.any(|operand| self.is_reload_derived(operand, value, depth + 1))
		})
	}

	/// A snapshot-derived value: a read of the destination before the CPI, a
	/// local defined from one, or a cast or arithmetic result computed from
	/// one. A delta is reload-derived instead.
	fn is_snapshot_derived(
		&self,
		expr: &'tcx Expr<'tcx>,
		value: &ValueContext<'_>,
		depth: usize,
	) -> bool {
		if depth > 16 || self.is_delta(expr, value, depth + 1) {
			return false;
		}
		let expr = self.peel_value(expr);
		if self.amount_reads.iter().any(|read| {
			read.hir_id == expr.hir_id
				&& read.account.as_deref() == Some(value.destination)
				&& precedes(read.span, value.invocation)
		}) {
			return true;
		}
		if let Some(definition) = self.local_definition(expr) {
			return self.is_snapshot_derived(definition.value, value, depth + 1);
		}
		if let ExprKind::Block(block, _) = &expr.kind {
			return block
				.expr
				.is_some_and(|tail| self.is_snapshot_derived(tail, value, depth + 1));
		}

		self.arithmetic_parts(expr).is_some_and(|(_, operands)| {
			operands
				.iter()
				.any(|operand| self.is_snapshot_derived(operand, value, depth + 1))
		})
	}

	/// Follows a snapshot-derived value from `expr` to where it ends up and
	/// returns the span where it is trusted as a balance, if anywhere.
	///
	/// The value may only become:
	/// - one side of a comparison whose other side is reload-derived or a
	///   constant;
	/// - an operand of a subtraction-like operation with a reload, which
	///   yields a delta; or
	/// - an operand of an addition-like operation whose other operand is a
	///   delta.
	///
	/// Casts, `?`, unwrapping, other arithmetic, and `let`/assignment carry
	/// the value on; every other destination (a call argument, a return, a
	/// tuple, a struct field, a store) is a stale use.
	fn stale_flow(
		&self,
		expr: &'tcx Expr<'tcx>,
		value: &ValueContext<'_>,
		depth: usize,
	) -> Option<Span> {
		if depth > 16 {
			return Some(expr.span);
		}
		let mut current = expr;

		loop {
			let parent = self.cx.tcx.parent_hir_node(current.hir_id);
			let Node::Expr(parent_expr) = parent else {
				return self.stale_flow_into_binding(parent, current, value, depth);
			};

			let passes_through = match &parent_expr.kind {
				ExprKind::DropTemps(_) | ExprKind::Cast(..) | ExprKind::Type(..) => true,
				ExprKind::Match(scrutinee, _, MatchSource::TryDesugar(_)) => {
					scrutinee.hir_id == current.hir_id
				}
				ExprKind::Call(..) => {
					shared::try_branch_argument(self.cx, parent_expr)
						.is_some_and(|argument| argument.hir_id == current.hir_id)
				}
				ExprKind::MethodCall(segment, receiver, ..) => {
					receiver.hir_id == current.hir_id
						&& UNWRAPPING_METHODS.contains(&segment.ident.name.as_str())
				}
				_ => false,
			};
			if passes_through {
				current = parent_expr;
				continue;
			}

			if let ExprKind::Assign(target, source, _) = &parent_expr.kind
				&& source.hir_id == current.hir_id
				&& local_path(target).is_some()
			{
				return self.stale_uses_of_definition(parent_expr.span, value, depth);
			}

			let Some((operation, operands)) = self.arithmetic_parts(parent_expr) else {
				return Some(current.span);
			};
			let others = operands
				.iter()
				.filter(|operand| operand.hir_id != current.hir_id)
				.copied()
				.collect::<Vec<_>>();

			if is_comparison(&operation) {
				let verified = others.iter().all(|other| {
					self.is_constant(other) || self.is_reload_derived(other, value, depth + 1)
				});
				return (!verified).then_some(current.span);
			}
			if is_subtraction_like(&operation)
				&& others
					.iter()
					.any(|other| self.is_reload(other, value, depth + 1))
			{
				return None;
			}
			if is_addition_like(&operation)
				&& others
					.iter()
					.any(|other| self.is_delta(other, value, depth + 1))
			{
				return None;
			}

			// Any other arithmetic keeps the result snapshot-derived.
			current = parent_expr;
		}
	}

	/// A snapshot-derived value bound by `let` or assigned to a local flows
	/// on through that local's later uses.
	fn stale_flow_into_binding(
		&self,
		parent: Node<'tcx>,
		current: &'tcx Expr<'tcx>,
		value: &ValueContext<'_>,
		depth: usize,
	) -> Option<Span> {
		let Node::LetStmt(local) = parent else {
			return Some(current.span);
		};
		if local
			.init
			.is_none_or(|initializer| initializer.hir_id != current.hir_id)
		{
			return Some(current.span);
		}
		if matches!(local.pat.kind, PatKind::Wild) {
			return None;
		}
		if !matches!(local.pat.kind, PatKind::Binding(_, _, _, None)) {
			return Some(current.span);
		}

		self.stale_uses_of_definition(local.span, value, depth)
	}

	fn stale_uses_of_definition(
		&self,
		span: Span,
		value: &ValueContext<'_>,
		depth: usize,
	) -> Option<Span> {
		// Only integer locals are tracked; a snapshot-derived value bound to
		// anything else (an `Option`, a struct) is treated as trusted there.
		let Some(definition) = self
			.definitions
			.iter()
			.find(|definition| definition.span == span)
		else {
			return Some(span);
		};

		self.local_uses
			.iter()
			.filter(|usage| definition.bindings.contains(&usage.binding))
			.filter(|usage| {
				self.reaching_definition(usage.binding, usage.span)
					.is_some_and(|reaching| std::ptr::eq(reaching, definition))
			})
			.find_map(|usage| {
				let use_expr = self.cx.tcx.hir_expect_expr(usage.hir_id);
				let nested = ValueContext {
					target: usage.span,
					..*value
				};
				self.stale_flow(use_expr, &nested, depth + 1)
			})
	}

	/// The snapshot tier, which applies to every destination: returns where
	/// a pre-CPI balance snapshot of `destination` is trusted after a CPI
	/// that can reach it.
	fn stale_snapshot_use(
		&self,
		destination: &str,
		invocation: HirId,
		invocation_span: Span,
	) -> Option<Span> {
		let context = ValueContext {
			destination,
			invocation: invocation_span,
			target: invocation_span,
		};
		let snapshots = self.snapshot_definitions(&context);

		self.local_uses
			.iter()
			.filter(|usage| precedes(invocation_span, usage.span))
			.filter(|usage| {
				self.reaching_definition(usage.binding, usage.span)
					.is_some_and(|definition| {
						snapshots
							.iter()
							.any(|snapshot| std::ptr::eq(*snapshot, definition))
					})
			})
			.filter(|usage| self.reaches(invocation, usage.span))
			.find_map(|usage| {
				let use_expr = self.cx.tcx.hir_expect_expr(usage.hir_id);
				let context = ValueContext {
					target: usage.span,
					..context
				};
				self.stale_flow(use_expr, &context, 0)
			})
	}

	/// The custody tier: a transfer into a custody-named account must be
	/// bracketed by destination reads with no other CPI in between.
	fn custody_transfer_is_unaccounted(
		&self,
		destination: &str,
		invocation: Span,
		cpi_spans: &[Span],
	) -> bool {
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
		let account = self.account_key(account).map(|key| key.id);
		self.amount_reads.push(AmountRead {
			hir_id: expr.hir_id,
			span: expr.span,
			account,
		});
	}
}

impl<'tcx> Visitor<'tcx> for Analyzer<'_, 'tcx> {
	fn visit_local(&mut self, local: &'tcx rustc_hir::LetStmt<'tcx>) {
		if let Some(initializer) = local.init {
			if let PatKind::Binding(_, binding, _, None) = local.pat.kind
				&& local.els.is_none()
			{
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
					let receiver_ty = self.cx.typeck_results().expr_ty(receiver);
					self.invocations.insert(
						expr.span,
						Invocation {
							hir_id: expr.hir_id,
							receiver: receiver_ty
								.peel_refs()
								.ty_adt_def()
								.map(|definition| definition.did()),
							receiver_is_legacy_builder: self.is_legacy_builder(receiver_ty),
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
						value,
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

/// The custody check as the name-based lint on `main` performs it: reads
/// matched by their written receiver text, ordered by collected call
/// position.
///
/// Used only where the typed identity names no account, so code the
/// name-based check accepted is not newly rejected for that reason alone.
fn textual_custody_is_accounted(
	calls: &[shared::CallInfo],
	invocation: usize,
	written: &str,
) -> bool {
	let reads_amount = |candidate: &shared::CallInfo| {
		candidate.method == "amount" && candidate.receiver.as_deref() == Some(written)
	};
	let has_before = calls[..invocation]
		.iter()
		.rposition(reads_amount)
		.is_some_and(|before| {
			!calls[before + 1..invocation]
				.iter()
				.any(|call| is_cpi_method(&call.method))
		});
	let has_after = calls[invocation + 1..]
		.iter()
		.position(reads_amount)
		.is_some_and(|offset| {
			!calls[invocation + 1..invocation + 1 + offset]
				.iter()
				.any(|call| is_cpi_method(&call.method))
		});

	has_before && has_after
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
		let unknown_read_spans = analyzer
			.amount_reads
			.iter()
			.filter(|read| read.account.is_none())
			.map(|read| read.span)
			.collect::<HashSet<_>>();

		for (index, call) in facts.calls.iter().enumerate() {
			let Some(constructor) = analyzer.constructors.get(&call.span) else {
				continue;
			};
			let Some(offset) = facts.calls[index + 1..]
				.iter()
				.position(|next| invocation_matches(call, next))
			else {
				// The builder was passed through an opaque wrapper. Avoid a
				// deny-level guess when the actual invocation cannot be associated.
				continue;
			};
			let invocation_index = index + 1 + offset;
			let invocation_call = &facts.calls[invocation_index];
			let Some(invocation) = analyzer.invocations.get(&invocation_call.span) else {
				continue;
			};

			// A static `invoke()`/`invoke_signed()` whose receiver's full type is
			// the legacy-program builder targets SPL Token, which cannot deduct a
			// fee, so the requested amount is exactly what arrives. A wrapper's
			// `invoke()`, or any expression that yields a Token-2022 builder,
			// is not exempt.
			let is_static_legacy_invoke =
				matches!(invocation_call.method.as_str(), "invoke" | "invoke_signed")
					&& invocation.receiver == Some(constructor.builder)
					&& invocation.receiver_is_legacy_builder;
			if is_static_legacy_invoke {
				continue;
			}

			let written = call
				.args
				.get(constructor.destination_index)
				.and_then(Option::as_deref);
			let destination = constructor.destination.as_ref();
			let is_custody = constructor.kind.is_transfer()
				&& (is_custody_account(&constructor.display)
					|| destination.is_some_and(|key| is_custody_account(&key.text)));
			if is_custody {
				let typed_unaccounted = destination.is_none_or(|key| {
					analyzer.custody_transfer_is_unaccounted(
						&key.id,
						invocation_call.span,
						&cpi_spans,
					)
				});
				// Where the typed identity names no account (the destination or a
				// matching read), fall back to the name-based check `main` runs.
				let identity_unknown = destination.is_none()
					|| facts.calls.iter().any(|read| {
						read.method == "amount"
							&& read.receiver.as_deref() == written
							&& unknown_read_spans.contains(&read.span)
					});
				let textually_accounted = identity_unknown
					&& written.is_some_and(|written| {
						textual_custody_is_accounted(&facts.calls, invocation_index, written)
					});
				if typed_unaccounted && !textually_accounted {
					lint_custody_transfer(cx, invocation_call.span, &constructor.display);
					continue;
				}
			}

			let Some(destination) = destination else {
				continue;
			};
			if let Some(stale_use) = analyzer.stale_snapshot_use(
				&destination.id,
				invocation.hir_id,
				invocation_call.span,
			) {
				lint_stale_snapshot(
					cx,
					invocation_call.span,
					stale_use,
					constructor.kind,
					&constructor.display,
				);
			}
		}
	}
}
