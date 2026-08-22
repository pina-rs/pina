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
	///
	/// `None` means the instruction arm does not route through either supported
	/// account conversion: `StructName::try_from(accounts)` or the canonical
	/// `StructName::try_from((program_id, accounts))`. Such an arm has no
	/// extractable accounts metadata.
	pub accounts_struct: Option<String>,
}

/// Extract the instruction dispatch map from `process_instruction` functions.
///
/// Looks for the canonical pattern and the legacy account-only equivalent:
/// ```ignore
/// match instruction {
///     Enum::Variant => AccountsStruct::try_from((program_id, accounts))?.process(data),
///     Enum::Legacy => AccountsStruct::try_from(accounts)?.process(data),
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
						if let Item::Fn(f) = inner
							&& f.sig.ident == "process_instruction"
						{
							extract_from_fn_body(&f.block.stmts, &mut entries);
						}
					}
				}
			}
			_ => {}
		}
	}

	entries
}

/// Return whether a source file defines a top-level or inline-module
/// `process_instruction` function.
pub fn has_process_instruction(file: &File) -> bool {
	file.items.iter().any(|item| {
		matches!(item, Item::Fn(function) if function.sig.ident == "process_instruction")
			|| matches!(
				item,
				Item::Mod(module)
					if module.content.as_ref().is_some_and(|(_, items)| items.iter().any(|inner| {
						matches!(inner, Item::Fn(function) if function.sig.ident == "process_instruction")
					}))
			)
	})
}

fn extract_from_fn_body(stmts: &[Stmt], entries: &mut Vec<DispatchEntry>) {
	for stmt in stmts {
		let Stmt::Expr(expr, _) = stmt else {
			continue;
		};

		extract_from_expr(expr, entries);
	}
}

fn extract_from_expr(expr: &Expr, entries: &mut Vec<DispatchEntry>) {
	match expr {
		Expr::Match(m) => {
			for arm in &m.arms {
				entries.extend(parse_match_arm(arm));
			}
		}

		Expr::Block(b) => {
			for stmt in &b.block.stmts {
				let Stmt::Expr(expr, _) = stmt else {
					continue;
				};

				extract_from_expr(expr, entries);
			}
		}

		_ => {}
	}
}

/// Parse a match arm that uses either supported account conversion form.
fn parse_match_arm(arm: &syn::Arm) -> Vec<DispatchEntry> {
	let variants = extract_variant_names(&arm.pat);
	if variants.is_empty() {
		return Vec::new();
	}

	let accounts_struct = extract_accounts_struct_from_body(&arm.body);

	variants
		.into_iter()
		.map(|variant| {
			DispatchEntry {
				variant,
				accounts_struct: accounts_struct.clone(),
			}
		})
		.collect()
}

/// Extract one or more variant names from a pattern like `Enum::Variant` or an
/// or-pattern like `Enum::A | Enum::B`.
fn extract_variant_names(pat: &syn::Pat) -> Vec<String> {
	match pat {
		syn::Pat::Path(pp) => {
			pp.path
				.segments
				.last()
				.map(|s| vec![s.ident.to_string()])
				.unwrap_or_default()
		}
		syn::Pat::TupleStruct(ts) => {
			ts.path
				.segments
				.last()
				.map(|s| vec![s.ident.to_string()])
				.unwrap_or_default()
		}
		syn::Pat::Struct(ps) => {
			ps.path
				.segments
				.last()
				.map(|s| vec![s.ident.to_string()])
				.unwrap_or_default()
		}
		syn::Pat::Or(or_pat) => {
			or_pat
				.cases
				.iter()
				.flat_map(extract_variant_names)
				.collect()
		}
		_ => Vec::new(),
	}
}

/// Extract the accounts struct name from an expression that uses either
/// `StructName::try_from(accounts)` or the canonical
/// `StructName::try_from((program_id, accounts))` form.
fn extract_accounts_struct_from_body(expr: &Expr) -> Option<String> {
	match expr {
		Expr::MethodCall(mc) if mc.method == "process" => {
			extract_accounts_struct_from_body(&mc.receiver)
		}
		Expr::Try(t) => extract_accounts_struct_from_body(&t.expr),
		Expr::Call(call) => extract_accounts_struct_from_call(call),
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

fn extract_accounts_struct_from_call(call: &syn::ExprCall) -> Option<String> {
	let Expr::Path(path) = &*call.func else {
		return None;
	};

	if path.path.segments.len() != 2 {
		return None;
	}

	if path.path.segments.last()?.ident != "try_from" {
		return None;
	}

	// Accept both `Struct::try_from(accounts)` (legacy fixtures) and the
	// current `Struct::try_from((program_id, accounts))` shape.
	let args = call.args.iter().collect::<Vec<_>>();
	match args.as_slice() {
		[arg] if expr_is_ident(arg, "accounts") => {}
		[Expr::Tuple(tuple)]
			if tuple.elems.len() == 2
				&& expr_is_ident(&tuple.elems[0], "program_id")
				&& expr_is_ident(&tuple.elems[1], "accounts") => {}
		_ => return None,
	}

	Some(path.path.segments.first()?.ident.to_string())
}

fn expr_is_ident(expr: &Expr, ident: &str) -> bool {
	matches!(expr, Expr::Path(path) if path.path.is_ident(ident))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn detects_accountless_process_instruction() {
		let file = syn::parse_file(
			r#"
				mod entrypoint {
					fn process_instruction() { Ok(()) }
				}
			"#,
		)
		.unwrap_or_else(|error| panic!("parse failed: {error}"));

		assert!(has_process_instruction(&file));
	}

	#[test]
	fn extracts_dispatch_entries() {
		let source = r#"
			mod entrypoint {
				pub fn process_instruction(
					program_id: &Address,
					accounts: &mut [AccountView],
					data: &[u8],
				) -> ProgramResult {
					let instruction: CounterInstruction = parse_instruction(program_id, &ID, data)?;
					match instruction {
						CounterInstruction::Initialize => {
							InitializeAccounts::try_from((program_id, accounts))?.process(data)
						}
						CounterInstruction::Increment => {
							IncrementAccounts::try_from((program_id, accounts))?.process(data)
						}
					}
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let dispatch = extract_dispatch_map(&file);
		assert_eq!(dispatch.len(), 2);
		assert_eq!(dispatch[0].variant, "Initialize");
		assert_eq!(
			dispatch[0].accounts_struct,
			Some("InitializeAccounts".to_owned())
		);
		assert_eq!(dispatch[1].variant, "Increment");
		assert_eq!(
			dispatch[1].accounts_struct,
			Some("IncrementAccounts".to_owned())
		);
	}

	#[test]
	fn extracts_single_line_dispatch() {
		let source = r#"
			mod entrypoint {
				pub fn process_instruction(
					program_id: &Address,
					accounts: &mut [AccountView],
					data: &[u8],
				) -> ProgramResult {
					let instruction: HelloInstruction = parse_instruction(program_id, &ID, data)?;
					match instruction {
						HelloInstruction::Hello => HelloAccounts::try_from((program_id, accounts))?.process(data),
					}
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let dispatch = extract_dispatch_map(&file);
		assert_eq!(dispatch.len(), 1);
		assert_eq!(dispatch[0].variant, "Hello");
		assert_eq!(
			dispatch[0].accounts_struct,
			Some("HelloAccounts".to_owned())
		);
	}

	#[test]
	fn extracts_or_pattern_dispatch_entries() {
		let source = r#"
			mod entrypoint {
				pub fn process_instruction(
					program_id: &Address,
					accounts: &mut [AccountView],
					data: &[u8],
				) -> ProgramResult {
					let instruction: TodoInstruction = parse_instruction(program_id, &ID, data)?;
					match instruction {
						TodoInstruction::Initialize => {
							InitializeAccounts::try_from((program_id, accounts))?.process(data)
						}
						TodoInstruction::ToggleCompleted | TodoInstruction::UpdateDigest => {
							UpdateAccounts::try_from((program_id, accounts))?.process(data)
						}
					}
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let dispatch = extract_dispatch_map(&file);
		assert_eq!(dispatch.len(), 3);
		assert_eq!(dispatch[0].variant, "Initialize");
		assert_eq!(dispatch[1].variant, "ToggleCompleted");
		assert_eq!(dispatch[2].variant, "UpdateDigest");
		assert!(dispatch.iter().all(|entry| {
			matches!(
				entry.accounts_struct.as_deref(),
				Some("InitializeAccounts") | Some("UpdateAccounts")
			)
		}));
	}

	#[test]
	fn extracts_accountless_dispatch_entries() {
		let source = r#"
			mod entrypoint {
				pub fn process_instruction(
					program_id: &Address,
					accounts: &mut [AccountView],
					data: &[u8],
				) -> ProgramResult {
					let instruction: DuplicateMutableInstruction = parse_instruction(program_id, &ID, data)?;
					match instruction {
						DuplicateMutableInstruction::AllowsDuplicateMutable => {
							let _ = AllowsDuplicateMutableInstruction::try_from_bytes(data)?;
							Ok(())
						}
						DuplicateMutableInstruction::AllowsDuplicateReadonly => {
							DuplicateReadonlyAccounts::try_from((program_id, accounts))?.process(data)
						}
					}
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let dispatch = extract_dispatch_map(&file);
		assert_eq!(dispatch.len(), 2);
		assert_eq!(dispatch[0].variant, "AllowsDuplicateMutable");
		assert_eq!(dispatch[0].accounts_struct, None);
		assert_eq!(
			dispatch[1].accounts_struct,
			Some("DuplicateReadonlyAccounts".to_owned())
		);
	}

	#[test]
	fn ignores_non_try_from_associated_calls() {
		let source = r#"
			mod entrypoint {
				pub fn process_instruction(
					program_id: &Address,
					accounts: &mut [AccountView],
					data: &[u8],
				) -> ProgramResult {
					let instruction: ExampleInstruction = parse_instruction(program_id, &ID, data)?;
					match instruction {
						ExampleInstruction::ParseOnly => Payload::parse(data),
						ExampleInstruction::HelperProcess => Handler::helper(data)?.process(data),
					}
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let dispatch = extract_dispatch_map(&file);
		assert_eq!(dispatch.len(), 2);
		assert_eq!(dispatch[0].variant, "ParseOnly");
		assert_eq!(dispatch[0].accounts_struct, None);
		assert_eq!(dispatch[1].variant, "HelperProcess");
		assert_eq!(dispatch[1].accounts_struct, None);
	}

	#[test]
	fn rejects_misordered_or_unrelated_try_from_tuples() {
		for expression in [
			syn::parse_quote!(Accounts::try_from((accounts, program_id))),
			syn::parse_quote!(Accounts::try_from((other, accounts))),
			syn::parse_quote!(Accounts::try_from((program_id, other))),
		] {
			assert_eq!(extract_accounts_struct_from_call(&expression), None);
		}

		let valid = syn::parse_quote!(Accounts::try_from((program_id, accounts)));
		assert_eq!(
			extract_accounts_struct_from_call(&valid),
			Some("Accounts".to_owned())
		);
	}
}
