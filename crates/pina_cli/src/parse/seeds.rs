use syn::File;
use syn::Item;

use crate::ir::PdaIr;
use crate::ir::PdaSeedIr;

/// Extract PDA seed information from `const SEED: &[u8] = b"...";`
/// declarations and `macro_rules!` seed macros.
///
/// This is a best-effort heuristic: it finds byte-string constants that are
/// likely used as PDA seeds and associates them with field names from
/// `#[derive(Accounts)]` structs.
pub fn extract_seed_constants(file: &File) -> Vec<SeedConstant> {
	let mut result = Vec::new();

	for item in &file.items {
		if let Item::Const(c) = item {
			if let Some(value) = extract_byte_string_value(&c.expr) {
				result.push(SeedConstant {
					name: c.ident.to_string(),
					value,
				});
			}
		}
	}

	result
}

/// A constant byte-string seed found in the source.
#[derive(Debug, Clone)]
pub struct SeedConstant {
	pub name: String,
	pub value: Vec<u8>,
}

/// Extract seed macros to understand PDA derivation patterns.
///
/// Looks for macros like:
/// ```ignore
/// macro_rules! counter_seeds {
///     ($authority:expr) => {
///         &[COUNTER_SEED, $authority]
///     };
/// }
/// ```
///
/// Returns a list of `PdaIr` with the seeds. The heuristic works by:
/// 1. Finding `macro_rules!` items whose name ends with `_seeds` or starts with
///    `seeds_`.
/// 2. Parsing the first (non-bump) arm to identify constant refs and variable
///    captures.
pub fn extract_pda_from_seed_macros(file: &File, seed_constants: &[SeedConstant]) -> Vec<PdaIr> {
	let mut pdas = Vec::new();

	for item in &file.items {
		let Item::Macro(item_macro) = item else {
			continue;
		};
		let Some(ident) = &item_macro.ident else {
			continue;
		};
		let macro_name = ident.to_string();
		if !macro_name.contains("seeds") {
			continue;
		}

		// Derive a PDA name from the macro name by stripping `_seeds` or
		// `seeds_` prefix.
		let pda_name = macro_name
			.strip_suffix("_seeds")
			.or_else(|| macro_name.strip_prefix("seeds_"))
			.unwrap_or(&macro_name)
			.to_owned();

		let tokens_str = item_macro.mac.tokens.to_string();
		let seeds = parse_seed_macro_tokens(&tokens_str, seed_constants);

		if !seeds.is_empty() {
			pdas.push(PdaIr {
				name: pda_name,
				seeds,
			});
		}
	}

	pdas
}

/// Parse the token stream of a seed macro to extract seeds.
///
/// The heuristic looks for the first arm with the fewest macro params (the
/// non-bump version). Within that arm's body `&[...]`, each element is either:
/// - A known constant name → `PdaSeedIr::Constant`
/// - A `$variable:expr` capture → `PdaSeedIr::Variable`
fn parse_seed_macro_tokens(tokens: &str, seed_constants: &[SeedConstant]) -> Vec<PdaSeedIr> {
	// Find the first `& [ ... ]` or `&[ ... ]` in the macro body.
	// proc-macro2 tokenization may insert spaces between `&` and `[`.
	let start = tokens.find("& [").or_else(|| tokens.find("&["));
	let Some(start) = start else {
		return Vec::new();
	};
	// Skip past `& [` or `&[`.
	let skip = if tokens[start..].starts_with("& [") {
		3
	} else {
		2
	};
	let rest = &tokens[start + skip..];
	let Some(end) = find_matching_bracket(rest) else {
		return Vec::new();
	};
	let body = &rest[..end];

	let mut seeds = Vec::new();
	for element in body.split(',') {
		let element = element.trim();
		if element.is_empty() {
			continue;
		}

		// Skip bump-related elements (e.g. `&[$bump]` or `& [$ bump]`).
		if element.starts_with("&[")
			|| element.starts_with("& [")
			|| element.starts_with("&[$")
			|| element.starts_with("& [$")
		{
			continue;
		}

		// Check if this is a known constant.
		if let Some(constant) = seed_constants.iter().find(|c| element.contains(&c.name)) {
			seeds.push(PdaSeedIr::Constant {
				value: constant.value.clone(),
			});
		} else if element.starts_with('$') || element.starts_with("$ ") {
			// It's a macro variable — extract the name.
			// Tokenized form may be `$ authority` (with space).
			let var_name = element
				.trim_start_matches('$')
				.trim()
				.split(':')
				.next()
				.unwrap_or("unknown")
				.trim()
				.to_owned();
			// Default to Address/PublicKey type for variable seeds.
			seeds.push(PdaSeedIr::Variable {
				name: var_name,
				rust_type: "Address".to_owned(),
			});
		}
	}

	seeds
}

fn find_matching_bracket(s: &str) -> Option<usize> {
	let mut depth = 1;
	for (i, ch) in s.char_indices() {
		match ch {
			'[' => depth += 1,
			']' => {
				depth -= 1;
				if depth == 0 {
					return Some(i);
				}
			}
			_ => {}
		}
	}
	None
}

fn extract_byte_string_value(expr: &syn::Expr) -> Option<Vec<u8>> {
	match expr {
		syn::Expr::Lit(syn::ExprLit {
			lit: syn::Lit::ByteStr(bs),
			..
		}) => Some(bs.value()),
		syn::Expr::Reference(r) => extract_byte_string_value(&r.expr),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn extracts_seed_constants() {
		let source = r#"
			const COUNTER_SEED: &[u8] = b"counter";
			const OTHER: u8 = 42;
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let constants = extract_seed_constants(&file);
		assert_eq!(constants.len(), 1);
		assert_eq!(constants[0].name, "COUNTER_SEED");
		assert_eq!(constants[0].value, b"counter");
	}

	#[test]
	fn extracts_pda_from_macro() {
		let source = r#"
			const COUNTER_SEED: &[u8] = b"counter";

			macro_rules! counter_seeds {
				($authority:expr) => {
					&[COUNTER_SEED, $authority]
				};
				($authority:expr, $bump:expr) => {
					&[COUNTER_SEED, $authority, &[$bump]]
				};
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let constants = extract_seed_constants(&file);
		let pdas = extract_pda_from_seed_macros(&file, &constants);
		assert_eq!(pdas.len(), 1);
		assert_eq!(pdas[0].name, "counter");
		assert_eq!(pdas[0].seeds.len(), 2);
		assert!(matches!(&pdas[0].seeds[0], PdaSeedIr::Constant { value } if value == b"counter"));
		assert!(
			matches!(&pdas[0].seeds[1], PdaSeedIr::Variable { name, .. } if name == "authority")
		);
	}
}
