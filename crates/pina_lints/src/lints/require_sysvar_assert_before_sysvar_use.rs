extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashSet;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::HirId;
use rustc_hir::Node;
use rustc_hir::def::Res;
use rustc_hir::intravisit::FnKind;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_lint::LintContext;

use crate::shared;

crate::declare_late_lint! {
	/// ### What it does
	///
	/// Warns when raw sysvar-like accounts are read without asserting their
	/// sysvar identity first.
	///
	/// ### Why is this bad?
	///
	/// Spoofed sysvar accounts can distort rent, clock, and instruction-data
	/// logic. Pinocchio's checked `from_account_view()` and `try_from()` sysvar
	/// loaders perform the identity check while they parse the account, so they
	/// do not require a separate assertion.
	///
	/// ### Example
	///
	/// ```ignore
	/// // See lints/readme.md for the preferred pattern.
	/// ```
	pub REQUIRE_SYSVAR_ASSERT_BEFORE_SYSVAR_USE,
	Deny,
	"raw sysvar access should be preceded by `assert_sysvar()` on the same account"
}

const TARGET_NEEDLES: &[&str] = &["process", "process_instruction", "instruction", "sysvar"];
const KNOWN_SYSVAR_NAMES: &[&str] = &[
	"clock",
	"epoch_rewards",
	"epoch_schedule",
	"fees",
	"instructions",
	"last_restart_slot",
	"recent_blockhashes",
	"rent",
	"rewards",
	"slot_hashes",
	"slot_history",
	"stake_history",
];
const TRUSTED_SYSVAR_TYPES: &[&str] = &[
	"pinocchio::sysvars::clock::Clock",
	"pinocchio::sysvars::instructions::Instructions",
	"pinocchio::sysvars::rent::Rent",
	"pinocchio::sysvars::slot_hashes::SlotHashes",
];

fn terminal_identifier(value: &str) -> &str {
	value.rsplit(['.', ':']).next().unwrap_or(value)
}

fn sysvar_identifier(value: &str) -> &str {
	let terminal = terminal_identifier(value);
	terminal.strip_suffix("_account").unwrap_or(terminal)
}

fn normalized_tokens(value: &str) -> Vec<String> {
	value
		.split(|c: char| !c.is_ascii_alphanumeric())
		.filter(|token| !token.is_empty())
		.map(|token| token.to_ascii_lowercase())
		.collect()
}

fn matches_sysvar_id(receiver: &str, asserted_id: &str) -> bool {
	let expected_tokens = normalized_tokens(sysvar_identifier(receiver));
	let asserted_tokens = normalized_tokens(asserted_id);
	if expected_tokens.is_empty() || asserted_tokens.is_empty() {
		return false;
	}

	let mut asserted_iter = asserted_tokens.iter();
	expected_tokens
		.iter()
		.all(|token| asserted_iter.by_ref().any(|candidate| candidate == token))
}

fn is_sysvar_receiver(name: &str) -> bool {
	let terminal = sysvar_identifier(name).to_ascii_lowercase();
	KNOWN_SYSVAR_NAMES.contains(&terminal.as_str())
		|| terminal.ends_with("_sysvar")
		|| terminal.ends_with("_instructions")
}

fn trusted_definition(definition: &shared::TypeDefinition) -> Option<&'static str> {
	(definition.crate_name == "pinocchio")
		.then(|| {
			TRUSTED_SYSVAR_TYPES
				.iter()
				.find_map(|trusted| (definition.path == *trusted).then_some(*trusted))
		})
		.flatten()
}

fn trusted_sysvar_receiver_type(definitions: &[shared::TypeDefinition]) -> Option<&'static str> {
	let outer = definitions.first()?;
	if let Some(trusted) = trusted_definition(outer) {
		return Some(trusted);
	}

	// `Clock::from_account_view` returns Pinocchio's account borrow wrapper,
	// whose `Deref` target is the trusted `Clock`. Do not generally search
	// generic arguments: doing so would misclassify `Result<Clock, _>` methods
	// such as `.unwrap()` as sysvar operations.
	let is_pinocchio_borrow = outer.crate_name == "solana_account_view"
		&& matches!(
			terminal_identifier(&outer.path),
			"Ref" | "RefMut" | "MappedRef" | "MappedRefMut"
		);
	is_pinocchio_borrow
		.then(|| definitions[1..].iter().find_map(trusted_definition))
		.flatten()
}

fn definition_identity(
	tcx: rustc_middle::ty::TyCtxt<'_>,
	definition: rustc_hir::def_id::DefId,
) -> (String, String) {
	(
		tcx.def_path_str(definition),
		tcx.crate_name(definition.krate).as_str().to_string(),
	)
}

fn call_definition<'tcx>(
	tcx: rustc_middle::ty::TyCtxt<'tcx>,
	callee: &'tcx Expr<'tcx>,
) -> Option<rustc_hir::def_id::DefId> {
	let ExprKind::Path(path) = &callee.kind else {
		return None;
	};
	let typeck = tcx.typeck(callee.hir_id.owner.def_id);
	let Res::Def(_, definition) = typeck.qpath_res(path, callee.hir_id) else {
		return None;
	};
	Some(definition)
}

fn method_definition<'tcx>(
	tcx: rustc_middle::ty::TyCtxt<'tcx>,
	expression: &'tcx Expr<'tcx>,
) -> Option<rustc_hir::def_id::DefId> {
	tcx.typeck(expression.hir_id.owner.def_id)
		.type_dependent_def_id(expression.hir_id)
}

fn is_checked_loader_definition(
	tcx: rustc_middle::ty::TyCtxt<'_>,
	definition: rustc_hir::def_id::DefId,
	trusted_type: &str,
) -> bool {
	let (path, crate_name) = definition_identity(tcx, definition);
	if trusted_type.ends_with("::Instructions") {
		return path.ends_with("TryFrom::try_from");
	}

	crate_name == "pinocchio"
		&& path.starts_with(trusted_type)
		&& path.ends_with("::from_account_view")
}

fn is_core_result_operation(
	tcx: rustc_middle::ty::TyCtxt<'_>,
	definition: rustc_hir::def_id::DefId,
	methods: &[&str],
) -> bool {
	let crate_symbol = tcx.crate_name(definition.krate);
	let crate_name = crate_symbol.as_str();
	if !matches!(crate_name, "core" | "std") {
		return false;
	}
	let method = tcx.item_name(definition);
	if !methods.contains(&method.as_str()) {
		return false;
	}
	let path = tcx.def_path_str(definition);
	path.contains("::result::Result")
		|| (matches!(method.as_str(), "branch" | "from_output") && path.contains("::ops::Try::"))
}

fn expression_is_binding(expression: &Expr<'_>, binding: HirId) -> bool {
	match &expression.kind {
		ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => path.res == Res::Local(binding),
		ExprKind::Use(inner, _) | ExprKind::DropTemps(inner) => {
			expression_is_binding(inner, binding)
		}
		ExprKind::Block(block, _) => {
			block
				.expr
				.is_some_and(|tail| expression_is_binding(tail, binding))
		}
		_ => false,
	}
}

fn expression_returns_binding_in_result(expression: &Expr<'_>, binding: HirId) -> bool {
	match &expression.kind {
		ExprKind::Call(_, arguments) => {
			arguments.len() == 1 && expression_is_binding(&arguments[0], binding)
		}
		ExprKind::Use(inner, _) | ExprKind::DropTemps(inner) => {
			expression_returns_binding_in_result(inner, binding)
		}
		ExprKind::Block(block, _) => {
			block
				.expr
				.is_some_and(|tail| expression_returns_binding_in_result(tail, binding))
		}
		_ => false,
	}
}

fn adapter_preserves_success_value(
	tcx: rustc_middle::ty::TyCtxt<'_>,
	method: &str,
	arguments: &[Expr<'_>],
) -> bool {
	if matches!(method, "inspect" | "inspect_err" | "map_err") {
		return true;
	}
	let Some(Expr {
		kind: ExprKind::Closure(closure),
		..
	}) = arguments.first()
	else {
		return false;
	};
	let body = tcx.hir_body(closure.body);
	let [parameter] = body.params else {
		return false;
	};
	let rustc_hir::PatKind::Binding(_, binding, _, None) = parameter.pat.kind else {
		return false;
	};
	match method {
		"map" => expression_is_binding(body.value, binding),
		"and_then" => expression_returns_binding_in_result(body.value, binding),
		_ => false,
	}
}

fn binding_source<'tcx>(
	tcx: rustc_middle::ty::TyCtxt<'tcx>,
	binding: HirId,
) -> Option<&'tcx Expr<'tcx>> {
	let mut inside_match_arm = false;
	for (_, node) in tcx.hir_parent_iter(binding) {
		match node {
			Node::LetStmt(local) => return local.init,
			Node::Expr(Expr {
				kind: ExprKind::Let(let_expression),
				..
			}) => return Some(let_expression.init),
			Node::Arm(_) => inside_match_arm = true,
			Node::Expr(expression) if inside_match_arm => {
				if let ExprKind::Match(scrutinee, ..) = expression.kind {
					return Some(scrutinee);
				}
			}
			Node::Param(_) => return None,
			_ => {}
		}
	}
	None
}

fn closure_adapter_source<'tcx>(
	cx: &LateContext<'tcx>,
	binding: HirId,
) -> Option<&'tcx Expr<'tcx>> {
	let closure_expression = cx.tcx.hir_parent_iter(binding).find_map(|(_, node)| {
		let Node::Expr(expression) = node else {
			return None;
		};
		matches!(expression.kind, ExprKind::Closure(_)).then_some(expression)
	})?;
	let ExprKind::Closure(closure) = closure_expression.kind else {
		unreachable!();
	};
	let body = cx.tcx.hir_body(closure.body);
	let [parameter] = body.params else {
		return None;
	};
	let rustc_hir::PatKind::Binding(_, parameter_binding, _, None) = parameter.pat.kind else {
		return None;
	};
	if parameter_binding != binding {
		return None;
	}

	let Node::Expr(parent) = cx.tcx.parent_hir_node(closure_expression.hir_id) else {
		return None;
	};
	let ExprKind::MethodCall(_, receiver, arguments, _) = &parent.kind else {
		return None;
	};
	if !arguments
		.iter()
		.any(|argument| argument.hir_id == closure_expression.hir_id)
	{
		return None;
	}
	let definition = method_definition(cx.tcx, parent)?;
	is_core_result_operation(cx.tcx, definition, &["map", "and_then", "inspect"])
		.then_some(*receiver)
}

fn expression_has_checked_loader<'tcx>(
	cx: &LateContext<'tcx>,
	facts: &shared::FunctionFacts,
	trusted_type: &str,
	expression: &'tcx Expr<'tcx>,
	use_span: rustc_span::Span,
	visited: &mut HashSet<HirId>,
) -> bool {
	match &expression.kind {
		ExprKind::Call(callee, arguments) => {
			let Some(definition) = call_definition(cx.tcx, callee) else {
				return false;
			};
			if is_checked_loader_definition(cx.tcx, definition, trusted_type) {
				return true;
			}
			let is_transparent_wrapper =
				is_core_result_operation(cx.tcx, definition, &["branch", "from_output"])
					|| (cx.tcx.crate_name(definition.krate).as_str() == "core"
						&& cx.tcx.item_name(definition).as_str() == "Some");
			is_transparent_wrapper
				&& arguments.len() == 1
				&& expression_has_checked_loader(
					cx,
					facts,
					trusted_type,
					&arguments[0],
					use_span,
					visited,
				)
		}
		ExprKind::MethodCall(segment, receiver, arguments, _) => {
			let Some(definition) = method_definition(cx.tcx, expression) else {
				return false;
			};
			let method = segment.ident.name.as_str();
			let preserves_success =
				is_core_result_operation(
					cx.tcx,
					definition,
					&["branch", "from_output", "inspect", "inspect_err", "map_err"],
				) || (is_core_result_operation(cx.tcx, definition, &["map", "and_then"])
					&& adapter_preserves_success_value(cx.tcx, method, arguments));
			preserves_success
				&& expression_has_checked_loader(
					cx,
					facts,
					trusted_type,
					receiver,
					use_span,
					visited,
				)
		}
		ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
			let Res::Local(binding) = path.res else {
				return false;
			};
			if !visited.insert(binding)
				|| facts.assignments.iter().any(|assignment| {
					assignment.binding == Some(binding) && assignment.span.lo() < use_span.lo()
				}) {
				return false;
			}
			binding_source(cx.tcx, binding)
				.or_else(|| closure_adapter_source(cx, binding))
				.is_some_and(|source| {
					expression_has_checked_loader(
						cx,
						facts,
						trusted_type,
						source,
						use_span,
						visited,
					)
				})
		}
		ExprKind::If(_, then, Some(otherwise)) => {
			expression_has_checked_loader(
				cx,
				facts,
				trusted_type,
				then,
				use_span,
				&mut visited.clone(),
			) && expression_has_checked_loader(
				cx,
				facts,
				trusted_type,
				otherwise,
				use_span,
				&mut visited.clone(),
			)
		}
		ExprKind::Match(scrutinee, arms, source) => {
			if matches!(source, rustc_hir::MatchSource::TryDesugar(_)) {
				return expression_has_checked_loader(
					cx,
					facts,
					trusted_type,
					scrutinee,
					use_span,
					visited,
				);
			}
			!arms.is_empty()
				&& arms.iter().all(|arm| {
					expression_has_checked_loader(
						cx,
						facts,
						trusted_type,
						arm.body,
						use_span,
						&mut visited.clone(),
					)
				})
		}
		ExprKind::Block(block, _) => {
			block.expr.is_some_and(|tail| {
				expression_has_checked_loader(cx, facts, trusted_type, tail, use_span, visited)
			})
		}
		ExprKind::Use(inner, _)
		| ExprKind::DropTemps(inner)
		| ExprKind::AddrOf(_, _, inner)
		| ExprKind::Unary(_, inner)
		| ExprKind::Cast(inner, _)
		| ExprKind::Type(inner, _)
		| ExprKind::UnsafeBinderCast(_, inner, _) => {
			expression_has_checked_loader(cx, facts, trusted_type, inner, use_span, visited)
		}
		_ => false,
	}
}

fn has_checked_typed_loader(
	cx: &LateContext<'_>,
	facts: &shared::FunctionFacts,
	call: &shared::CallInfo,
) -> bool {
	let Some(trusted_type) = trusted_sysvar_receiver_type(&call.receiver_type_definitions) else {
		return false;
	};
	let Some(receiver_hir_id) = call.receiver_hir_id else {
		return false;
	};
	let Node::Expr(receiver) = cx.tcx.hir_node(receiver_hir_id) else {
		return false;
	};
	expression_has_checked_loader(
		cx,
		facts,
		trusted_type,
		receiver,
		call.span,
		&mut HashSet::new(),
	)
}

fn emit_unchecked_sysvar(cx: &LateContext<'_>, span: rustc_span::Span) {
	cx.lint(REQUIRE_SYSVAR_ASSERT_BEFORE_SYSVAR_USE, |diag| {
		diag.span(span);
		diag.primary_message(
			"raw sysvar access should be preceded by `assert_sysvar()` on the same account",
		);
		diag.help(
			"use the sysvar type's checked `from_account_view()` or `try_from()` loader, or call \
			 `sysvar_account.assert_sysvar(&sysvar::ID)?` before borrowing raw data",
		);
	});
}

impl<'tcx> LateLintPass<'tcx> for RequireSysvarAssertBeforeSysvarUse {
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

		let facts = shared::collect_function_facts(cx, body);
		for (index, call) in facts.calls.iter().enumerate() {
			if call.method == "assert_sysvar" {
				continue;
			}

			if trusted_sysvar_receiver_type(&call.receiver_type_definitions).is_some() {
				if !has_checked_typed_loader(cx, &facts, call) {
					emit_unchecked_sysvar(cx, call.span);
				}
				continue;
			}

			if matches!(call.def_crate.as_deref(), Some("alloc" | "core" | "std")) {
				continue;
			}

			let looks_like_sysvar_use = call.receiver.as_deref().is_some_and(is_sysvar_receiver)
				|| call.path.as_deref().is_some_and(|path| {
					let terminal = terminal_identifier(path).to_ascii_lowercase();
					matches!(
						terminal.as_str(),
						"load_current_index" | "load_instruction_at"
					) || KNOWN_SYSVAR_NAMES.contains(&terminal.as_str())
						|| terminal.ends_with("_sysvar")
						|| terminal.ends_with("_instructions")
				});

			if !looks_like_sysvar_use {
				continue;
			}

			let has_guard = call.receiver.as_deref().is_some_and(|receiver| {
				facts.calls[..index].iter().any(|prior| {
					prior.method == "assert_sysvar"
						&& prior.receiver.as_deref() == Some(receiver)
						&& prior
							.args
							.first()
							.and_then(Option::as_deref)
							.is_some_and(|arg| matches_sysvar_id(receiver, arg))
				})
			});
			if !has_guard {
				emit_unchecked_sysvar(cx, call.span);
			}
		}

		for field in &facts.field_accesses {
			let Some(trusted_type) = trusted_sysvar_receiver_type(&field.receiver_type_definitions)
			else {
				continue;
			};
			let Node::Expr(receiver) = cx.tcx.hir_node(field.receiver_hir_id) else {
				continue;
			};
			if !expression_has_checked_loader(
				cx,
				&facts,
				trusted_type,
				receiver,
				field.span,
				&mut HashSet::new(),
			) {
				emit_unchecked_sysvar(cx, field.span);
			}
		}

		for projection in &facts.pattern_projections {
			let Some(trusted_type) =
				trusted_sysvar_receiver_type(&projection.value_type_definitions)
			else {
				continue;
			};
			let Node::Expr(value) = cx.tcx.hir_node(projection.value_hir_id) else {
				continue;
			};
			if !expression_has_checked_loader(
				cx,
				&facts,
				trusted_type,
				value,
				projection.span,
				&mut HashSet::new(),
			) {
				emit_unchecked_sysvar(cx, projection.span);
			}
		}
	}
}
