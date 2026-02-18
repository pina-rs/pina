use syn::Expr;
use syn::File;
use syn::Item;
use syn::Stmt;

/// A mapping from a discriminator variant to the accounts struct used for that
/// instruction.
#[derive(Debug, Clone)]
pub struct DispatchEntry {
	/// The discriminator enum variant name (e.g. `"Initialize"`).
	pub variant: String,
	/// The accounts struct name (e.g. `"InitializeAccounts"`).
	pub accounts_struct: String,
}

/// Extract the instruction dispatch map from `process_instruction` functions.
///
/// Looks for patterns like:
/// ```ignore
/// match instruction {
///     Enum::Variant => AccountsStruct::try_from(accounts)?.process(data),
/// }
/// ```
pub fn extract_dispatch_map(file: &File) -> Vec<DispatchEntry> {
	let mut entries = Vec::new();

	// Search in top-level items and also inside `mod entrypoint { ... }`.
	for item in &file.items {
		match item {
			Item::Fn(f) if f.sig.ident == "process_instruction" => {
				extract_from_fn_body(&f.block.stmts, &mut entries);
			}
			Item::Mod(m) => {
				if let Some((_, items)) = &m.content {
					for inner in items {
						if let Item::Fn(f) = inner {
							if f.sig.ident == "process_instruction" {
								extract_from_fn_body(&f.block.stmts, &mut entries);
							}
						}
					}
				}
			}
			_ => {}
		}
	}

	entries
}

fn extract_from_fn_body(stmts: &[Stmt], entries: &mut Vec<DispatchEntry>) {
	for stmt in stmts {
		if let Stmt::Expr(expr, _) = stmt {
			extract_from_expr(expr, entries);
		}
	}
}

fn extract_from_expr(expr: &Expr, entries: &mut Vec<DispatchEntry>) {
	match expr {
		Expr::Match(m) => {
			for arm in &m.arms {
				if let Some(entry) = parse_match_arm(arm) {
					entries.push(entry);
				}
			}
		}
		Expr::Block(b) => {
			for stmt in &b.block.stmts {
				if let Stmt::Expr(expr, _) = stmt {
					extract_from_expr(expr, entries);
				}
			}
		}
		_ => {}
	}
}

/// Parse a single match arm like:
/// `Enum::Variant => StructName::try_from(accounts)?.process(data)`
fn parse_match_arm(arm: &syn::Arm) -> Option<DispatchEntry> {
	let variant = extract_variant_name(&arm.pat)?;
	let accounts_struct = extract_accounts_struct_from_body(&arm.body)?;

	Some(DispatchEntry {
		variant,
		accounts_struct,
	})
}

/// Extract the variant name from a pattern like `Enum::Variant`.
fn extract_variant_name(pat: &syn::Pat) -> Option<String> {
	match pat {
		syn::Pat::Path(pp) => pp.path.segments.last().map(|s| s.ident.to_string()),
		syn::Pat::TupleStruct(ts) => ts.path.segments.last().map(|s| s.ident.to_string()),
		syn::Pat::Struct(ps) => ps.path.segments.last().map(|s| s.ident.to_string()),
		_ => None,
	}
}

/// Extract the accounts struct name from an expression like:
/// `StructName::try_from(accounts)?.process(data)`
fn extract_accounts_struct_from_body(expr: &Expr) -> Option<String> {
	match expr {
		// .process(data)
		Expr::MethodCall(mc) => extract_accounts_struct_from_body(&mc.receiver),
		// StructName::try_from(accounts)?
		Expr::Try(t) => extract_accounts_struct_from_body(&t.expr),
		// StructName::try_from(accounts)
		Expr::Call(call) => {
			if let Expr::Path(p) = &*call.func {
				// The first segment before `::try_from` is the struct name.
				if p.path.segments.len() >= 2 {
					return Some(p.path.segments[0].ident.to_string());
				}
			}
			None
		}
		// Blocks wrapping the expression
		Expr::Block(b) => {
			if let Some(Stmt::Expr(expr, _)) = b.block.stmts.last() {
				extract_accounts_struct_from_body(expr)
			} else {
				None
			}
		}
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn extracts_dispatch_entries() {
		let source = r#"
			mod entrypoint {
				pub fn process_instruction(
					program_id: &Address,
					accounts: &[AccountView],
					data: &[u8],
				) -> ProgramResult {
					let instruction: CounterInstruction = parse_instruction(program_id, &ID, data)?;
					match instruction {
						CounterInstruction::Initialize => {
							InitializeAccounts::try_from(accounts)?.process(data)
						}
						CounterInstruction::Increment => {
							IncrementAccounts::try_from(accounts)?.process(data)
						}
					}
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let dispatch = extract_dispatch_map(&file);
		assert_eq!(dispatch.len(), 2);
		assert_eq!(dispatch[0].variant, "Initialize");
		assert_eq!(dispatch[0].accounts_struct, "InitializeAccounts");
		assert_eq!(dispatch[1].variant, "Increment");
		assert_eq!(dispatch[1].accounts_struct, "IncrementAccounts");
	}

	#[test]
	fn extracts_single_line_dispatch() {
		let source = r#"
			mod entrypoint {
				pub fn process_instruction(
					program_id: &Address,
					accounts: &[AccountView],
					data: &[u8],
				) -> ProgramResult {
					let instruction: HelloInstruction = parse_instruction(program_id, &ID, data)?;
					match instruction {
						HelloInstruction::Hello => HelloAccounts::try_from(accounts)?.process(data),
					}
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let dispatch = extract_dispatch_map(&file);
		assert_eq!(dispatch.len(), 1);
		assert_eq!(dispatch[0].variant, "Hello");
		assert_eq!(dispatch[0].accounts_struct, "HelloAccounts");
	}
}
