pub mod account_state;
pub mod accounts_struct;
pub mod collision;
pub mod discriminator;
pub mod doc_comments;
pub mod entrypoint;
pub mod error_enum;
pub mod instruction_data;
pub mod module_resolver;
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
use crate::ir::ErrorIr;
use crate::ir::InstructionAccountIr;
use crate::ir::InstructionIr;
use crate::ir::PdaIr;
use crate::ir::ProgramIr;

/// Parse a program crate directory and assemble a `ProgramIr`.
///
/// Resolves all source files starting from `src/lib.rs`, following `mod`
/// declarations to discover additional files. All discovered files are
/// parsed and their contents merged for IDL extraction.
pub fn parse_program(
	program_path: &Path,
	name_override: Option<&str>,
) -> Result<ProgramIr, IdlError> {
	let cargo_toml = program_path.join("Cargo.toml");
	let cargo_contents =
		std::fs::read_to_string(&cargo_toml).map_err(|e| IdlError::io(&cargo_toml, e))?;

	let package_name =
		extract_package_name(&cargo_contents).unwrap_or_else(|| "unknown_program".to_owned());

	let src_dir = program_path.join("src");
	let lib_path = src_dir.join("lib.rs");

	let resolved_files = module_resolver::resolve_crate(&src_dir, &lib_path)?;
	let syn_files: Vec<&syn::File> = resolved_files.iter().map(|rf| &rf.file).collect();

	assemble_program_ir_multi(&syn_files, name_override.unwrap_or(&package_name))
}

/// Assemble a `ProgramIr` from multiple parsed syn `File`s.
///
/// Merges extractions from all files. The first file should be `lib.rs`
/// (containing `declare_id!` and the entrypoint dispatch).
pub fn assemble_program_ir_multi(
	files: &[&syn::File],
	program_name: &str,
) -> Result<ProgramIr, IdlError> {
	let mut all_disc_enums = Vec::new();
	let mut all_account_structs = Vec::new();
	let mut all_instruction_structs = Vec::new();
	let mut all_ix_accounts_structs = Vec::new();
	let mut all_errors = Vec::new();
	let mut all_seed_constants = Vec::new();
	let mut dispatch = Vec::new();
	let mut all_validation_props = HashMap::new();
	let mut public_key = None;
	let mut pdas_ir = Vec::new();

	for file in files {
		if public_key.is_none() {
			public_key = program_id::extract_program_id(file);
		}

		all_disc_enums.extend(discriminator::extract_discriminator_enums(file));
		all_account_structs.extend(account_state::extract_account_structs(file));
		all_instruction_structs.extend(instruction_data::extract_instruction_structs(file));
		all_ix_accounts_structs.extend(accounts_struct::extract_accounts_structs(file));
		all_errors.extend(error_enum::extract_error_enums(file));

		let file_dispatch = entrypoint::extract_dispatch_map(file);
		if !file_dispatch.is_empty() {
			dispatch = file_dispatch;
		}

		let file_validation_props = validation::extract_validation_properties(file);
		all_validation_props.extend(file_validation_props);

		let file_seed_constants = seeds::extract_seed_constants(file);
		let file_pdas = seeds::extract_pda_from_seed_macros(file, &file_seed_constants);

		all_seed_constants.extend(file_seed_constants);
		pdas_ir.extend(file_pdas);
	}

	let public_key = public_key.ok_or(IdlError::NoProgramId)?;

	assemble_from_extracted(
		program_name,
		public_key,
		&all_disc_enums,
		&all_account_structs,
		&all_instruction_structs,
		&all_ix_accounts_structs,
		&all_errors,
		&dispatch,
		&all_validation_props,
		&pdas_ir,
	)
}

/// Assemble a `ProgramIr` from a single parsed syn `File`.
pub fn assemble_program_ir(file: &syn::File, program_name: &str) -> Result<ProgramIr, IdlError> {
	assemble_program_ir_multi(&[file], program_name)
}

/// Internal assembly from pre-extracted components.
#[allow(clippy::too_many_arguments)]
fn assemble_from_extracted(
	program_name: &str,
	public_key: String,
	disc_enums: &[discriminator::DiscriminatorEnum],
	account_structs: &[account_state::AccountStruct],
	instruction_structs: &[instruction_data::InstructionStruct],
	ix_accounts_structs: &[accounts_struct::AccountsStruct],
	errors: &[ErrorIr],
	dispatch: &[entrypoint::DispatchEntry],
	validation_props: &HashMap<String, HashMap<String, validation::AccountProperties>>,
	pdas_ir: &[PdaIr],
) -> Result<ProgramIr, IdlError> {
	let discriminator_map = build_discriminator_map(disc_enums);

	// Step 2: Build accounts IR.
	let accounts: Vec<AccountIr> = account_structs
		.iter()
		.map(|acct| {
			resolve_discriminator_value(
				&discriminator_map,
				&acct.discriminator_enum,
				&acct.name,
				"account",
			)
			.map(|disc_value| {
				AccountIr {
					name: acct.name.clone(),
					fields: acct.fields.clone(),
					discriminator: disc_value,
					docs: acct.docs.clone(),
				}
			})
		})
		.collect::<Result<_, _>>()?;

	// Step 3: Build instructions IR by connecting dispatch, accounts structs,
	// instruction data, and validation properties.
	let instructions = if dispatch.is_empty() {
		build_accountless_instructions_from_structs(instruction_structs, &discriminator_map)?
	} else {
		build_instructions_from_dispatch(
			&discriminator_map,
			instruction_structs,
			ix_accounts_structs,
			dispatch,
			validation_props,
			pdas_ir,
		)?
	};

	let ir = ProgramIr {
		name: program_name.to_owned(),
		public_key,
		accounts,
		instructions,
		errors: errors.to_vec(),
		pdas: pdas_ir.to_vec(),
	};

	// Step 4: Validate the assembled IR for collisions and duplicates.
	validate_program_ir(&ir)?;

	Ok(ir)
}

/// Run static validation checks on a fully assembled [`ProgramIr`].
///
/// Currently checks:
/// - Discriminator collisions within accounts and within instructions.
/// - Duplicate input field names within instructions (account names vs
///   argument names).
///
/// Returns `Ok(())` when the IR is valid, or an [`IdlError`] describing the
/// first set of violations found.
pub fn validate_program_ir(ir: &ProgramIr) -> Result<(), IdlError> {
	let collisions = collision::find_discriminator_collisions(ir);

	if !collisions.is_empty() {
		let messages = collision::format_collision_errors(&collisions);

		return Err(IdlError::Other(format!(
			"Discriminator collisions detected:\n  {}",
			messages.join("\n  "),
		)));
	}

	let duplicates = collision::find_duplicate_input_fields(ir);

	if !duplicates.is_empty() {
		let messages = collision::format_duplicate_field_errors(&duplicates);

		return Err(IdlError::Other(format!(
			"Duplicate instruction input field names detected:\n  {}",
			messages.join("\n  "),
		)));
	}

	Ok(())
}

fn resolve_discriminator_value(
	discriminator_map: &HashMap<(String, String), DiscriminatorIr>,
	enum_name: &str,
	variant_name: &str,
	kind: &str,
) -> Result<DiscriminatorIr, IdlError> {
	discriminator_map
		.get(&(enum_name.to_owned(), variant_name.to_owned()))
		.cloned()
		.ok_or_else(|| {
			IdlError::Other(format!(
				"Could not resolve {kind} discriminator for variant `{variant_name}` of \
				 discriminator `{enum_name}`"
			))
		})
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
#[must_use]
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

fn build_accountless_instructions_from_structs(
	instruction_structs: &[instruction_data::InstructionStruct],
	discriminator_map: &HashMap<(String, String), DiscriminatorIr>,
) -> Result<Vec<InstructionIr>, IdlError> {
	instruction_structs
		.iter()
		.map(|ix_struct| {
			resolve_discriminator_value(
				discriminator_map,
				&ix_struct.discriminator_enum,
				&ix_struct.variant,
				"instruction",
			)
			.map(|discriminator| {
				InstructionIr {
					name: ix_struct.variant.to_snake_case(),
					accounts: Vec::new(),
					arguments: ix_struct.fields.clone(),
					discriminator,
					docs: ix_struct.docs.clone(),
				}
			})
		})
		.collect()
}

fn build_instructions_from_dispatch(
	discriminator_map: &HashMap<(String, String), DiscriminatorIr>,
	instruction_structs: &[instruction_data::InstructionStruct],
	ix_accounts_structs: &[accounts_struct::AccountsStruct],
	dispatch: &[entrypoint::DispatchEntry],
	validation_props: &HashMap<String, HashMap<String, validation::AccountProperties>>,
	pdas_ir: &[PdaIr],
) -> Result<Vec<InstructionIr>, IdlError> {
	let mut instructions = Vec::with_capacity(dispatch.len());

	for entry in dispatch {
		let ix_struct = instruction_structs
			.iter()
			.find(|ix| ix.variant == entry.variant)
			.ok_or_else(|| {
				IdlError::UnresolvedInstruction {
					discriminator: "unknown".to_owned(),
					variant: entry.variant.clone(),
				}
			})?;

		let instruction_accounts = if let Some(accounts_struct_name) = &entry.accounts_struct {
			let accts_struct = ix_accounts_structs
				.iter()
				.find(|a| a.name == *accounts_struct_name)
				.ok_or_else(|| {
					IdlError::UnresolvedAccounts {
						name: accounts_struct_name.clone(),
					}
				})?;

			let val_props = validation_props.get(accounts_struct_name);
			build_instruction_accounts(accts_struct, val_props, pdas_ir)
		} else {
			Vec::new()
		};

		let discriminator = resolve_discriminator_value(
			discriminator_map,
			&ix_struct.discriminator_enum,
			&entry.variant,
			"instruction",
		)?;

		instructions.push(InstructionIr {
			name: entry.variant.to_snake_case(),
			accounts: instruction_accounts,
			arguments: ix_struct.fields.clone(),
			discriminator,
			docs: ix_struct.docs.clone(),
		});
	}

	Ok(instructions)
}

fn build_instruction_accounts(
	accts_struct: &accounts_struct::AccountsStruct,
	val_props: Option<&HashMap<String, validation::AccountProperties>>,
	pdas_ir: &[PdaIr],
) -> Vec<InstructionAccountIr> {
	accts_struct
		.fields
		.iter()
		.map(|field| {
			let props = val_props
				.and_then(|m| m.get(&field.name))
				.cloned()
				.unwrap_or_default();

			let pda_name = if props.is_pda {
				infer_pda_name_for_field(&field.name, pdas_ir)
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
		.collect()
}

fn infer_pda_name_for_field(field_name: &str, pdas: &[PdaIr]) -> Option<String> {
	let candidates = [
		field_name.to_owned(),
		field_name.trim_end_matches("_account").to_owned(),
		field_name.trim_end_matches("_pda").to_owned(),
	];

	for candidate in candidates {
		if candidate.is_empty() {
			continue;
		}

		if let Some(pda) = pdas.iter().find(|p| p.name == candidate) {
			return Some(pda.name.clone());
		}
	}

	None
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ir::PdaSeedIr;

	#[test]
	fn infer_pda_name_for_field_matches_exact_name() {
		let pdas = vec![
			PdaIr {
				name: "counter".to_owned(),
				seeds: vec![PdaSeedIr::Constant {
					value: b"counter".to_vec(),
				}],
			},
			PdaIr {
				name: "vault".to_owned(),
				seeds: vec![PdaSeedIr::Constant {
					value: b"vault".to_vec(),
				}],
			},
		];

		assert_eq!(
			infer_pda_name_for_field("counter", &pdas),
			Some("counter".to_owned())
		);
		assert_eq!(
			infer_pda_name_for_field("vault_account", &pdas),
			Some("vault".to_owned())
		);
		assert_eq!(infer_pda_name_for_field("unknown", &pdas), None);
	}

	#[test]
	fn assemble_program_ir_falls_back_to_accountless_instruction_structs() {
		let source = r#"
			declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

			#[discriminator]
			pub enum EventsInstruction {
				Initialize = 0,
				TestEvent = 1,
			}

			#[instruction(discriminator = EventsInstruction, variant = Initialize)]
			pub struct InitializeInstruction {}

			#[instruction(discriminator = EventsInstruction, variant = TestEvent)]
			pub struct TestEventInstruction {}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let ir = assemble_program_ir(&file, "events").unwrap_or_else(|e| panic!("assemble: {e}"));

		assert_eq!(ir.instructions.len(), 2);
		assert_eq!(ir.instructions[0].name, "initialize");
		assert_eq!(ir.instructions[0].accounts.len(), 0);
		assert_eq!(ir.instructions[1].name, "test_event");
		assert_eq!(ir.instructions[1].accounts.len(), 0);
	}

	#[test]
	fn assemble_program_ir_keeps_accountless_dispatch_arms() {
		let source = r#"
			declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

			#[discriminator]
			pub enum DuplicateMutableInstruction {
				FailsDuplicateMutable = 0,
				AllowsDuplicateMutable = 1,
			}

			#[instruction(discriminator = DuplicateMutableInstruction, variant = FailsDuplicateMutable)]
			pub struct FailsDuplicateMutableInstruction {}

			#[instruction(discriminator = DuplicateMutableInstruction, variant = AllowsDuplicateMutable)]
			pub struct AllowsDuplicateMutableInstruction {}

			#[derive(Accounts, Debug)]
			pub struct DuplicateMutableAccounts<'a> {
				pub account1: &'a AccountView,
				pub account2: &'a AccountView,
			}

			impl<'a> ProcessAccountInfos<'a> for DuplicateMutableAccounts<'a> {
				fn process(&self, _data: &[u8]) -> ProgramResult {
					Ok(())
				}
			}

			pub mod entrypoint {
				use super::*;

				pub fn process_instruction(
					program_id: &Address,
					accounts: &[AccountView],
					data: &[u8],
				) -> ProgramResult {
					let instruction: DuplicateMutableInstruction = parse_instruction(program_id, &ID, data)?;

					match instruction {
						DuplicateMutableInstruction::FailsDuplicateMutable => {
							DuplicateMutableAccounts::try_from(accounts)?.process(data)
						}
						DuplicateMutableInstruction::AllowsDuplicateMutable => {
							let _ = AllowsDuplicateMutableInstruction::try_from_bytes(data)?;
							Ok(())
						}
					}
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let ir =
			assemble_program_ir(&file, "duplicate").unwrap_or_else(|e| panic!("assemble: {e}"));

		assert_eq!(ir.instructions.len(), 2);
		assert_eq!(ir.instructions[0].name, "fails_duplicate_mutable");
		assert_eq!(ir.instructions[0].accounts.len(), 2);
		assert_eq!(ir.instructions[1].name, "allows_duplicate_mutable");
		assert!(ir.instructions[1].accounts.is_empty());
	}

	#[test]
	fn assemble_program_ir_rejects_missing_instruction_discriminator_variant() {
		let source = r#"
			declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

			#[discriminator]
			pub enum ExampleInstruction {
				Initialize = 0,
			}

			#[instruction(discriminator = ExampleInstruction, variant = Missing)]
			pub struct MissingInstruction {}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let error = assemble_program_ir(&file, "example").unwrap_err();
		let message = error.to_string();

		assert!(message.contains("Could not resolve instruction discriminator"));
		assert!(message.contains("Missing"));
		assert!(message.contains("ExampleInstruction"));
	}

	#[test]
	fn assemble_program_ir_rejects_missing_account_discriminator_variant() {
		let source = r#"
			declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

			#[discriminator]
			pub enum ExampleAccount {
				Config = 0,
			}

			#[account(discriminator = ExampleAccount)]
			pub struct MissingState {
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let error = assemble_program_ir(&file, "example").unwrap_err();
		let message = error.to_string();

		assert!(message.contains("Could not resolve account discriminator"));
		assert!(message.contains("MissingState"));
		assert!(message.contains("ExampleAccount"));
	}
}
