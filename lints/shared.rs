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
	pub path: Option<String>,
}

#[derive(Debug, Default)]
pub struct FunctionFacts {
	pub calls: Vec<CallInfo>,
	pub has_match: bool,
	pub has_byte_string: bool,
}

pub fn collect_function_facts(body: &Body<'_>) -> FunctionFacts {
	let mut facts = FunctionFacts::default();
	collect_from_expr(body.value, &mut facts);
	facts
}

pub fn receiver_name(expr: &Expr<'_>) -> Option<String> {
	match &expr.kind {
		ExprKind::Field(_, ident) => Some(ident.name.as_str().to_string()),
		ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
			path.segments
				.last()
				.map(|seg| seg.ident.name.as_str().to_string())
		}
		ExprKind::MethodCall(_, receiver, ..) => receiver_name(receiver),
		ExprKind::Match(scrutinee, ..) => receiver_name(scrutinee),
		_ => None,
	}
}

pub fn has_prior_method_with_receiver_match(
	calls: &[CallInfo],
	_index: usize,
	methods: &[&str],
	_receiver: &Option<String>,
) -> bool {
	calls
		.iter()
		.any(|call| methods.contains(&call.method.as_str()))
}

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

fn collect_from_expr(expr: &Expr<'_>, facts: &mut FunctionFacts) {
	match &expr.kind {
		ExprKind::MethodCall(path_segment, receiver, args, _) => {
			collect_from_expr(receiver, facts);
			for arg in *args {
				collect_from_expr(arg, facts);
			}
			facts.calls.push(CallInfo {
				span: expr.span,
				method: path_segment.ident.name.as_str().to_string(),
				receiver: receiver_name(receiver),
				path: None,
			});
		}
		ExprKind::Call(callee, args) => {
			collect_from_expr(callee, facts);
			for arg in *args {
				collect_from_expr(arg, facts);
			}
			if let rustc_hir::ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = &callee.kind {
				let path_name = path
					.segments
					.iter()
					.map(|segment| segment.ident.name.as_str())
					.collect::<Vec<_>>()
					.join("::");
				let method = path
					.segments
					.last()
					.map(|segment| segment.ident.name.as_str().to_string())
					.unwrap_or_else(|| path_name.clone());
				facts.calls.push(CallInfo {
					span: expr.span,
					method,
					receiver: None,
					path: Some(path_name),
				});
			}
		}
		ExprKind::Block(block, _) => {
			for stmt in block.stmts {
				match &stmt.kind {
					rustc_hir::StmtKind::Let(local) => {
						if let Some(init) = local.init {
							collect_from_expr(init, facts);
						}
					}
					rustc_hir::StmtKind::Expr(e) | rustc_hir::StmtKind::Semi(e) => {
						collect_from_expr(e, facts);
					}
					_ => {}
				}
			}
			if let Some(expr) = block.expr {
				collect_from_expr(expr, facts);
			}
		}
		ExprKind::Match(scrutinee, arms, _) => {
			facts.has_match = true;
			collect_from_expr(scrutinee, facts);
			for arm in *arms {
				collect_from_expr(arm.body, facts);
			}
		}
		ExprKind::If(cond, then, else_opt) => {
			collect_from_expr(cond, facts);
			collect_from_expr(then, facts);
			if let Some(el) = else_opt {
				collect_from_expr(el, facts);
			}
		}
		ExprKind::Unary(_, e)
		| ExprKind::Cast(e, _)
		| ExprKind::DropTemps(e)
		| ExprKind::AddrOf(_, _, e)
		| ExprKind::Field(e, _) => {
			collect_from_expr(e, facts);
		}
		ExprKind::Binary(_, lhs, rhs) | ExprKind::Assign(lhs, rhs, _) => {
			collect_from_expr(lhs, facts);
			collect_from_expr(rhs, facts);
		}
		ExprKind::Tup(exprs) | ExprKind::Array(exprs) => {
			for e in *exprs {
				collect_from_expr(e, facts);
			}
		}
		ExprKind::Lit(lit) => {
			if matches!(lit.node, LitKind::ByteStr(..)) {
				facts.has_byte_string = true;
			}
		}
		_ => {}
	}
}
