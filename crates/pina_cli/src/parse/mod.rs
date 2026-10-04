pub mod account_state;
pub mod accounts_struct;
pub mod collision;
pub mod discriminator;
pub mod doc_comments;
pub mod entrypoint;
pub mod error_enum;
pub mod event_data;
pub mod instruction_data;
pub mod module_resolver;
pub mod pda_attr;
pub mod pod_enum;
pub mod program_id;
pub mod seeds;
pub mod types;
pub mod validation;

use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
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

/// Per-item migration opt-in declared on a schema attribute.
///
/// The declaration is resolved against the manifest's auto policy, so the
/// parser reports what the source said rather than the effective decision.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MigrationOptIn {
	/// A bare `migrations` token or `migrations = true`.
	Explicit,
	/// No `migrations` argument; the manifest auto policy decides.
	#[default]
	Unspecified,
	/// `migrations = false`; the auto policy does not apply.
	Disabled,
}

impl MigrationOptIn {
	/// Whether the declaration is migration-aware under an auto policy.
	#[must_use]
	pub fn is_enabled(self, auto: bool) -> bool {
		match self {
			Self::Explicit => true,
			Self::Disabled => false,
			Self::Unspecified => auto,
		}
	}

	/// Whether the declaration explicitly disables migrations.
	#[must_use]
	pub fn is_disabled(self) -> bool {
		matches!(self, Self::Disabled)
	}
}

/// Parse a program crate directory and assemble a `ProgramIr`.
///
/// Resolves all source files starting from `src/lib.rs`, following `mod`
/// declarations to discover additional files. All discovered files are
/// parsed and their contents merged for IDL extraction.
///
/// This applies no migration auto policy, so only declarations with an explicit
/// `migrations` token are marked migration-aware. Policy-aware callers (for
/// example [`crate::generate_idl`]) read the recorded policy from the manifest
/// and use [`parse_program_with_auto`].
pub fn parse_program(
	program_path: &Path,
	name_override: Option<&str>,
) -> Result<ProgramIr, IdlError> {
	parse_program_with_auto(
		program_path,
		name_override,
		&pina_abi::MigrationAuto::none(),
	)
}

/// [`parse_program`] with an explicit migration auto policy.
///
/// The IR marks contracts as migration-aware when their declaration opts in
/// explicitly or the manifest policy covers their kind.
pub fn parse_program_with_auto(
	program_path: &Path,
	name_override: Option<&str>,
	auto: &pina_abi::MigrationAuto,
) -> Result<ProgramIr, IdlError> {
	parse_program_with_sources(program_path, name_override, auto).map(|(program, _)| program)
}

/// Parse a program and retain the exact source snapshot used to construct it.
///
/// Migration discovery consumes the returned syntax trees for event contracts,
/// avoiding a second filesystem read that could observe a different source
/// revision from the assembled program IR.
pub(crate) fn parse_program_with_sources(
	program_path: &Path,
	name_override: Option<&str>,
	auto: &pina_abi::MigrationAuto,
) -> Result<(ProgramIr, Vec<module_resolver::ResolvedFile>), IdlError> {
	let cargo_toml = program_path.join("Cargo.toml");
	let cargo_contents =
		std::fs::read_to_string(&cargo_toml).map_err(|e| IdlError::io(&cargo_toml, e))?;

	let package_name = extract_package_name(&cargo_contents)
		.ok_or_else(|| IdlError::missing_package_name(&cargo_toml))?;

	let src_dir = program_path.join("src");
	let lib_path = src_dir.join("lib.rs");

	let resolved_files = module_resolver::resolve_crate(&src_dir, &lib_path)?;
	let syn_files: Vec<&syn::File> = resolved_files.iter().map(|rf| &rf.file).collect();
	let has_instruction_structs = syn_files.iter().try_fold(false, |found, file| {
		instruction_data::extract_instruction_structs(file)
			.map(|instructions| found || !instructions.is_empty())
	})?;
	let entrypoint_source_count = syn_files
		.iter()
		.filter(|file| {
			entrypoint::has_process_instruction(file) || entrypoint::has_dispatch_attribute(file)
		})
		.count();
	if entrypoint_source_count > 1 {
		return Err(IdlError::ambiguous_entrypoint(entrypoint_source_count));
	}
	if has_instruction_structs && entrypoint_source_count == 0 {
		return Err(IdlError::NoEntrypoint);
	}

	// `#[discriminator(entrypoint)]` generates the program's dispatch, so more
	// than one declared entrypoint enum would emit competing `process_instruction`
	// implementations, and none means the declared instructions are unreachable
	// from the chain.
	// Extraction cannot fail here: `assemble_program_ir_multi_with_auto` ran the
	// same extractor over the same files and surfaced any error already.
	let mut entrypoint_enums = Vec::new();
	for file in &syn_files {
		for enum_ in discriminator::extract_discriminator_enums(file)? {
			if enum_.entrypoint {
				entrypoint_enums.push(enum_.name);
			}
		}
	}
	if entrypoint_enums.len() > 1 {
		return Err(IdlError::ambiguous_discriminator_entrypoint(
			&entrypoint_enums.join(", "),
		));
	}

	let program = assemble_program_ir_multi_with_auto(
		&syn_files,
		name_override.unwrap_or(&package_name),
		auto,
	)?;

	Ok((program, resolved_files))
}

/// Assemble a `ProgramIr` from multiple parsed syn `File`s.
///
/// Merges extractions from all files. The first file should be `lib.rs`
/// (containing `declare_id!` and the entrypoint dispatch).
pub fn assemble_program_ir_multi(
	files: &[&syn::File],
	program_name: &str,
) -> Result<ProgramIr, IdlError> {
	assemble_program_ir_multi_with_auto(files, program_name, &pina_abi::MigrationAuto::none())
}

/// [`assemble_program_ir_multi`] with an explicit migration auto policy.
pub fn assemble_program_ir_multi_with_auto(
	files: &[&syn::File],
	program_name: &str,
	auto: &pina_abi::MigrationAuto,
) -> Result<ProgramIr, IdlError> {
	let mut all_disc_enums = Vec::new();
	let mut all_account_structs = Vec::new();
	let mut all_event_structs = Vec::new();
	let mut all_instruction_structs = Vec::new();
	let mut all_ix_accounts_structs = Vec::new();
	let mut all_errors = Vec::new();
	let mut dispatch = Vec::new();
	let mut dispatch_source_count = 0;
	let mut all_validation_facts = HashMap::new();
	let mut all_declared_validation_props = HashMap::new();
	let helpers = validation::HelperFunctions::collect(files);
	let mut all_pinapod_enums = Vec::new();
	let mut public_key = None;
	let mut pdas_ir = Vec::new();
	let all_seed_constants = files
		.iter()
		.flat_map(|file| seeds::extract_seed_constants(file))
		.collect::<Vec<_>>();

	for file in files {
		if public_key.is_none() {
			public_key = program_id::extract_program_id(file);
		}

		all_disc_enums.extend(discriminator::extract_discriminator_enums(file)?);
		all_account_structs.extend(account_state::extract_account_structs(file)?);
		all_event_structs.extend(event_data::extract_event_declarations(file)?);
		all_instruction_structs.extend(instruction_data::extract_instruction_structs(file)?);
		all_ix_accounts_structs.extend(
			accounts_struct::extract_accounts_structs(file)
				.map_err(|error| IdlError::Other(error.to_string()))?,
		);
		all_errors.extend(
			error_enum::extract_error_enums(file)
				.map_err(|error| IdlError::Other(error.to_string()))?,
		);
		all_pinapod_enums.extend(pod_enum::extract_pinapod_enums(file)?);

		// A hand-written match is authoritative when present. Otherwise the
		// `#[discriminator(entrypoint)]` annotation carries the same routing facts,
		// because the macro generates the match the extractor would have read.
		let file_dispatch = match entrypoint::extract_dispatch_map(file) {
			dispatch if !dispatch.is_empty() => dispatch,
			_ => entrypoint::extract_dispatch_from_attribute(file),
		};
		if !file_dispatch.is_empty() {
			dispatch_source_count += 1;
			if dispatch_source_count > 1 {
				return Err(IdlError::ambiguous_entrypoint(dispatch_source_count));
			}
			dispatch = file_dispatch;
		}

		all_validation_facts.extend(validation::extract_validation_facts(file, &helpers));
		let file_declared_validation_props =
			validation::extract_declared_validation_properties(file)
				.map_err(|error| IdlError::Other(error.to_string()))?;
		all_declared_validation_props.extend(file_declared_validation_props);

		let file_seed_constants = seeds::extract_seed_constants(file);
		let file_pdas = seeds::extract_pda_from_seed_macros(file, &file_seed_constants);
		let file_attr_pdas = pda_attr::extract_pda_from_attributes(file, &all_seed_constants)?;

		pdas_ir.extend(file_pdas);
		pdas_ir.extend(file_attr_pdas);
	}

	for (struct_name, fields) in all_declared_validation_props {
		let facts = all_validation_facts.entry(struct_name).or_default();
		for (field_name, declared) in fields {
			let property = &mut facts.entry(field_name).or_default().properties;
			property.is_signer |= declared.is_signer;
			property.is_writable |= declared.is_writable;
			property.is_pda |= declared.is_pda;
			if declared.default_value.is_some() {
				property.default_value = declared.default_value;
			}
		}
	}

	let public_key = public_key.ok_or(IdlError::NoProgramId)?;

	assemble_from_extracted(
		program_name,
		public_key,
		&all_disc_enums,
		&all_account_structs,
		&all_event_structs,
		&all_instruction_structs,
		&all_ix_accounts_structs,
		&all_errors,
		&all_pinapod_enums,
		&dispatch,
		&all_validation_facts,
		&pdas_ir,
		auto,
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
	event_structs: &[event_data::EventDeclaration],
	instruction_structs: &[instruction_data::InstructionStruct],
	ix_accounts_structs: &[accounts_struct::AccountsStruct],
	errors: &[ErrorIr],
	pinapod_enums: &[crate::ir::PinaPodEnumIr],
	dispatch: &[entrypoint::DispatchEntry],
	validation_facts: &HashMap<String, HashMap<String, validation::FieldFacts>>,
	pdas_ir: &[PdaIr],
	auto: &pina_abi::MigrationAuto,
) -> Result<ProgramIr, IdlError> {
	let discriminator_map = build_discriminator_map(disc_enums);

	// Step 2: Build accounts IR.
	let accounts: Vec<AccountIr> = account_structs
		.iter()
		.map(|acct| {
			resolve_discriminator_value(
				&discriminator_map,
				&acct.discriminator_enum,
				&acct.variant,
				"account",
			)
			.map(|disc_value| {
				let mut docs = acct.docs.clone();
				if acct
					.migrations
					.is_enabled(auto.contains(pina_abi::ContractKind::Account))
				{
					docs.push(crate::ir::MIGRATABLE_DOC_MARKER.to_owned());
				}
				debug_assert_eq!(
					acct.is_compact(),
					docs.iter()
						.any(|doc| doc == crate::ir::COMPACT_ACCOUNT_DOC_MARKER)
				);
				AccountIr {
					name: acct.name.clone(),
					fields: acct.fields.clone(),
					discriminator: disc_value,
					docs,
					pda_name: acct.pda_name.clone(),
				}
			})
		})
		.collect::<Result<_, _>>()?;

	// Step 2b: Build events IR.
	let events: Vec<crate::ir::EventIr> = event_structs
		.iter()
		.map(|event| {
			resolve_discriminator_value(
				&discriminator_map,
				&event.discriminator_enum,
				&event.variant,
				"event",
			)
			.map(|discriminator| {
				let mut docs = event.docs.clone();
				if event
					.migrations
					.is_enabled(auto.contains(pina_abi::ContractKind::Event))
				{
					docs.push(crate::ir::MIGRATABLE_DOC_MARKER.to_owned());
				}
				crate::ir::EventIr {
					name: event.name.clone(),
					discriminator,
					fields: event.fields.clone(),
					docs,
				}
			})
		})
		.collect::<Result<_, _>>()?;

	// Step 3: Build instructions IR by connecting dispatch, accounts structs,
	// instruction data, and validation properties.
	for account in &accounts {
		if let Some(pda_name) = &account.pda_name
			&& !pdas_ir.iter().any(|pda| pda.name == *pda_name)
		{
			return Err(IdlError::unresolved_pda(&account.name));
		}
	}

	let instructions = if dispatch.is_empty() {
		build_accountless_instructions_from_structs(instruction_structs, &discriminator_map, auto)?
	} else {
		build_instructions_from_dispatch(
			&discriminator_map,
			instruction_structs,
			ix_accounts_structs,
			dispatch,
			validation_facts,
			&PdaCatalog::new(pdas_ir, account_structs),
			auto,
		)?
	};

	let ir = ProgramIr {
		name: program_name.to_owned(),
		public_key,
		pinapod_enums: pinapod_enums.to_vec(),
		accounts,
		instructions,
		events,
		errors: errors.to_vec(),
		pdas: pdas_ir.to_vec(),
	};

	// Step 4: Validate the assembled IR for collisions and duplicates.
	validate_program_ir(&ir)?;

	Ok(ir)
}

/// The IR marker recording how an instruction takes part in ABI history.
///
/// Only an explicit `migrations` token envelopes an instruction. An auto
/// policy that covers instructions records the rest without an envelope, so
/// their snapshot gates wire-breaking changes while their bytes stay unchanged.
fn instruction_marker(
	migrations: MigrationOptIn,
	auto: &pina_abi::MigrationAuto,
) -> Option<String> {
	match migrations {
		MigrationOptIn::Explicit => Some(crate::ir::MIGRATABLE_DOC_MARKER.to_owned()),
		MigrationOptIn::Unspecified if auto.contains(pina_abi::ContractKind::Instruction) => {
			Some(crate::ir::RECORDED_DOC_MARKER.to_owned())
		}
		MigrationOptIn::Unspecified | MigrationOptIn::Disabled => None,
	}
}

fn build_accountless_instructions_from_structs(
	instruction_structs: &[instruction_data::InstructionStruct],
	discriminator_map: &HashMap<(String, String), DiscriminatorIr>,
	auto: &pina_abi::MigrationAuto,
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
				let mut docs = ix_struct.docs.clone();
				docs.extend(instruction_marker(ix_struct.migrations, auto));
				InstructionIr {
					name: ix_struct.variant.to_snake_case(),
					rust_name: ix_struct.name.clone(),
					accounts: Vec::new(),
					arguments: ix_struct.fields.clone(),
					discriminator,
					docs,
				}
			})
		})
		.collect()
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
	let mut pinapod_enum_names = HashSet::new();
	for pinapod_enum in &ir.pinapod_enums {
		if !pinapod_enum_names.insert(pinapod_enum.name.as_str()) {
			return Err(IdlError::Other(format!(
				"Duplicate PinaPod enum `{}` cannot be flattened into one Codama program",
				pinapod_enum.name
			)));
		}
	}

	let mut pda_names = HashSet::new();
	for pda in &ir.pdas {
		if !pda_names.insert(pda.name.as_str()) {
			return Err(IdlError::Other(format!(
				"Duplicate PDA definition `{}` cannot be flattened into one Codama program",
				pda.name
			)));
		}
	}

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
		if in_package
			&& let Some((key, value)) = trimmed.split_once('=')
			&& key.trim() == "name"
		{
			let value = value.trim();
			let name = value.strip_prefix('"')?.strip_suffix('"')?;
			return (!name.is_empty()).then(|| name.to_owned());
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

fn build_instructions_from_dispatch(
	discriminator_map: &HashMap<(String, String), DiscriminatorIr>,
	instruction_structs: &[instruction_data::InstructionStruct],
	ix_accounts_structs: &[accounts_struct::AccountsStruct],
	dispatch: &[entrypoint::DispatchEntry],
	validation_facts: &HashMap<String, HashMap<String, validation::FieldFacts>>,
	pdas: &PdaCatalog<'_>,
	auto: &pina_abi::MigrationAuto,
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

			let facts = validation_facts.get(accounts_struct_name);
			build_instruction_accounts(accts_struct, facts, pdas)?
		} else {
			Vec::new()
		};

		let discriminator = resolve_discriminator_value(
			discriminator_map,
			&ix_struct.discriminator_enum,
			&entry.variant,
			"instruction",
		)?;

		let mut docs = ix_struct.docs.clone();
		docs.extend(instruction_marker(ix_struct.migrations, auto));
		instructions.push(InstructionIr {
			name: entry.variant.to_snake_case(),
			rust_name: ix_struct.name.clone(),
			accounts: instruction_accounts,
			arguments: ix_struct.fields.clone(),
			discriminator,
			docs,
		});
	}

	Ok(instructions)
}

fn build_instruction_accounts(
	accts_struct: &accounts_struct::AccountsStruct,
	field_facts: Option<&HashMap<String, validation::FieldFacts>>,
	pdas: &PdaCatalog<'_>,
) -> Result<Vec<InstructionAccountIr>, IdlError> {
	accts_struct
		.fields
		.iter()
		.map(|field| {
			let facts = field_facts
				.and_then(|m| m.get(&field.name))
				.cloned()
				.unwrap_or_default();
			let slot_pda = pdas.slot_pda(&field.name, &facts)?;
			let properties = facts.properties;

			Ok(InstructionAccountIr {
				name: field.name.clone(),
				is_writable: field.is_mutable || properties.is_writable,
				is_signer: properties.is_signer,
				is_optional: field.is_optional,
				default_value: properties.default_value,
				is_pda: slot_pda.is_pda,
				pda_name: slot_pda.pda_name,
				constraints: field.constraints.clone(),
				docs: field.docs.clone(),
			})
		})
		.collect()
}

/// What the processor proves about one account slot's PDA.
#[derive(Debug, Default, PartialEq, Eq)]
struct SlotPda {
	/// The processor pins the slot's address: it derives or checks the address
	/// from the PDA's seeds, or loads a PDA account type with only constant
	/// seeds, which has a single address.
	is_pda: bool,
	/// The PDA the slot's account belongs to, also known when the processor
	/// loads a variable-seed PDA account type without checking its address.
	pda_name: Option<String>,
}

/// The PDAs a program declares, and the `#[pda]` account types behind them.
struct PdaCatalog<'a> {
	pdas: &'a [PdaIr],
	/// Account type name to the PDA its `#[pda]` attribute declares.
	account_pdas: HashMap<&'a str, &'a str>,
}

impl<'a> PdaCatalog<'a> {
	fn new(pdas: &'a [PdaIr], account_structs: &'a [account_state::AccountStruct]) -> Self {
		let account_pdas = account_structs
			.iter()
			.filter_map(|account| Some((account.name.as_str(), account.pda_name.as_deref()?)))
			.collect();

		Self { pdas, account_pdas }
	}

	/// The PDA an instruction account slot holds, as far as its processor
	/// proves it.
	///
	/// A validated slot is pinned to its PDA; the loaded account type names the
	/// PDA when it is unambiguous, and the field name names it otherwise. A
	/// typed load alone proves which PDA the account belongs to but not which
	/// seeds derived it, so it pins the address only when every seed is
	/// constant. Generated clients derive a default address only for a pinned
	/// slot: a variable seed that happens to share a name with another account
	/// in the instruction is not proof that the account supplies the seed.
	///
	/// # Errors
	///
	/// Returns an unresolved-PDA error when the processor validates the slot as
	/// a PDA but neither its types nor its name identify one.
	fn slot_pda(
		&self,
		field_name: &str,
		facts: &validation::FieldFacts,
	) -> Result<SlotPda, IdlError> {
		let typed_pda_name = self.typed_pda_name(&facts.account_types);

		if facts.properties.is_pda {
			let pda_name = typed_pda_name
				.or_else(|| infer_pda_name_for_field(field_name, self.pdas))
				.ok_or_else(|| IdlError::unresolved_pda(field_name))?;

			return Ok(SlotPda {
				is_pda: true,
				pda_name: Some(pda_name),
			});
		}

		let Some(pda_name) = typed_pda_name else {
			return Ok(SlotPda::default());
		};

		Ok(SlotPda {
			is_pda: self.has_single_address(&pda_name),
			pda_name: Some(pda_name),
		})
	}

	/// Whether every seed of the named PDA is a constant.
	fn has_single_address(&self, pda_name: &str) -> bool {
		self.pdas
			.iter()
			.find(|pda| pda.name == pda_name)
			.is_some_and(|pda| {
				pda.seeds
					.iter()
					.all(|seed| matches!(seed, crate::ir::PdaSeedIr::Constant { .. }))
			})
	}

	/// The single PDA declared by the account types a field is loaded as.
	///
	/// A field loaded as two account types with different PDAs is ambiguous, so
	/// it names no PDA.
	fn typed_pda_name(&self, account_types: &BTreeSet<String>) -> Option<String> {
		let pda_names = account_types
			.iter()
			.filter_map(|account_type| self.account_pdas.get(account_type.as_str()))
			.collect::<BTreeSet<_>>();
		let mut pda_names = pda_names.into_iter();

		match (pda_names.next(), pda_names.next()) {
			(Some(pda_name), None) => Some((*pda_name).to_owned()),
			_ => None,
		}
	}
}

fn infer_pda_name_for_field(field_name: &str, pdas: &[PdaIr]) -> Option<String> {
	let candidates = [
		field_name.to_owned(),
		field_name.trim_end_matches("_account").to_owned(),
		field_name.trim_end_matches("_pda").to_owned(),
		field_name.trim_end_matches("_state").to_owned(),
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
	use codama_nodes::NestedTypeNodeTrait;

	use super::*;
	use crate::ir::PdaSeedIr;

	/// Write a minimal crate whose `src/lib.rs` is `source`, for the
	/// program-level validation that needs a real file tree.
	fn write_crate(source: &str) -> tempfile::TempDir {
		let temp = tempfile::TempDir::new().expect("temp dir");
		std::fs::create_dir_all(temp.path().join("src")).expect("src dir");
		std::fs::write(
			temp.path().join("Cargo.toml"),
			"[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[lib]\nname \
			 = \"fixture\"\npath = \"src/lib.rs\"\n",
		)
		.expect("manifest");
		std::fs::write(temp.path().join("src/lib.rs"), source).expect("lib.rs");
		temp
	}

	#[test]
	fn two_entrypoint_enums_are_rejected_in_a_realistic_program() {
		// The same program parses with one entrypoint enum, so the failure above
		// is the second declaration and not the fixture's shape.
		let source = r#"
declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator(entrypoint)]
pub enum OnlyInstruction {
	Run = 0,
}

#[instruction(discriminator = OnlyInstruction::Run)]
pub struct RunInstruction {
	pub value: u64,
}

#[derive(Accounts)]
pub struct RunAccounts<'a> {
	pub authority: &'a AccountView,
}
"#;
		let temp = write_crate(source);
		let parsed = parse_program_with_auto(temp.path(), None, &pina_abi::MigrationAuto::none());
		assert!(parsed.is_ok(), "one entrypoint must parse: {parsed:?}");

		let duplicated = source.replace(
			"pub enum OnlyInstruction {",
			"#[discriminator(entrypoint)]\npub enum SecondInstruction {\n\tStop = \
			 1,\n}\n\n#[discriminator(entrypoint)]\npub enum OnlyInstruction {",
		);
		let temp = write_crate(&duplicated);
		let error = parse_program_with_auto(temp.path(), None, &pina_abi::MigrationAuto::none())
			.expect_err("a second entrypoint enum must fail closed");
		let message = error.to_string();
		assert!(message.contains("entrypoint"), "message: {message}");
		assert!(
			message.contains("OnlyInstruction") && message.contains("SecondInstruction"),
			"the error names both enums: {message}"
		);
	}

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
		assert_eq!(
			infer_pda_name_for_field("counter_state", &pdas),
			Some("counter".to_owned())
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

			#[instruction(
				discriminator = EventsInstruction,
				variant = Initialize,
				migrations
			)]
			pub struct InitializeInstruction {}

			#[instruction(discriminator = EventsInstruction, variant = TestEvent, migrations)]
			pub struct TestEventInstruction {}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let ir = assemble_program_ir(&file, "events").unwrap_or_else(|e| panic!("assemble: {e}"));

		assert_eq!(ir.instructions.len(), 2);
		assert_eq!(ir.instructions[0].name, "initialize");
		assert_eq!(ir.instructions[0].accounts.len(), 0);
		assert_eq!(ir.instructions[0].docs, [crate::ir::MIGRATABLE_DOC_MARKER]);
		assert_eq!(ir.instructions[1].name, "test_event");
		assert_eq!(ir.instructions[1].accounts.len(), 0);
		assert!(
			ir.instructions[1]
				.docs
				.iter()
				.any(|doc| doc == crate::ir::MIGRATABLE_DOC_MARKER)
		);
	}

	#[test]
	fn assemble_program_ir_marks_only_migration_aware_events() {
		let source = r#"
			declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

			#[discriminator]
			pub enum Events {
				Current = 1,
				Ephemeral = 2,
			}

			#[event(discriminator = Events::Current, migrations)]
			pub struct CurrentEvent { pub value: u64 }

			#[event(discriminator = Events::Ephemeral)]
			pub struct EphemeralEvent { pub value: u64 }
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let ir = assemble_program_ir(&file, "events").unwrap_or_else(|e| panic!("assemble: {e}"));

		assert_eq!(ir.events.len(), 2);
		assert!(ir.events[0].is_migratable());
		assert_eq!(ir.events[0].docs, [crate::ir::MIGRATABLE_DOC_MARKER]);
		assert_eq!(ir.events[0].visible_docs(), Vec::<String>::new());
		assert!(!ir.events[1].is_migratable());
		assert!(ir.events[1].visible_docs().is_empty());
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
				fn process(self, _data: &[u8]) -> ProgramResult {
					Ok(())
				}
			}

			pub mod entrypoint {
				use super::*;

				pub fn process_instruction(
					program_id: &Address,
					accounts: &mut [AccountView],
					data: &[u8],
				) -> ProgramResult {
					let instruction: DuplicateMutableInstruction = parse_instruction(program_id, &ID, data)?;

					match instruction {
						DuplicateMutableInstruction::FailsDuplicateMutable => {
							DuplicateMutableAccounts::try_from((program_id, accounts))?.process(data)
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
	fn assemble_program_ir_marks_mutable_account_fields_writable() {
		let source = r#"
			declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

			#[discriminator]
			pub enum MutableInstruction {
				Initialize = 0,
			}

			#[instruction(discriminator = MutableInstruction, variant = Initialize)]
			pub struct InitializeInstruction {}

			#[derive(Accounts, Debug)]
			pub struct InitializeAccounts<'a> {
				pub authority: &'a AccountView,
				pub state: &'a mut AccountView,
			}

			impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					let _ = InitializeInstruction::try_from_bytes(data)?;
					self.authority.assert_signer()?;
					Ok(())
				}
			}

			pub mod entrypoint {
				use super::*;

				pub fn process_instruction(
					program_id: &Address,
					accounts: &mut [AccountView],
					data: &[u8],
				) -> ProgramResult {
					let instruction: MutableInstruction = parse_instruction(program_id, &ID, data)?;

					match instruction {
						MutableInstruction::Initialize => {
							InitializeAccounts::try_from((program_id, accounts))?.process(data)
						}
					}
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let ir = assemble_program_ir(&file, "mutable").unwrap_or_else(|e| panic!("assemble: {e}"));

		assert_eq!(ir.instructions.len(), 1);
		assert_eq!(ir.instructions[0].accounts.len(), 2);
		assert!(!ir.instructions[0].accounts[0].is_writable);
		assert!(ir.instructions[0].accounts[1].is_writable);
	}

	#[test]
	fn assemble_program_ir_resolves_pdas_from_account_types() {
		let source = r#"
			declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

			const SEED_CONFIG: &[u8] = b"config";
			const SEED_VAULT: &[u8] = b"vault";
			const SEED_POSITION: &[u8] = b"position";

			#[discriminator]
			pub enum PdaAccount {
				ConfigState = 1,
				PoolVault = 2,
				Position = 3,
				Plain = 4,
			}

			#[account(discriminator = PdaAccount)]
			#[pda(seeds = [SEED_CONFIG], bump = bump)]
			pub struct ConfigState {
				pub bump: u8,
			}

			#[account(discriminator = PdaAccount)]
			#[pda(seeds = [SEED_VAULT], bump = bump)]
			pub struct PoolVault {
				pub bump: u8,
			}

			#[account(discriminator = PdaAccount)]
			#[pda(seeds = [SEED_POSITION, owner: Address], bump = bump)]
			pub struct Position {
				pub bump: u8,
			}

			#[account(discriminator = PdaAccount)]
			pub struct Plain {
				pub value: u64,
			}

			#[discriminator]
			pub enum PdaInstruction {
				Run = 0,
			}

			#[instruction(discriminator = PdaInstruction, variant = Run)]
			pub struct RunInstruction {}

			#[derive(Accounts)]
			pub struct RunAccounts<'a> {
				pub owner: &'a AccountView,
				pub config: &'a mut AccountView,
				pub vault: &'a AccountView,
				pub position: &'a mut AccountView,
				pub plain: &'a AccountView,
				pub mixed: &'a AccountView,
				pub checked: &'a AccountView,
				pub pool_vault: &'a AccountView,
			}

			impl<'a> ProcessAccountInfos<'a> for RunAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					let _ = RunInstruction::try_from_bytes(data)?;
					self.config.as_account_mut::<ConfigState>(&ID)?;
					let bump = self.vault.as_account::<PoolVault>(&ID)?.bump;
					PoolVault::assert_stored_bump(self.vault, bump, &ID)?;
					self.position.as_account_mut::<Position>(&ID)?;
					self.plain.as_account::<Plain>(&ID)?;
					self.mixed.as_account::<ConfigState>(&ID)?;
					self.mixed.as_account::<Position>(&ID)?;
					check_position(self.checked, self.owner)?;
					self.pool_vault.assert_seeds_with_bump(&[SEED_VAULT, &[bump]], &ID)?;
					self.pool_vault.as_account::<ConfigState>(&ID)?;
					self.pool_vault.as_account::<Position>(&ID)?;
					Ok(())
				}
			}

			fn check_position(position: &AccountView, owner: &AccountView) -> ProgramResult {
				owner.assert_signer()?;
				Position::load_checked_pda(position, owner.address(), &ID)?;
				Ok(())
			}

			pub mod entrypoint {
				use super::*;

				pub fn process_instruction(
					program_id: &Address,
					accounts: &mut [AccountView],
					data: &[u8],
				) -> ProgramResult {
					let instruction: PdaInstruction = parse_instruction(program_id, &ID, data)?;

					match instruction {
						PdaInstruction::Run => RunAccounts::try_from((program_id, accounts))?.process(data),
					}
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let ir = assemble_program_ir(&file, "typed").unwrap_or_else(|e| panic!("assemble: {e}"));
		let slots = ir.instructions[0]
			.accounts
			.iter()
			.map(|account| {
				(
					account.name.as_str(),
					account.is_pda,
					account.pda_name.as_deref(),
				)
			})
			.collect::<Vec<_>>();

		assert_eq!(
			slots,
			[
				// The helper asserts the owner signs, but nothing makes it a PDA.
				("owner", false, None),
				// A typed load of a constant-seed PDA has one possible address.
				("config", true, Some("config")),
				// The validating call's type names the PDA, not the field name.
				("vault", true, Some("pool_vault")),
				// A typed load of a variable-seed PDA names the PDA, but nothing
				// ties the address to seeds a client could supply.
				("position", false, Some("position")),
				// A typed load of an account without `#[pda]` stays an account.
				("plain", false, None),
				// Two different PDA types cannot both describe one slot.
				("mixed", false, None),
				// The helper's checked loader validates the slot it is handed.
				("checked", true, Some("position")),
				// Ambiguous types fall back to the field name for a validated PDA.
				("pool_vault", true, Some("pool_vault")),
			]
		);
		assert!(ir.instructions[0].accounts[0].is_signer);
	}

	#[test]
	fn assemble_program_ir_rejects_validated_pda_without_identity() {
		let source = r#"
			declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

			#[discriminator]
			pub enum LooseInstruction {
				Run = 0,
			}

			#[instruction(discriminator = LooseInstruction, variant = Run)]
			pub struct RunInstruction {}

			#[derive(Accounts)]
			pub struct RunAccounts<'a> {
				pub anonymous: &'a AccountView,
			}

			impl<'a> ProcessAccountInfos<'a> for RunAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					Unknown::assert_stored_bump(self.anonymous, 255, &ID)?;
					Ok(())
				}
			}

			pub mod entrypoint {
				use super::*;

				pub fn process_instruction(
					program_id: &Address,
					accounts: &mut [AccountView],
					data: &[u8],
				) -> ProgramResult {
					let instruction: LooseInstruction = parse_instruction(program_id, &ID, data)?;

					match instruction {
						LooseInstruction::Run => RunAccounts::try_from((program_id, accounts))?.process(data),
					}
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let error = assemble_program_ir(&file, "loose")
			.expect_err("a validated PDA no type or name identifies must fail closed");

		assert!(error.to_string().contains("anonymous"), "{error}");
	}

	#[test]
	fn assemble_program_ir_reads_accounts_through_versioned_dispatch() {
		// A hand-written dispatcher hands an enveloped instruction's accounts to
		// the generated `process_versioned`. The instruction must keep its
		// accounts: an empty list makes every generated client send none.
		let source = r#"
			declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

			#[discriminator]
			pub enum WalkInstruction {
				Update = 0,
			}

			#[instruction(discriminator = WalkInstruction::Update, migrations)]
			pub struct UpdateInstruction {
				pub score: u64,
			}

			#[derive(Accounts)]
			pub struct UpdateAccounts<'a> {
				#[pina(validate(signer))]
				pub authority: &'a AccountView,
				pub profile: Option<&'a mut AccountView>,
			}

			impl<'a> ProcessAccountInfos<'a> for UpdateAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					let _ = UpdateInstruction::try_from_bytes(data)?;
					Ok(())
				}
			}

			pub mod entrypoint {
				use super::*;

				pub fn process_instruction(
					program_id: &Address,
					accounts: &mut [AccountView],
					data: &[u8],
				) -> ProgramResult {
					let instruction: WalkInstruction = parse_instruction(program_id, &ID, data)?;

					match instruction {
						WalkInstruction::Update => UpdateInstruction::process_versioned(
							UpdateAccounts::try_from((program_id, accounts))?,
							data,
						),
					}
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let ir = assemble_program_ir(&file, "walk").unwrap_or_else(|e| panic!("assemble: {e}"));

		assert_eq!(ir.instructions.len(), 1);
		let accounts = &ir.instructions[0].accounts;
		assert_eq!(
			accounts
				.iter()
				.map(|account| account.name.as_str())
				.collect::<Vec<_>>(),
			["authority", "profile"]
		);
		assert!(accounts[0].is_signer);
		assert!(accounts[1].is_writable);
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
	fn assemble_program_ir_numbers_implicit_error_codes_and_rejects_unevaluable_ones() {
		let source = r#"
			declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

			#[error]
			pub enum ExampleError {
				First = 6000,
				Second,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let ir = assemble_program_ir(&file, "example").unwrap_or_else(|e| panic!("assemble: {e}"));
		let codes: Vec<_> = ir
			.errors
			.iter()
			.map(|error| (error.name.as_str(), error.code))
			.collect();

		assert_eq!(codes, [("First", 6000), ("Second", 6001)]);

		let source = r#"
			declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

			#[error]
			pub enum ExampleError {
				Computed = BASE + 1,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let error = assemble_program_ir(&file, "example").unwrap_err();

		assert!(
			error
				.to_string()
				.contains("cannot evaluate the discriminant of `ExampleError::Computed`")
		);
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

	#[test]
	fn assemble_program_ir_collects_local_pinapod_enums() {
		let source = r#"
			declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

			#[derive(PinaPod)]
			#[repr(u8)]
			pub enum Color {
				Red = 0,
				Blue = 1,
			}

			#[discriminator]
			pub enum ExampleAccount {
				Palette = 0,
			}

			#[account(discriminator = ExampleAccount)]
			pub struct Palette {
				pub color: Color,
				pub recent: Vec<Color, 8>,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|error| panic!("parse failed: {error}"));
		let ir = assemble_program_ir(&file, "example")
			.unwrap_or_else(|error| panic!("assemble failed: {error}"));

		assert_eq!(ir.pinapod_enums.len(), 1);
		assert_eq!(ir.pinapod_enums[0].name, "Color");
		assert_eq!(ir.accounts[0].fields[0].rust_type, "Color");
		assert_eq!(ir.accounts[0].fields[1].rust_type, "Vec<Color, 8>");

		let root = crate::codegen::try_ir_to_root_node(&ir)
			.unwrap_or_else(|error| panic!("lowering failed: {error}"));
		assert_eq!(root.program.defined_types[0].name.as_ref(), "color");
		let account = root.program.accounts[0].data.get_nested_type_node();
		assert!(matches!(
			account.fields[1].r#type.as_ref(),
			codama_nodes::TypeNode::Link(_)
		));
		assert!(matches!(
			account.fields[2].r#type.as_ref(),
			codama_nodes::TypeNode::FixedSize(_)
		));
	}

	#[test]
	fn assemble_program_ir_rejects_duplicate_pinapod_enums() {
		let first = syn::parse_file(
			r#"
				declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");
				#[derive(PinaPod)]
				#[repr(u8)]
				enum Color { Red = 0 }
			"#,
		)
		.unwrap_or_else(|error| panic!("parse failed: {error}"));
		let second = syn::parse_file(
			r#"
				#[derive(PinaPod)]
				#[repr(u8)]
				enum Color { Blue = 0 }
			"#,
		)
		.unwrap_or_else(|error| panic!("parse failed: {error}"));

		let error = assemble_program_ir_multi(&[&first, &second], "example")
			.expect_err("duplicate flattened companions must fail");
		assert!(error.to_string().contains("Duplicate PinaPod enum"));
	}

	#[test]
	fn assemble_program_ir_rejects_multiple_dispatch_sources() {
		let first = syn::parse_file(
			r#"
				declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");
				fn process_instruction() {
					match instruction {
						ExampleInstruction::Run => RunAccounts::try_from((program_id, accounts))?.process(data),
					}
				}
			"#,
		)
		.unwrap_or_else(|error| panic!("parse failed: {error}"));
		let second = syn::parse_file(
			r#"
				fn process_instruction() {
					match instruction {
						ExampleInstruction::Run => RunAccounts::try_from((program_id, accounts))?.process(data),
					}
				}
			"#,
		)
		.unwrap_or_else(|error| panic!("parse failed: {error}"));

		let error = assemble_program_ir_multi(&[&first, &second], "example")
			.expect_err("multiple dispatch sources must fail");
		assert!(error.to_string().contains("sources found (2)"));
	}

	#[test]
	fn validate_program_ir_rejects_duplicate_pdas() {
		let pda = PdaIr {
			name: "vault".to_owned(),
			seeds: Vec::new(),
		};
		let ir = ProgramIr {
			name: "example".to_owned(),
			public_key: "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS".to_owned(),
			pinapod_enums: Vec::new(),
			accounts: Vec::new(),
			instructions: Vec::new(),
			events: Vec::new(),
			errors: Vec::new(),
			pdas: vec![pda.clone(), pda],
		};

		let error = validate_program_ir(&ir).expect_err("duplicate PDAs must fail");
		assert!(
			error
				.to_string()
				.contains("Duplicate PDA definition `vault`")
		);
	}

	#[test]
	fn assemble_from_extracted_rejects_unresolved_account_pda_link() {
		let discriminator = discriminator::DiscriminatorEnum {
			name: "ExampleAccount".to_owned(),
			variants: vec![discriminator::DiscriminatorVariant {
				name: "Vault".to_owned(),
				value: 1,
			}],
			repr_size: 1,
			entrypoint: false,
		};
		let account = account_state::AccountStruct {
			name: "VaultState".to_owned(),
			discriminator_enum: "ExampleAccount".to_owned(),
			variant: "Vault".to_owned(),
			fields: Vec::new(),
			docs: Vec::new(),
			migrations: MigrationOptIn::Explicit,
			pda_name: Some("vault".to_owned()),
		};

		let error = assemble_from_extracted(
			"example",
			"GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS".to_owned(),
			&[discriminator],
			&[account],
			&[],
			&[],
			&[],
			&[],
			&[],
			&[],
			&HashMap::new(),
			&[],
			&pina_abi::MigrationAuto::none(),
		)
		.expect_err("unresolved account PDA links must fail");

		assert!(error.to_string().contains("Account `VaultState`"));
	}

	#[test]
	fn extracts_exact_package_name() {
		let manifest = r#"
			[workspace.package]
			name = "wrong"

			[package]
			name = "example_program"
		"#;

		assert_eq!(
			extract_package_name(manifest).as_deref(),
			Some("example_program")
		);
	}
}
