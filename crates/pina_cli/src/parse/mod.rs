pub mod account_state;
pub mod accounts_struct;
pub mod discriminator;
pub mod doc_comments;
pub mod entrypoint;
pub mod error_enum;
pub mod instruction_data;
pub mod program_id;
pub mod seeds;
pub mod types;
pub mod validation;

use std::collections::HashMap;
use std::path::Path;

use heck::ToSnakeCase;

use crate::error::IdlError;
use crate::ir::AccountIr;
use crate::ir::DiscriminatorIr;
use crate::ir::InstructionAccountIr;
use crate::ir::InstructionIr;
use crate::ir::ProgramIr;

/// Parse a program crate directory and assemble a `ProgramIr`.
pub fn parse_program(
	program_path: &Path,
	name_override: Option<&str>,
) -> Result<ProgramIr, IdlError> {
	let cargo_toml = program_path.join("Cargo.toml");
	let cargo_contents =
		std::fs::read_to_string(&cargo_toml).map_err(|e| IdlError::io(&cargo_toml, e))?;

	let package_name =
		extract_package_name(&cargo_contents).unwrap_or_else(|| "unknown_program".to_owned());

	let src_path = program_path.join("src/lib.rs");
	let source = std::fs::read_to_string(&src_path).map_err(|e| IdlError::io(&src_path, e))?;
	let file = syn::parse_file(&source).map_err(|e| IdlError::parse(&src_path, &e))?;

	assemble_program_ir(&file, name_override.unwrap_or(&package_name))
}

/// Assemble a `ProgramIr` from a parsed syn `File`.
pub fn assemble_program_ir(file: &syn::File, program_name: &str) -> Result<ProgramIr, IdlError> {
	// Step 1: Extract all pieces.
	let public_key = program_id::extract_program_id(file).ok_or(IdlError::NoProgramId)?;
	let disc_enums = discriminator::extract_discriminator_enums(file);
	let account_structs = account_state::extract_account_structs(file);
	let instruction_structs = instruction_data::extract_instruction_structs(file);
	let ix_accounts_structs = accounts_struct::extract_accounts_structs(file);
	let errors = error_enum::extract_error_enums(file);
	let dispatch = entrypoint::extract_dispatch_map(file);
	let validation_props = validation::extract_validation_properties(file);
	let seed_constants = seeds::extract_seed_constants(file);
	let pdas_ir = seeds::extract_pda_from_seed_macros(file, &seed_constants);

	// Step 2: Build accounts IR.
	let accounts: Vec<AccountIr> = account_structs
		.iter()
		.map(|acct| {
			let disc_value =
				find_discriminator_value(&disc_enums, &acct.discriminator_enum, &acct.name);
			AccountIr {
				name: acct.name.clone(),
				fields: acct.fields.clone(),
				discriminator: disc_value,
				docs: acct.docs.clone(),
			}
		})
		.collect();

	// Step 3: Build instructions IR by connecting dispatch, accounts structs,
	// instruction data, and validation properties.
	let instructions: Vec<InstructionIr> = dispatch
		.iter()
		.filter_map(|entry| {
			// Find the instruction data struct for this variant.
			let ix_struct = instruction_structs
				.iter()
				.find(|ix| ix.variant == entry.variant)?;

			// Find the accounts struct.
			let accts_struct = ix_accounts_structs
				.iter()
				.find(|a| a.name == entry.accounts_struct)?;

			// Find validation properties for this accounts struct.
			let val_props = validation_props.get(&entry.accounts_struct);

			// Find discriminator value.
			let disc_value = find_discriminator_value_by_variant(
				&disc_enums,
				&ix_struct.discriminator_enum,
				&entry.variant,
			);

			// Build instruction accounts with merged validation properties.
			let instruction_accounts: Vec<InstructionAccountIr> = accts_struct
				.fields
				.iter()
				.map(|field| {
					let props = val_props
						.and_then(|m| m.get(&field.name))
						.cloned()
						.unwrap_or_default();

					// Check if this field is a PDA from the pdas we found.
					let pda_name = if props.is_pda {
						pdas_ir.iter().find(|_| true).map(|p| p.name.clone())
					} else {
						None
					};

					InstructionAccountIr {
						name: field.name.clone(),
						is_writable: props.is_writable,
						is_signer: props.is_signer,
						is_optional: false,
						default_value: props.default_value,
						is_pda: props.is_pda,
						pda_name,
						docs: field.docs.clone(),
					}
				})
				.collect();

			// Use the variant name (snake_case) as the instruction name.
			let instruction_name = entry.variant.to_snake_case();

			Some(InstructionIr {
				name: instruction_name,
				accounts: instruction_accounts,
				arguments: ix_struct.fields.clone(),
				discriminator: disc_value,
				docs: ix_struct.docs.clone(),
			})
		})
		.collect();

	Ok(ProgramIr {
		name: program_name.to_owned(),
		public_key,
		accounts,
		instructions,
		errors,
		pdas: pdas_ir,
	})
}

/// Find the discriminator value for an account struct by matching the struct
/// name to a variant in the discriminator enum.
fn find_discriminator_value(
	disc_enums: &[discriminator::DiscriminatorEnum],
	enum_name: &str,
	struct_name: &str,
) -> DiscriminatorIr {
	for disc in disc_enums {
		if disc.name == enum_name {
			for variant in &disc.variants {
				if variant.name == struct_name {
					return DiscriminatorIr {
						value: variant.value,
						repr_size: disc.repr_size,
					};
				}
			}
		}
	}
	// Fallback
	DiscriminatorIr {
		value: 0,
		repr_size: 1,
	}
}

/// Find the discriminator value by variant name.
fn find_discriminator_value_by_variant(
	disc_enums: &[discriminator::DiscriminatorEnum],
	enum_name: &str,
	variant_name: &str,
) -> DiscriminatorIr {
	for disc in disc_enums {
		if disc.name == enum_name {
			for variant in &disc.variants {
				if variant.name == variant_name {
					return DiscriminatorIr {
						value: variant.value,
						repr_size: disc.repr_size,
					};
				}
			}
		}
	}
	DiscriminatorIr {
		value: 0,
		repr_size: 1,
	}
}

/// Simple Cargo.toml parser to extract `name = "..."` from `[package]`.
fn extract_package_name(cargo_contents: &str) -> Option<String> {
	let mut in_package = false;
	for line in cargo_contents.lines() {
		let trimmed = line.trim();
		if trimmed == "[package]" {
			in_package = true;
			continue;
		}
		if trimmed.starts_with('[') {
			in_package = false;
			continue;
		}
		if in_package {
			if let Some(rest) = trimmed.strip_prefix("name") {
				let rest = rest.trim().strip_prefix('=')?;
				let rest = rest.trim().trim_matches('"');
				return Some(rest.to_owned());
			}
		}
	}
	None
}

/// Build a lookup from discriminator enum name + variant name → (value,
/// `repr_size`).
pub fn build_discriminator_map(
	disc_enums: &[discriminator::DiscriminatorEnum],
) -> HashMap<(String, String), DiscriminatorIr> {
	let mut map = HashMap::new();
	for disc in disc_enums {
		for variant in &disc.variants {
			map.insert(
				(disc.name.clone(), variant.name.clone()),
				DiscriminatorIr {
					value: variant.value,
					repr_size: disc.repr_size,
				},
			);
		}
	}
	map
}
