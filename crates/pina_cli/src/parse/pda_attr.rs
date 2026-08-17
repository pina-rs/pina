//! Extract PDA declarations from `#[pda(...)]` attributes on account structs.
//!
//! The attribute declares the typed PDA seeds for an `#[account]` struct:
//!
//! ```ignore
//! #[account(discriminator = CounterAccount)]
//! #[pda(seeds = [COUNTER_SEED, authority: Address], bump = bump)]
//! pub struct CounterState { ... }
//! ```
//!
//! Unlike the `macro_rules!` heuristic in [`super::seeds`], this extraction
//! reads the seed types directly from the attribute, so the generated IDL
//! always matches the on-chain derivation. Constant seed references (e.g.
//! `COUNTER_SEED`) are resolved through the file's `const &[u8]`
//! declarations.

use heck::ToSnakeCase;
use syn::File;
use syn::Item;
use syn::parse::Parse;

use super::seeds::SeedConstant;
use crate::ir::PdaIr;
use crate::ir::PdaSeedIr;
/// Extract PDA declarations from `#[pda(...)]` attributes on account structs.
///
/// Constant seed references (e.g. `COUNTER_SEED`) are resolved through the
/// file's `const &[u8]` declarations. A PDA whose constant cannot be resolved
/// is skipped so IDL generation can still proceed for other accounts.
pub fn extract_pda_from_attributes(file: &File, seed_constants: &[SeedConstant]) -> Vec<PdaIr> {
	let mut pdas = Vec::new();

	for item in &file.items {
		let Item::Struct(item_struct) = item else {
			continue;
		};

		let Some(attr) = item_struct
			.attrs
			.iter()
			.find(|attr| attr.path().is_ident("pda"))
		else {
			continue;
		};

		let Ok(args) = attr.parse_args_with(PdaAttrArgs::parse) else {
			// Malformed attributes are rejected by the compiler; skip them
			// here so IDL generation can still proceed for other accounts.
			continue;
		};

		let name = pda_name_for_struct(&item_struct.ident.to_string());
		let mut seeds = Vec::with_capacity(args.seeds.len());
		let mut resolved = true;

		for seed in args.seeds {
			match seed {
				PdaAttrSeed::Constant(value) => seeds.push(PdaSeedIr::Constant { value }),
				PdaAttrSeed::ConstantRef(path) => {
					let Some(ident) = path.get_ident() else {
						resolved = false;
						break;
					};
					let Some(constant) =
						seed_constants.iter().find(|c| c.name == ident.to_string())
					else {
						resolved = false;
						break;
					};
					seeds.push(PdaSeedIr::Constant {
						value: constant.value.clone(),
					});
				}
				PdaAttrSeed::Variable { name, rust_type } => {
					seeds.push(PdaSeedIr::Variable { name, rust_type });
				}
			}
		}

		if resolved {
			pdas.push(PdaIr { name, seeds });
		}
	}

	pdas
}

/// The IDL name for a PDA declared on a struct.
///
/// The `State` suffix is stripped so `CounterState` produces the `counter`
/// PDA, matching the naming convention used by the example programs.
fn pda_name_for_struct(struct_name: &str) -> String {
	let stripped = struct_name.strip_suffix("State").unwrap_or(struct_name);
	if stripped.is_empty() {
		return struct_name.to_snake_case();
	}
	stripped.to_snake_case()
}

/// Parsed `#[pda(...)]` attribute arguments.
struct PdaAttrArgs {
	seeds: Vec<PdaAttrSeed>,
}

/// A single element of a `#[pda(seeds = [...])]` list.
enum PdaAttrSeed {
	Constant(Vec<u8>),
	ConstantRef(syn::Path),
	Variable { name: String, rust_type: String },
}

impl syn::parse::Parse for PdaAttrArgs {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		let mut seeds = None;

		while !input.is_empty() {
			let key: syn::Ident = input.call(syn::ext::IdentExt::parse_any)?;
			input.parse::<syn::Token![=]>()?;

			if key == "seeds" {
				if seeds.is_some() {
					return Err(syn::Error::new(key.span(), "duplicate `seeds` argument"));
				}
				seeds = Some(parse_seed_list(input)?);
			} else if key == "bump" {
				// The bump field name does not affect the IDL.
				let _: syn::Ident = input.call(syn::ext::IdentExt::parse_any)?;
			} else if key == "crate" {
				// The crate path does not affect the IDL.
				let _: syn::Path = input.parse()?;
			} else {
				return Err(syn::Error::new(
					key.span(),
					format!("unknown `#[pda]` argument `{key}`"),
				));
			}

			if input.is_empty() {
				break;
			}
			input.parse::<syn::Token![,]>()?;
		}

		let seeds = seeds
			.ok_or_else(|| syn::Error::new(input.span(), "`seeds` is required in `#[pda(...)]`"))?;

		Ok(Self { seeds })
	}
}

/// Parse a `[b"literal", name: Type, ...]` seed list.
///
/// The list is parsed from raw tokens because `name: Type` type ascriptions
/// are not valid Rust expressions.
fn parse_seed_list(input: syn::parse::ParseStream) -> syn::Result<Vec<PdaAttrSeed>> {
	let content;
	syn::bracketed!(content in input);

	let mut seeds = Vec::new();
	while !content.is_empty() {
		if content.peek(syn::LitByteStr) {
			let lit: syn::LitByteStr = content.parse()?;
			seeds.push(PdaAttrSeed::Constant(lit.value()));
		} else if content.peek2(syn::Token![:]) {
			let name: syn::Ident = content.parse()?;
			content.parse::<syn::Token![:]>()?;
			let ty: syn::Type = content.parse()?;
			let rust_type = seed_type_to_string(&ty);
			seeds.push(PdaAttrSeed::Variable {
				name: name.to_string(),
				rust_type,
			});
		} else {
			// A path to a `const &[u8]` seed (e.g. `COUNTER_SEED`).
			let path: syn::Path = content.parse()?;
			seeds.push(PdaAttrSeed::ConstantRef(path));
		}

		if content.is_empty() {
			break;
		}
		content.parse::<syn::Token![,]>()?;
	}

	Ok(seeds)
}

/// Convert a typed seed parameter type to its IDL Rust type name.
fn seed_type_to_string(ty: &syn::Type) -> String {
	match ty {
		syn::Type::Path(type_path) => {
			type_path
				.path
				.get_ident()
				.map_or_else(|| "Address".to_string(), |ident| ident.to_string())
		}
		syn::Type::Array(array) => {
			let len = match &array.len {
				syn::Expr::Lit(lit) => {
					match &lit.lit {
						syn::Lit::Int(int) => int.base10_parse::<usize>().unwrap_or(0),
						_ => 0,
					}
				}
				_ => 0,
			};
			format!("[u8; {len}]")
		}
		_ => "Address".to_string(),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn extracts_pda_with_address_seed() {
		let source = r#"
			const COUNTER_SEED: &[u8] = b"counter";

			#[account(discriminator = CounterAccount)]
			#[pda(seeds = [COUNTER_SEED, authority: Address], bump = bump)]
			pub struct CounterState {
				pub authority: Address,
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let constants = super::super::seeds::extract_seed_constants(&file);
		let pdas = extract_pda_from_attributes(&file, &constants);

		assert_eq!(pdas.len(), 1);
		assert_eq!(pdas[0].name, "counter");
		assert_eq!(pdas[0].seeds.len(), 2);
		assert!(matches!(
			&pdas[0].seeds[0],
			PdaSeedIr::Constant { value } if value == b"counter"
		));
		assert!(matches!(
			&pdas[0].seeds[1],
			PdaSeedIr::Variable { name, rust_type }
				if name == "authority" && rust_type == "Address"
		));
	}

	#[test]
	fn extracts_pda_with_numeric_seeds() {
		let source = r#"
			#[account(discriminator = EscrowAccount)]
			#[pda(seeds = [b"escrow", maker: Address, seed: u64], bump = bump)]
			pub struct EscrowState {
				pub maker: Address,
				pub seed: PodU64,
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let pdas = extract_pda_from_attributes(&file, &[]);

		assert_eq!(pdas.len(), 1);
		assert_eq!(pdas[0].name, "escrow");
		assert!(matches!(
			&pdas[0].seeds[2],
			PdaSeedIr::Variable { name, rust_type }
				if name == "seed" && rust_type == "u64"
		));
	}

	#[test]
	fn extracts_pda_with_byte_array_seed() {
		let source = r#"
			#[account(discriminator = PositionAccount)]
			#[pda(seeds = [b"position", owner: Address, tag: [u8; 8]], bump = bump)]
			pub struct PositionState {
				pub owner: Address,
				pub tag: [u8; 8],
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let pdas = extract_pda_from_attributes(&file, &[]);

		assert!(matches!(
			&pdas[0].seeds[2],
			PdaSeedIr::Variable { name, rust_type }
				if name == "tag" && rust_type == "[u8; 8]"
		));
	}

	#[test]
	fn extracts_multiple_pdas() {
		let source = r#"
			#[account(discriminator = RegistryAccount)]
			#[pda(seeds = [b"registry", admin: Address], bump = bump)]
			pub struct RegistryConfig {
				pub admin: Address,
				pub bump: u8,
			}

			#[account(discriminator = RoleEntryAccount)]
			#[pda(seeds = [b"role-entry", registry: Address, role_id: u64], bump = bump)]
			pub struct RoleEntry {
				pub registry: Address,
				pub role_id: PodU64,
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let pdas = extract_pda_from_attributes(&file, &[]);

		assert_eq!(pdas.len(), 2);
		assert_eq!(pdas[0].name, "registry_config");
		assert_eq!(pdas[1].name, "role_entry");
	}

	#[test]
	fn skips_pda_with_unresolved_constant_seed() {
		let source = r#"
			#[account(discriminator = CounterAccount)]
			#[pda(seeds = [MISSING_SEED, authority: Address], bump = bump)]
			pub struct CounterState {
				pub authority: Address,
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let pdas = extract_pda_from_attributes(&file, &[]);

		assert!(
			pdas.is_empty(),
			"unresolved constant seeds must skip the PDA"
		);
	}

	#[test]
	fn skips_structs_without_pda_attribute() {
		let source = r#"
			#[account(discriminator = PlainAccount)]
			pub struct PlainState {
				pub value: PodU64,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let pdas = extract_pda_from_attributes(&file, &[]);

		assert!(pdas.is_empty());
	}

	#[test]
	fn extracts_pda_with_unknown_seed_type() {
		// The compiler rejects unknown seed types; the CLI passes the type
		// string through so the IDL still reflects the declaration.
		let source = r#"
			#[account(discriminator = BrokenAccount)]
			#[pda(seeds = [b"broken", authority: NotAType], bump = bump)]
			pub struct BrokenState {
				pub authority: Address,
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let pdas = extract_pda_from_attributes(&file, &[]);

		assert_eq!(pdas.len(), 1);
		assert!(matches!(
			&pdas[0].seeds[1],
			PdaSeedIr::Variable { name, rust_type }
				if name == "authority" && rust_type == "NotAType"
		));
	}

	#[test]
	fn pda_name_strips_state_suffix() {
		assert_eq!(pda_name_for_struct("CounterState"), "counter");
		assert_eq!(pda_name_for_struct("EscrowState"), "escrow");
		assert_eq!(pda_name_for_struct("RegistryConfig"), "registry_config");
		assert_eq!(pda_name_for_struct("RoleEntry"), "role_entry");
		assert_eq!(pda_name_for_struct("State"), "state");
	}
}
