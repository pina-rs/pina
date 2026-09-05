//! Extract PDA declarations from `#[pda(...)]` attributes on account structs.
//!
//! The attribute declares the typed PDA seeds for an `#[account]` struct:
//!
//! ```ignore
//! #[account(discriminator = CounterAccount)]
//! #[pda(seeds = [SEED_COUNTER, authority: Address], bump = bump)]
//! pub struct CounterState { ... }
//! ```
//!
//! Unlike the `macro_rules!` heuristic in [`super::seeds`], this extraction
//! reads the seed types directly from the attribute, so the generated IDL
//! always matches the on-chain derivation. Constant seed references (e.g.
//! `SEED_COUNTER`) are resolved through the file's `const &[u8]`
//! declarations.

use heck::ToSnakeCase;
use syn::File;
use syn::Item;
use syn::parse::Parse;

use super::seeds::SeedConstant;
use crate::error::IdlError;
use crate::ir::PdaIr;
use crate::ir::PdaSeedIr;
/// Extract PDA declarations from `#[pda(...)]` attributes on account structs.
///
/// Constant seed references (e.g. `SEED_COUNTER`) are resolved through the
/// file's `const &[u8]` declarations.
///
/// # Errors
///
/// Returns an error when an attribute is malformed, a seed type does not match
/// the on-chain macro grammar, or a constant seed cannot be resolved uniquely.
pub fn extract_pda_from_attributes(
	file: &File,
	seed_constants: &[SeedConstant],
) -> Result<Vec<PdaIr>, IdlError> {
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

		let account = item_struct.ident.to_string();
		let args = attr
			.parse_args_with(PdaAttrArgs::parse)
			.map_err(|error| IdlError::invalid_pda(&account, error))?;

		let name = pda_name_for_struct(&item_struct.ident.to_string());
		let mut seeds = Vec::with_capacity(args.seeds.len());

		for seed in args.seeds {
			match seed {
				PdaAttrSeed::Constant(value) => seeds.push(PdaSeedIr::Constant { value }),
				PdaAttrSeed::ConstantRef(path) => {
					let ident = path
						.segments
						.last()
						.expect("syn paths always contain at least one segment")
						.ident
						.to_string();
					let mut matches = seed_constants
						.iter()
						.filter(|constant| constant.name == ident);
					let constant = matches.next().ok_or_else(|| {
						IdlError::invalid_pda(
							&account,
							format!(
								"constant seed `{}` could not be resolved",
								path_to_string(&path)
							),
						)
					})?;
					if matches.next().is_some() {
						return Err(IdlError::invalid_pda(
							&account,
							format!(
								"constant seed `{}` is ambiguous across source modules",
								path_to_string(&path)
							),
						));
					}
					seeds.push(PdaSeedIr::Constant {
						value: constant.value.clone(),
					});
				}
				PdaAttrSeed::Variable { name, rust_type } => {
					seeds.push(PdaSeedIr::Variable { name, rust_type });
				}
			}
		}

		pdas.push(PdaIr { name, seeds });
	}

	Ok(pdas)
}

fn path_to_string(path: &syn::Path) -> String {
	path.segments
		.iter()
		.map(|segment| segment.ident.to_string())
		.collect::<Vec<_>>()
		.join("::")
}

/// The IDL name for a PDA declared on a struct.
///
/// The `State` suffix is stripped so `CounterState` produces the `counter`
/// PDA, matching the naming convention used by the example programs.
pub(super) fn pda_name_for_struct(struct_name: &str) -> String {
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

impl Parse for PdaAttrArgs {
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
		} else if content.peek2(syn::Token![:]) && !content.peek3(syn::Token![:]) {
			let name: syn::Ident = content.parse()?;
			content.parse::<syn::Token![:]>()?;
			let ty: syn::Type = content.parse()?;
			let rust_type = seed_type_to_string(&ty)?;
			seeds.push(PdaAttrSeed::Variable {
				name: name.to_string(),
				rust_type,
			});
		} else {
			// A path to a `const &[u8]` seed (e.g. `SEED_COUNTER`).
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
fn seed_type_to_string(ty: &syn::Type) -> syn::Result<String> {
	let unsupported = || {
		syn::Error::new_spanned(
			ty,
			"unsupported seed type; expected `Address`, `u8`, `u16`, `u32`, `u64`, or `[u8; N]`",
		)
	};

	match ty {
		syn::Type::Path(type_path) => {
			let Some(ident) = type_path.path.get_ident() else {
				return Err(unsupported());
			};
			match ident.to_string().as_str() {
				"Address" | "u8" | "u16" | "u32" | "u64" => Ok(ident.to_string()),
				_ => Err(unsupported()),
			}
		}
		syn::Type::Array(array) => {
			let syn::Type::Path(element) = &*array.elem else {
				return Err(unsupported());
			};
			if !element.path.is_ident("u8") {
				return Err(unsupported());
			}
			let syn::Expr::Lit(literal) = &array.len else {
				return Err(unsupported());
			};
			let syn::Lit::Int(length) = &literal.lit else {
				return Err(unsupported());
			};
			let len = length.base10_parse::<usize>().map_err(|error| {
				syn::Error::new_spanned(ty, format!("invalid seed array length: {error}"))
			})?;
			if !(1..=32).contains(&len) {
				return Err(syn::Error::new_spanned(
					ty,
					"seed array length must be between 1 and 32",
				));
			}
			Ok(format!("[u8; {len}]"))
		}
		_ => Err(unsupported()),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn extracts_pda_with_address_seed() {
		let source = r#"
			const SEED_COUNTER: &[u8] = b"counter";

			#[account(discriminator = CounterAccount)]
			#[pda(seeds = [SEED_COUNTER, authority: Address], bump = bump)]
			pub struct CounterState {
				pub authority: Address,
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let constants = super::super::seeds::extract_seed_constants(&file);
		let pdas = extract_pda_from_attributes(&file, &constants)
			.unwrap_or_else(|error| panic!("extract failed: {error}"));

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
		let pdas = extract_pda_from_attributes(&file, &[])
			.unwrap_or_else(|error| panic!("extract failed: {error}"));

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
		let pdas = extract_pda_from_attributes(&file, &[])
			.unwrap_or_else(|error| panic!("extract failed: {error}"));

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
		let pdas = extract_pda_from_attributes(&file, &[])
			.unwrap_or_else(|error| panic!("extract failed: {error}"));

		assert_eq!(pdas.len(), 2);
		assert_eq!(pdas[0].name, "registry_config");
		assert_eq!(pdas[1].name, "role_entry");
	}

	#[test]
	fn rejects_pda_with_unresolved_constant_seed() {
		let source = r#"
			#[account(discriminator = CounterAccount)]
			#[pda(seeds = [MISSING_SEED, authority: Address], bump = bump)]
			pub struct CounterState {
				pub authority: Address,
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let error = extract_pda_from_attributes(&file, &[])
			.expect_err("unresolved constant seeds must fail");

		assert!(error.to_string().contains("MISSING_SEED"));
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
		let pdas = extract_pda_from_attributes(&file, &[])
			.unwrap_or_else(|error| panic!("extract failed: {error}"));

		assert!(pdas.is_empty());
	}

	#[test]
	fn rejects_pda_with_unknown_seed_type() {
		let source = r#"
			#[account(discriminator = BrokenAccount)]
			#[pda(seeds = [b"broken", authority: NotAType], bump = bump)]
			pub struct BrokenState {
				pub authority: Address,
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let error =
			extract_pda_from_attributes(&file, &[]).expect_err("unknown seed types must fail");

		assert!(error.to_string().contains("unsupported seed type"));
	}

	#[test]
	fn rejects_malformed_pda_attributes() {
		let source = r#"
			#[account(discriminator = BrokenAccount)]
			#[pda(seeds)]
			pub struct BrokenState {
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let error =
			extract_pda_from_attributes(&file, &[]).expect_err("malformed attributes must fail");

		assert!(error.to_string().contains("BrokenState"));
	}

	#[test]
	fn resolves_pda_with_multi_segment_constant_path() {
		let source = r#"
			const SEED: &[u8] = b"seed";
			#[account(discriminator = BrokenAccount)]
			#[pda(seeds = [crate::SEED, authority: Address], bump = bump)]
			pub struct BrokenState {
				pub authority: Address,
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let constants = super::super::seeds::extract_seed_constants(&file);
		let pdas = extract_pda_from_attributes(&file, &constants)
			.unwrap_or_else(|error| panic!("extract failed: {error}"));

		assert!(matches!(
			&pdas[0].seeds[0],
			PdaSeedIr::Constant { value } if value == b"seed"
		));
	}

	#[test]
	fn parses_bump_and_crate_arguments() {
		let source = r#"
			#[account(discriminator = CounterAccount)]
			#[pda(seeds = [b"counter", authority: Address], bump = bump, crate = ::pina)]
			pub struct CounterState {
				pub authority: Address,
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let pdas = extract_pda_from_attributes(&file, &[])
			.unwrap_or_else(|error| panic!("extract failed: {error}"));

		assert_eq!(pdas.len(), 1);
		assert_eq!(pdas[0].name, "counter");
	}

	#[test]
	fn rejects_pda_with_duplicate_seeds_argument() {
		let source = r#"
			#[account(discriminator = BrokenAccount)]
			#[pda(seeds = [b"broken"], seeds = [b"again"])]
			pub struct BrokenState {}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let error = extract_pda_from_attributes(&file, &[])
			.expect_err("duplicate `seeds` arguments must fail");

		assert!(error.to_string().contains("duplicate `seeds`"));
	}

	#[test]
	fn rejects_pda_with_unknown_argument() {
		let source = r#"
			#[account(discriminator = BrokenAccount)]
			#[pda(seeds = [b"broken"], unknown = 5)]
			pub struct BrokenState {}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let error =
			extract_pda_from_attributes(&file, &[]).expect_err("unknown arguments must fail");

		assert!(error.to_string().contains("unknown `#[pda]` argument"));
	}

	#[test]
	fn rejects_pda_with_const_length_byte_array_seed() {
		let source = r#"
			#[account(discriminator = PositionAccount)]
			#[pda(seeds = [b"position", owner: Address, tag: [u8; SIZE]], bump = bump)]
			pub struct PositionState {
				pub owner: Address,
				pub tag: [u8; 8],
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let error = extract_pda_from_attributes(&file, &[])
			.expect_err("non-literal array lengths must fail");

		assert!(error.to_string().contains("unsupported seed type"));
	}

	#[test]
	fn rejects_pda_with_char_literal_array_length() {
		let source = r#"
			#[account(discriminator = PositionAccount)]
			#[pda(seeds = [b"position", owner: Address, tag: [u8; 'a']], bump = bump)]
			pub struct PositionState {
				pub owner: Address,
				pub tag: [u8; 8],
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let error = extract_pda_from_attributes(&file, &[])
			.expect_err("non-integer array lengths must fail");

		assert!(error.to_string().contains("unsupported seed type"));
	}

	#[test]
	fn rejects_pda_with_reference_seed_type() {
		let source = r#"
			#[account(discriminator = PositionAccount)]
			#[pda(seeds = [b"position", owner: &[u8]], bump = bump)]
			pub struct PositionState {
				pub owner: Address,
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let error =
			extract_pda_from_attributes(&file, &[]).expect_err("reference seed types must fail");

		assert!(error.to_string().contains("unsupported seed type"));
	}

	#[test]
	fn rejects_ambiguous_constant_seed() {
		let source = r#"
			#[account(discriminator = CounterAccount)]
			#[pda(seeds = [SEED], bump = bump)]
			pub struct CounterState { pub bump: u8 }
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let constants = [
			SeedConstant {
				name: "SEED".to_owned(),
				value: b"first".to_vec(),
			},
			SeedConstant {
				name: "SEED".to_owned(),
				value: b"second".to_vec(),
			},
		];

		let error = extract_pda_from_attributes(&file, &constants)
			.expect_err("ambiguous constants must fail");
		assert!(error.to_string().contains("ambiguous"));
	}

	#[test]
	fn rejects_invalid_seed_array_element_and_length() {
		for seed_type in [
			"[u16; 8]",
			"[(u8, u8); 8]",
			"[u8; 0]",
			"[u8; 33]",
			"[u8; 999999999999999999999999999999999999999999]",
		] {
			let source = format!(
				r#"
					#[account(discriminator = PositionAccount)]
					#[pda(seeds = [tag: {seed_type}], bump = bump)]
					pub struct PositionState {{ pub bump: u8 }}
				"#,
			);
			let file = syn::parse_file(&source).unwrap_or_else(|e| panic!("parse failed: {e}"));

			assert!(
				extract_pda_from_attributes(&file, &[]).is_err(),
				"{seed_type} must be rejected"
			);
		}
	}

	#[test]
	fn rejects_multi_segment_seed_type() {
		let source = r#"
			#[account(discriminator = PositionAccount)]
			#[pda(seeds = [owner: pinocchio::Address], bump = bump)]
			pub struct PositionState { pub bump: u8 }
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let error = extract_pda_from_attributes(&file, &[])
			.expect_err("multi-segment seed types must fail");

		assert!(error.to_string().contains("unsupported seed type"));
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
