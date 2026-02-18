use std::collections::HashMap;

use syn::Expr;
use syn::ImplItem;
use syn::Item;
use syn::Stmt;

use crate::ir::DefaultValueIr;

/// Known program addresses used for default value resolution.
const KNOWN_ADDRESSES: &[(&str, &str)] = &[
	("system::ID", "11111111111111111111111111111111"),
	("token::ID", "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
	(
		"token_2022::ID",
		"TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
	),
	(
		"associated_token_account::ID",
		"ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",
	),
];

/// Properties inferred from validation chain analysis for a single account
/// field.
#[derive(Debug, Clone, Default)]
pub struct AccountProperties {
	pub is_signer: bool,
	pub is_writable: bool,
	pub is_pda: bool,
	pub default_value: Option<DefaultValueIr>,
}

/// Analyse all `impl ProcessAccountInfos for X` blocks in a file and return a
/// map from the struct name (without lifetime) to a map of field name ->
/// properties.
pub fn extract_validation_properties(
	file: &syn::File,
) -> HashMap<String, HashMap<String, AccountProperties>> {
	let mut result = HashMap::new();

	for item in &file.items {
		let Item::Impl(item_impl) = item else {
			continue;
		};

		// Must be `impl ProcessAccountInfos for X`.
		let Some(trait_path) = item_impl.trait_.as_ref().map(|(_, path, _)| path) else {
			continue;
		};
		if !path_ends_with(trait_path, "ProcessAccountInfos") {
			continue;
		}

		// Get the implementing struct name.
		let struct_name = type_to_name(&item_impl.self_ty);

		// Find the `process` method.
		let Some(process_fn) = find_process_method(&item_impl.items) else {
			continue;
		};

		let props = analyse_process_body(&process_fn.block.stmts);
		result.insert(struct_name, props);
	}

	result
}

/// Walk the statements in a `process()` body and collect assertions per field.
fn analyse_process_body(stmts: &[Stmt]) -> HashMap<String, AccountProperties> {
	let mut props: HashMap<String, AccountProperties> = HashMap::new();

	for stmt in stmts {
		collect_assertions_from_stmt(stmt, &mut props);
	}

	props
}

fn collect_assertions_from_stmt(stmt: &Stmt, props: &mut HashMap<String, AccountProperties>) {
	match stmt {
		Stmt::Expr(expr, _) => {
			collect_assertions_from_expr(expr, props);
		}
		Stmt::Local(syn::Local {
			init: Some(init), ..
		}) => {
			collect_assertions_from_expr(&init.expr, props);
			if let Some((_, diverge)) = &init.diverge {
				collect_assertions_from_expr(diverge, props);
			}
		}
		Stmt::Item(Item::Impl(item_impl)) => {
			// Nested impl blocks (unlikely but handle gracefully).
			for ii in &item_impl.items {
				if let ImplItem::Fn(f) = ii {
					for s in &f.block.stmts {
						collect_assertions_from_stmt(s, props);
					}
				}
			}
		}
		_ => {}
	}
}

fn collect_assertions_from_expr(expr: &Expr, props: &mut HashMap<String, AccountProperties>) {
	match expr {
		Expr::MethodCall(mc) => {
			let method = mc.method.to_string();
			if let Some(field_name) = resolve_self_field(&mc.receiver) {
				let entry = props.entry(field_name).or_default();
				apply_assertion(&method, &mc.args, entry);
			}
			// Also recurse into the receiver (for chained calls).
			collect_assertions_from_expr(&mc.receiver, props);
			// And recurse into arguments.
			for arg in &mc.args {
				collect_assertions_from_expr(arg, props);
			}
		}
		Expr::Try(t) => {
			collect_assertions_from_expr(&t.expr, props);
		}
		Expr::Block(b) => {
			for stmt in &b.block.stmts {
				collect_assertions_from_stmt(stmt, props);
			}
		}
		Expr::If(if_expr) => {
			collect_assertions_from_expr(&if_expr.cond, props);
			for stmt in &if_expr.then_branch.stmts {
				collect_assertions_from_stmt(stmt, props);
			}
			if let Some((_, else_expr)) = &if_expr.else_branch {
				collect_assertions_from_expr(else_expr, props);
			}
		}
		Expr::Let(let_expr) => {
			collect_assertions_from_expr(&let_expr.expr, props);
		}
		Expr::Call(call) => {
			collect_assertions_from_expr(&call.func, props);
			for arg in &call.args {
				collect_assertions_from_expr(arg, props);
			}
		}
		Expr::Paren(p) => {
			collect_assertions_from_expr(&p.expr, props);
		}
		Expr::Reference(r) => {
			collect_assertions_from_expr(&r.expr, props);
		}
		_ => {}
	}
}

/// Walk through chained method calls and `?` to find the originating
/// `self.<field>`.
fn resolve_self_field(expr: &Expr) -> Option<String> {
	match expr {
		Expr::Field(f) => {
			if is_self(&f.base) {
				Some(member_to_string(&f.member))
			} else {
				None
			}
		}
		Expr::Try(t) => resolve_self_field(&t.expr),
		Expr::MethodCall(mc) => resolve_self_field(&mc.receiver),
		Expr::Paren(p) => resolve_self_field(&p.expr),
		_ => None,
	}
}

fn is_self(expr: &Expr) -> bool {
	matches!(expr, Expr::Path(p) if p.path.is_ident("self"))
}

fn member_to_string(member: &syn::Member) -> String {
	match member {
		syn::Member::Named(ident) => ident.to_string(),
		syn::Member::Unnamed(idx) => idx.index.to_string(),
	}
}

/// Record the effect of a recognized assertion method.
fn apply_assertion(
	method: &str,
	args: &syn::punctuated::Punctuated<Expr, syn::Token![,]>,
	props: &mut AccountProperties,
) {
	match method {
		"assert_signer" => props.is_signer = true,
		"assert_writable" => props.is_writable = true,
		"assert_seeds" | "assert_seeds_with_bump" | "assert_canonical_bump" => {
			props.is_pda = true;
		}
		"assert_address" => {
			if let Some(addr) = first_arg_to_known_address(args) {
				props.default_value = Some(DefaultValueIr::PublicKey(addr));
			}
		}
		// Other assertions don't map directly to IDL properties.
		_ => {}
	}
}

/// If the first argument to `assert_address` is a known program ID reference,
/// return its base58 address.
fn first_arg_to_known_address(
	args: &syn::punctuated::Punctuated<Expr, syn::Token![,]>,
) -> Option<String> {
	let first = args.first()?;
	let path_str = expr_to_path_string(first)?;
	for &(known_path, known_addr) in KNOWN_ADDRESSES {
		if path_str.contains(known_path) {
			return Some(known_addr.to_owned());
		}
	}
	None
}

fn expr_to_path_string(expr: &Expr) -> Option<String> {
	match expr {
		Expr::Reference(r) => expr_to_path_string(&r.expr),
		Expr::Path(p) => {
			Some(
				p.path
					.segments
					.iter()
					.map(|s| s.ident.to_string())
					.collect::<Vec<_>>()
					.join("::"),
			)
		}
		_ => None,
	}
}

fn find_process_method(items: &[ImplItem]) -> Option<&syn::ImplItemFn> {
	for item in items {
		if let ImplItem::Fn(f) = item {
			if f.sig.ident == "process" {
				return Some(f);
			}
		}
	}
	None
}

fn path_ends_with(path: &syn::Path, ident: &str) -> bool {
	path.segments.last().is_some_and(|seg| seg.ident == ident)
}

fn type_to_name(ty: &syn::Type) -> String {
	match ty {
		syn::Type::Path(p) => {
			p.path
				.segments
				.last()
				.map_or_else(|| "Unknown".to_owned(), |seg| seg.ident.to_string())
		}
		_ => "Unknown".to_owned(),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn extracts_signer_and_writable() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(&self, data: &[u8]) -> ProgramResult {
					self.authority.assert_signer()?;
					self.counter.assert_writable()?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];
		assert!(props["authority"].is_signer);
		assert!(!props["authority"].is_writable);
		assert!(props["counter"].is_writable);
		assert!(!props["counter"].is_signer);
	}

	#[test]
	fn extracts_chained_assertions() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(&self, data: &[u8]) -> ProgramResult {
					self.sender.assert_signer()?.assert_writable()?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];
		assert!(props["sender"].is_signer);
		assert!(props["sender"].is_writable);
	}

	#[test]
	fn extracts_pda() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(&self, data: &[u8]) -> ProgramResult {
					self.counter
						.assert_empty()?
						.assert_writable()?
						.assert_seeds_with_bump(seeds, &ID)?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];
		assert!(props["counter"].is_pda);
		assert!(props["counter"].is_writable);
	}

	#[test]
	fn extracts_known_address() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(&self, data: &[u8]) -> ProgramResult {
					self.system_program.assert_address(&system::ID)?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];
		assert!(matches!(
			&props["system_program"].default_value,
			Some(DefaultValueIr::PublicKey(addr)) if addr == "11111111111111111111111111111111"
		));
	}
}
