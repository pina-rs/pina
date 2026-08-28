#![allow(dead_code)]

extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_span;

use rustc_ast::LitKind;
use rustc_hir::Body;
use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_span::Span;

#[derive(Debug, Clone)]
pub struct CallInfo {
	pub span: Span,
	pub method: String,
	pub receiver: Option<String>,
	pub receiver_span: Option<Span>,
	pub path: Option<String>,
	pub is_type_relative: bool,
	pub args: Vec<Option<String>>,
	pub result_binding: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AssignmentInfo {
	pub span: Span,
	pub identity: String,
}

#[derive(Debug, Default)]
pub struct FunctionFacts {
	pub calls: Vec<CallInfo>,
	pub has_match: bool,
	pub has_byte_string: bool,
	pub paths: Vec<String>,
	pub assignments: Vec<AssignmentInfo>,
}

pub fn collect_function_facts(body: &Body<'_>) -> FunctionFacts {
	let mut facts = FunctionFacts::default();
	collect_from_expr(body.value, &mut facts, None);
	facts
}

pub fn receiver_name(expr: &Expr<'_>) -> Option<String> {
	expression_identity(expr)
}

pub fn expression_identity(expr: &Expr<'_>) -> Option<String> {
	match &expr.kind {
		ExprKind::Field(base, ident) => {
			expression_identity(base).map(|base| format!("{base}.{}", ident.name.as_str()))
		}
		ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
			Some(
				path.segments
					.iter()
					.map(|seg| seg.ident.name.as_str())
					.collect::<Vec<_>>()
					.join("::"),
			)
		}
		ExprKind::MethodCall(_, receiver, ..) => expression_identity(receiver),
		ExprKind::Match(scrutinee, ..) => expression_identity(scrutinee),
		ExprKind::Unary(_, expr)
		| ExprKind::Cast(expr, _)
		| ExprKind::DropTemps(expr)
		| ExprKind::AddrOf(_, _, expr)
		| ExprKind::Index(expr, ..) => expression_identity(expr),
		ExprKind::Block(block, _) => block.expr.and_then(expression_identity),
		ExprKind::Call(callee, args) => {
			args.first()
				.and_then(expression_identity)
				.or_else(|| expression_identity(callee))
		}
		_ => None,
	}
}

pub fn has_prior_method_with_receiver_match(
	calls: &[CallInfo],
	index: usize,
	methods: &[&str],
	receiver: &Option<String>,
) -> bool {
	calls[..index]
		.iter()
		.any(|call| methods.contains(&call.method.as_str()) && &call.receiver == receiver)
}

pub const CONTROL_FLOW_LIMITATION_HELP: &str = "heuristic limitation: this lint tracks lexical \
                                                call order and merges `if`/`match` branches, so \
                                                review branch-sensitive code manually";

pub fn should_skip_def_path(def_path: &str) -> bool {
	def_path.contains("tests")
		|| def_path.contains("benchmarks")
		|| def_path.contains("fuzz")
		|| def_path.contains("snapshots")
		|| def_path.starts_with("pina::")
		|| def_path.contains("pina_macros::")
}

pub fn def_path_matches(def_path: &str, needles: &[&str]) -> bool {
	needles.iter().any(|needle| def_path.contains(needle))
}

fn collect_from_block(
	block: &rustc_hir::Block<'_>,
	facts: &mut FunctionFacts,
	result_binding: Option<&str>,
) {
	for stmt in block.stmts {
		match &stmt.kind {
			rustc_hir::StmtKind::Let(local) => {
				if let Some(init) = local.init {
					let binding = match local.pat.kind {
						rustc_hir::PatKind::Binding(_, _, ident, _) => {
							Some(ident.name.as_str().to_string())
						}
						_ => None,
					};
					collect_from_expr(init, facts, binding.as_deref());
				}
			}
			rustc_hir::StmtKind::Expr(expr) | rustc_hir::StmtKind::Semi(expr) => {
				collect_from_expr(expr, facts, None);
			}
			_ => {}
		}
	}
	if let Some(expr) = block.expr {
		collect_from_expr(expr, facts, result_binding);
	}
}

fn collect_from_expr(expr: &Expr<'_>, facts: &mut FunctionFacts, result_binding: Option<&str>) {
	match &expr.kind {
		ExprKind::MethodCall(path_segment, receiver, args, _) => {
			collect_from_expr(receiver, facts, result_binding);
			for arg in *args {
				collect_from_expr(arg, facts, result_binding);
			}
			facts.calls.push(CallInfo {
				span: expr.span,
				method: path_segment.ident.name.as_str().to_string(),
				receiver: expression_identity(receiver),
				receiver_span: Some(receiver.span),
				path: None,
				is_type_relative: false,
				args: args.iter().map(expression_identity).collect(),
				result_binding: result_binding.map(str::to_string),
			});
		}
		ExprKind::Call(callee, args) => {
			collect_from_expr(callee, facts, None);
			for arg in *args {
				collect_from_expr(arg, facts, result_binding);
			}
			if let rustc_hir::ExprKind::Path(path) = &callee.kind {
				let (path_name, is_type_relative) = match path {
					rustc_hir::QPath::Resolved(_, path) => {
						let path = path
							.segments
							.iter()
							.map(|segment| segment.ident.name.as_str())
							.collect::<Vec<_>>()
							.join("::");
						(path, false)
					}
					rustc_hir::QPath::TypeRelative(_, segment) => {
						(segment.ident.name.as_str().to_string(), true)
					}
				};
				let method = path_name
					.rsplit("::")
					.next()
					.unwrap_or(&path_name)
					.to_string();
				facts.calls.push(CallInfo {
					span: expr.span,
					method,
					receiver: None,
					receiver_span: None,
					path: Some(path_name),
					is_type_relative,
					args: args.iter().map(expression_identity).collect(),
					result_binding: result_binding.map(str::to_string),
				});
			}
		}
		ExprKind::Block(block, _) | ExprKind::Loop(block, ..) => {
			collect_from_block(block, facts, result_binding);
		}
		ExprKind::Match(scrutinee, arms, _) => {
			facts.has_match = true;
			collect_from_expr(scrutinee, facts, result_binding);
			for arm in *arms {
				collect_from_expr(arm.body, facts, result_binding);
			}
		}
		ExprKind::If(cond, then, else_opt) => {
			collect_from_expr(cond, facts, None);
			collect_from_expr(then, facts, result_binding);
			if let Some(el) = else_opt {
				collect_from_expr(el, facts, result_binding);
			}
		}
		ExprKind::Unary(_, expr)
		| ExprKind::Use(expr, _)
		| ExprKind::Cast(expr, _)
		| ExprKind::Type(expr, _)
		| ExprKind::DropTemps(expr)
		| ExprKind::AddrOf(_, _, expr)
		| ExprKind::Field(expr, _)
		| ExprKind::Repeat(expr, _)
		| ExprKind::Yield(expr, _)
		| ExprKind::Become(expr)
		| ExprKind::UnsafeBinderCast(_, expr, _) => {
			collect_from_expr(expr, facts, result_binding);
		}
		ExprKind::Binary(_, lhs, rhs) => {
			collect_from_expr(lhs, facts, result_binding);
			collect_from_expr(rhs, facts, result_binding);
		}
		ExprKind::Assign(lhs, rhs, _) | ExprKind::AssignOp(_, lhs, rhs) => {
			if let Some(identity) = expression_identity(lhs) {
				facts.assignments.push(AssignmentInfo {
					span: expr.span,
					identity,
				});
			}
			collect_from_expr(lhs, facts, None);
			collect_from_expr(rhs, facts, None);
		}
		ExprKind::Index(base, index, _) => {
			collect_from_expr(base, facts, result_binding);
			collect_from_expr(index, facts, None);
		}
		ExprKind::Let(let_expr) => {
			collect_from_expr(let_expr.init, facts, result_binding);
		}
		ExprKind::Tup(exprs) | ExprKind::Array(exprs) => {
			for e in *exprs {
				collect_from_expr(e, facts, result_binding);
			}
		}
		ExprKind::Struct(_, fields, tail) => {
			for field in *fields {
				collect_from_expr(field.expr, facts, result_binding);
			}
			if let rustc_hir::StructTailExpr::Base(base) = tail {
				collect_from_expr(base, facts, result_binding);
			}
		}
		ExprKind::Ret(Some(inner)) | ExprKind::Break(_, Some(inner)) => {
			collect_from_expr(inner, facts, result_binding);
		}
		ExprKind::Lit(lit) => {
			if matches!(lit.node, LitKind::ByteStr(..)) {
				facts.has_byte_string = true;
			}
		}
		ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
			facts.paths.push(
				path.segments
					.iter()
					.map(|segment| segment.ident.name.as_str())
					.collect::<Vec<_>>()
					.join("::"),
			);
		}
		ExprKind::Path(rustc_hir::QPath::TypeRelative(_, segment)) => {
			facts.paths.push(segment.ident.name.as_str().to_string());
		}
		_ => {}
	}
}
