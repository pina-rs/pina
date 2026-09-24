extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::HirId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::intravisit::Visitor;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_middle::ty::TyKind;
use rustc_span::Span;

use crate::diagnostics;
use crate::shared;

crate::declare_late_lint! {
	/// ### What it does
	///
	/// Requires the destination of every value-moving token CPI (`Transfer`,
	/// `TransferChecked`, `MintTo`, and `MintToChecked`) to be re-read after
	/// the CPI when a balance snapshot taken before it is used afterwards.
	/// Destinations named like protocol custody (`vault`, `custody`,
	/// `reserve`, or `pool`) must additionally be read both before and after
	/// every transfer, even when no snapshot exists yet.
	///
	/// ### Why is this bad?
	///
	/// Token-2022 transfer fees can make the amount received differ from the
	/// requested amount, and any CPI into an account makes an earlier read of
	/// its balance stale. Accounting must use the balance observed after the
	/// CPI, never a snapshot taken before it.
	pub REQUIRE_POST_CPI_BALANCE_RELOAD,
	Deny,
	"token balances snapshotted before a value-moving CPI must be reloaded after it"
}

/// A token-program instruction that increases its destination's balance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TokenCpiKind {
	Transfer,
	TransferChecked,
	MintTo,
	MintToChecked,
}

impl TokenCpiKind {
	fn from_type_name(name: &str) -> Option<Self> {
		match name {
			"Transfer" => Some(Self::Transfer),
			"TransferChecked" => Some(Self::TransferChecked),
			"MintTo" => Some(Self::MintTo),
			"MintToChecked" => Some(Self::MintToChecked),
			_ => None,
		}
	}

	/// Position of the credited account in the `new` and
	/// `with_multisig_signers` constructors.
	fn destination_index(self) -> usize {
		match self {
			Self::TransferChecked => 2,
			Self::Transfer | Self::MintTo | Self::MintToChecked => 1,
		}
	}

	/// Accounts plus the amount (and decimals) every token constructor takes.
	///
	/// The system program's lamport `Transfer` shares a type name but takes
	/// only two accounts and an amount, so arity keeps it out.
	fn minimum_arity(self) -> usize {
		match self {
			Self::Transfer | Self::MintTo => 4,
			Self::MintToChecked => 5,
			Self::TransferChecked => 6,
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
	/// The credited account, with local aliases resolved.
	destination: String,
	/// The builder is bound to the legacy SPL Token program, which cannot
	/// deduct a fee, when it is invoked without an explicit program.
	legacy_program: bool,
}

/// A `.amount()` read, with the receiver's local aliases resolved.
struct AmountRead {
	span: Span,
	identity: String,
}

/// A `let` statement that binds at least one integer local.
struct IntegerLet {
	bindings: Vec<HirId>,
	statement: Span,
	initializer: Span,
}

/// Collects the type-resolved facts the snapshot analysis needs in one pass.
struct Analyzer<'cx, 'tcx, 'facts> {
	cx: &'cx LateContext<'tcx>,
	aliases: &'facts HashMap<HirId, shared::AliasInfo>,
	constructors: HashMap<Span, TokenCpiConstructor>,
	amount_reads: Vec<AmountRead>,
	integer_lets: Vec<IntegerLet>,
	local_uses: Vec<(HirId, Span)>,
}

impl<'tcx> Analyzer<'_, 'tcx, '_> {
	/// Resolves `let` aliases so `let account = self.vault.as_token_account()?`
	/// and `self.vault` name the same destination.
	fn canonical_identity(&self, expr: &Expr<'_>) -> Option<String> {
		let mut identity = shared::expression_identity(expr)?;
		let mut binding = shared::expression_local_binding(expr);

		while let Some(alias) = binding.and_then(|binding| self.aliases.get(&binding)) {
			identity.clone_from(&alias.identity);
			binding = alias.binding;
		}

		Some(identity)
	}

	/// Classifies a call by the type it returns rather than by the spelling of
	/// its path, so re-exports, aliases, and `use ... as` imports all resolve.
	fn token_cpi_constructor(
		&self,
		expr: &Expr<'tcx>,
		callee: &Expr<'tcx>,
		args: &[Expr<'tcx>],
	) -> Option<TokenCpiConstructor> {
		let ExprKind::Path(path) = &callee.kind else {
			return None;
		};
		let rustc_hir::def::Res::Def(rustc_hir::def::DefKind::AssocFn, function) =
			self.cx.qpath_res(path, callee.hir_id)
		else {
			return None;
		};
		if !matches!(
			self.cx.tcx.item_name(function).as_str(),
			"new" | "with_multisig_signers"
		) {
			return None;
		}

		let TyKind::Adt(builder, generics) = self.cx.typeck_results().expr_ty(expr).kind() else {
			return None;
		};
		let kind = TokenCpiKind::from_type_name(self.cx.tcx.item_name(builder.did()).as_str())
			.filter(|kind| args.len() >= kind.minimum_arity())?;
		let destination = self.canonical_identity(args.get(kind.destination_index())?)?;

		// `pina::token_2022` re-exports these builders as aliases of the
		// `pinocchio_token` structs with a `Token2022Program` parameter, so the
		// crate alone does not identify the legacy program: the program type
		// parameter must be `pinocchio_token::TokenProgram` too.
		let legacy_program = self.is_pinocchio_token_item(builder.did(), None)
			&& generics.types().last().is_none_or(|program| {
				program.ty_adt_def().is_some_and(|program| {
					self.is_pinocchio_token_item(program.did(), Some("TokenProgram"))
				})
			});

		Some(TokenCpiConstructor {
			kind,
			destination,
			legacy_program,
		})
	}

	fn is_pinocchio_token_item(
		&self,
		definition: rustc_hir::def_id::DefId,
		name: Option<&str>,
	) -> bool {
		self.cx.tcx.crate_name(definition.krate).as_str() == "pinocchio_token"
			&& name.is_none_or(|name| self.cx.tcx.item_name(definition).as_str() == name)
	}
}

impl<'tcx> Visitor<'tcx> for Analyzer<'_, 'tcx, '_> {
	fn visit_local(&mut self, local: &'tcx rustc_hir::LetStmt<'tcx>) {
		if let Some(initializer) = local.init {
			let typeck = self.cx.typeck_results();
			let mut bindings = Vec::new();
			local.pat.each_binding(|_, binding, _, _| {
				if typeck.node_type(binding).is_integral() {
					bindings.push(binding);
				}
			});

			if !bindings.is_empty() {
				self.integer_lets.push(IntegerLet {
					bindings,
					statement: local.span,
					initializer: initializer.span,
				});
			}
		}

		rustc_hir::intravisit::walk_local(self, local);
	}

	fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
		match &expr.kind {
			ExprKind::Call(callee, args) => {
				if let Some(constructor) = self.token_cpi_constructor(expr, callee, args) {
					self.constructors.insert(expr.span, constructor);
				}
			}
			ExprKind::MethodCall(segment, receiver, [], _)
				if segment.ident.name.as_str() == "amount" =>
			{
				if let Some(identity) = self.canonical_identity(receiver) {
					self.amount_reads.push(AmountRead {
						span: expr.span,
						identity,
					});
				}
			}
			ExprKind::Path(rustc_hir::QPath::Resolved(None, path)) => {
				if let rustc_hir::def::Res::Local(binding) = path.res {
					self.local_uses.push((binding, expr.span));
				}
			}
			ExprKind::Closure(closure) => {
				// A snapshot captured by a closure is still used where the
				// closure runs; nested bodies are not visited by default.
				self.visit_body(self.cx.tcx.hir_body(closure.body));
			}
			_ => {}
		}

		rustc_hir::intravisit::walk_expr(self, expr);
	}
}

/// Whether `earlier` ends before `later` starts in the source the user wrote.
///
/// Macro and desugaring spans are mapped to their call site first so `?` and
/// assertion macros order like the code around them.
fn precedes(earlier: Span, later: Span) -> bool {
	earlier.source_callsite().hi() <= later.source_callsite().lo()
}

fn is_custody_account(identity: &str) -> bool {
	let name = identity.to_ascii_lowercase();
	["vault", "custody", "reserve", "pool"]
		.iter()
		.any(|part| name.contains(part))
}

fn is_cpi_invocation(call: &shared::CallInfo) -> bool {
	matches!(
		call.method.as_str(),
		"invoke"
			| "invoke_signed"
			| "invoke_with_program"
			| "invoke_signed_with_program"
			| "invoke_with_unverified_program"
			| "invoke_signed_with_unverified_program"
	)
}

/// Invocations that name the token program at runtime and can therefore
/// reach Token-2022 even when the builder defaults to the legacy program.
fn is_dynamic_token_invocation(call: &shared::CallInfo) -> bool {
	is_cpi_invocation(call) && !matches!(call.method.as_str(), "invoke" | "invoke_signed")
}

fn invocation_matches(constructor: &shared::CallInfo, invocation: &shared::CallInfo) -> bool {
	if !is_cpi_invocation(invocation) {
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

/// The custody tier: a transfer into a custody-named account must be
/// bracketed by destination reads with no other CPI in between.
fn custody_transfer_is_unaccounted(
	calls: &[shared::CallInfo],
	invocation: usize,
	destination: &str,
) -> bool {
	let reads_amount = |candidate: &shared::CallInfo| {
		candidate.method == "amount" && candidate.receiver.as_deref() == Some(destination)
	};
	let before = calls[..invocation].iter().rposition(reads_amount);
	let has_before =
		before.is_some_and(|before| !calls[before + 1..invocation].iter().any(is_cpi_invocation));
	let after = calls[invocation + 1..]
		.iter()
		.position(reads_amount)
		.map(|offset| invocation + 1 + offset);
	let has_after =
		after.is_some_and(|after| !calls[invocation + 1..after].iter().any(is_cpi_invocation));

	!(has_before && has_after)
}

/// The snapshot tier, which applies to every destination regardless of its
/// name: returns where an integer snapshot of the destination's balance,
/// bound before the CPI, is used after it without an intervening reload.
fn stale_snapshot_use(
	analyzer: &Analyzer<'_, '_, '_>,
	destination: &str,
	invocation: Span,
	next_cpi: Option<Span>,
) -> Option<Span> {
	let reads_destination = |read: &&AmountRead| read.identity == destination;
	let reloaded = analyzer
		.amount_reads
		.iter()
		.filter(reads_destination)
		.any(|read| {
			precedes(invocation, read.span) && next_cpi.is_none_or(|next| precedes(read.span, next))
		});
	if reloaded {
		return None;
	}

	analyzer
		.integer_lets
		.iter()
		.filter(|snapshot| precedes(snapshot.statement, invocation))
		.filter(|snapshot| {
			analyzer
				.amount_reads
				.iter()
				.filter(reads_destination)
				.any(|read| snapshot.initializer.contains(read.span))
		})
		.flat_map(|snapshot| &snapshot.bindings)
		.find_map(|binding| {
			analyzer
				.local_uses
				.iter()
				.find(|(used, span)| used == binding && precedes(invocation, *span))
				.map(|(_, span)| *span)
		})
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
			aliases: &facts.aliases,
			constructors: HashMap::new(),
			amount_reads: Vec::new(),
			integer_lets: Vec::new(),
			local_uses: Vec::new(),
		};
		analyzer.visit_body(body);

		for (index, call) in facts.calls.iter().enumerate() {
			let Some(constructor) = analyzer.constructors.get(&call.span) else {
				continue;
			};

			let invocation = facts.calls[index + 1..]
				.iter()
				.position(|next| invocation_matches(call, next));
			let Some(invocation) = invocation.map(|offset| index + 1 + offset) else {
				// The builder was passed through an opaque wrapper. Avoid a
				// deny-level guess when the actual invocation cannot be associated.
				continue;
			};
			let invocation_span = facts.calls[invocation].span;
			if constructor.legacy_program && !is_dynamic_token_invocation(&facts.calls[invocation])
			{
				continue;
			}

			// The custody tier keeps its historical destination spelling: the
			// argument exactly as written, matched against direct receivers.
			let custody_destination = call
				.args
				.get(constructor.kind.destination_index())
				.and_then(Option::as_deref)
				.filter(|destination| {
					constructor.kind.is_transfer() && is_custody_account(destination)
				});
			if let Some(destination) = custody_destination
				&& custody_transfer_is_unaccounted(&facts.calls, invocation, destination)
			{
				lint_custody_transfer(cx, invocation_span, destination);
				continue;
			}

			let next_cpi = facts.calls[invocation + 1..]
				.iter()
				.find(|next| is_cpi_invocation(next))
				.map(|next| next.span);
			if let Some(stale_use) = stale_snapshot_use(
				&analyzer,
				&constructor.destination,
				invocation_span,
				next_cpi,
			) {
				lint_stale_snapshot(
					cx,
					invocation_span,
					stale_use,
					constructor.kind,
					&constructor.destination,
				);
			}
		}
	}
}
