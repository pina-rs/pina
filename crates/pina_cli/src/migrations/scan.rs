//! Discovery of the migratable contracts declared by a project.

use std::collections::BTreeMap;

use pina_abi::ContractIdentity;
use pina_abi::ContractKind;
use pina_abi::DataSchema;
use pina_abi::FieldSchema;
use pina_abi::LayoutKind;
use pina_abi::MigrationManifest;
use pina_abi::MigrationVersionType;
use pina_abi::ProcessAccount;
use pina_abi::ProcessContract;

use super::MigrationError;
use crate::ir::DefaultValueIr;
use crate::ir::DiscriminatorIr;
use crate::ir::InstructionIr;
use crate::parse;
use crate::project::Project;

/// One source contract discovered during migration inspection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CurrentContract {
	pub(super) identity: ContractIdentity,
	pub(super) rust_name: String,
	pub(super) schema: DataSchema,
	pub(super) process: Option<ProcessContract>,
}

pub(super) fn next_migration_version(
	identity: &str,
	current: u32,
	version_type: MigrationVersionType,
) -> Result<u32, MigrationError> {
	if current == version_type.max_version() {
		return Err(MigrationError::VersionExhausted {
			version_type: version_type.to_string(),
			identity: identity.to_owned(),
		});
	}

	// The configured maximum is at most `u32::MAX`, and equality returned above.
	Ok(current + 1)
}

pub(super) struct CurrentProgram {
	pub(super) program_id: String,
	pub(super) contracts: Vec<CurrentContract>,
}

pub(super) fn scan_current_contracts(project: &Project) -> Result<CurrentProgram, MigrationError> {
	let (ir, files) =
		parse::parse_program_with_sources(&project.program_dir, Some(&project.library_name))?;
	let mut discriminators = Vec::new();
	let mut events = Vec::new();
	for resolved in &files {
		// The shared parser has already validated discriminator declarations in
		// these exact syntax trees while assembling `ir`: the same pure
		// extractor ran over the same files inside `parse_program_with_sources`
		// and any error surfaced there, so this pass cannot fail.
		discriminators.extend(
			parse::discriminator::extract_discriminator_enums(&resolved.file).unwrap_or_else(
				|error| panic!("the program parser validated every discriminator enum: {error:?}"),
			),
		);
		events.extend(parse::event_data::extract_migratable_events(
			&resolved.file,
		)?);
	}
	let discriminator_map = parse::build_discriminator_map(&discriminators);
	let mut contracts = BTreeMap::new();

	for account in ir.accounts.iter().filter(|account| account.is_migratable()) {
		let discriminator = &account.discriminator;
		let identity = ContractIdentity::try_new(
			ContractKind::Account,
			discriminator.repr_size,
			discriminator.value,
		)
		.map_err(MigrationError::InvalidHistory)?;
		let layout = if account.is_compact() {
			LayoutKind::Compact
		} else {
			LayoutKind::Fixed
		};
		let fields = account
			.fields
			.iter()
			.map(|field| {
				FieldSchema {
					name: field.name.clone(),
					rust_type: field.rust_type.clone(),
				}
			})
			.collect();
		let schema = DataSchema::try_new(layout, fields).map_err(MigrationError::InvalidHistory)?;
		insert_current(
			&mut contracts,
			CurrentContract {
				identity,
				rust_name: account.name.clone(),
				schema,
				process: None,
			},
		)
		.unwrap_or_else(|error| {
			panic!("the program parser rejected colliding account identities: {error:?}")
		});
	}

	for instruction in ir
		.instructions
		.iter()
		.filter(|instruction| instruction.is_migratable())
	{
		let discriminator = &instruction.discriminator;
		let identity = ContractIdentity::try_new(
			ContractKind::Instruction,
			discriminator.repr_size,
			discriminator.value,
		)
		.map_err(MigrationError::InvalidHistory)?;
		let fields = instruction
			.arguments
			.iter()
			.map(|field| {
				FieldSchema {
					name: field.name.clone(),
					rust_type: field.rust_type.clone(),
				}
			})
			.collect();
		let schema = DataSchema::try_new(LayoutKind::Fixed, fields)
			.map_err(MigrationError::InvalidHistory)?;
		insert_current(
			&mut contracts,
			CurrentContract {
				identity,
				rust_name: instruction.rust_name.clone(),
				schema,
				process: Some(process_contract(instruction)),
			},
		)
		.unwrap_or_else(|error| {
			panic!("the program parser rejected colliding instruction identities: {error:?}")
		});
	}

	for event in events {
		let discriminator = resolve_discriminator(
			&discriminator_map,
			&event.discriminator_enum,
			&event.variant,
			"event",
		)?;
		let identity = ContractIdentity::try_new(
			ContractKind::Event,
			discriminator.repr_size,
			discriminator.value,
		)
		.map_err(MigrationError::InvalidHistory)?;
		insert_current(
			&mut contracts,
			CurrentContract {
				identity,
				rust_name: event.name,
				schema: event.schema,
				process: None,
			},
		)?;
	}

	Ok(CurrentProgram {
		program_id: ir.public_key,
		contracts: contracts.into_values().collect(),
	})
}

pub(super) fn resolve_discriminator<'a>(
	map: &'a std::collections::HashMap<(String, String), DiscriminatorIr>,
	enum_name: &str,
	variant: &str,
	kind: &str,
) -> Result<&'a DiscriminatorIr, MigrationError> {
	map.get(&(enum_name.to_owned(), variant.to_owned()))
		.ok_or_else(|| {
			MigrationError::InvalidHistory(format!(
				"could not resolve {kind} discriminator `{enum_name}::{variant}`"
			))
		})
}

pub(super) fn insert_current(
	contracts: &mut BTreeMap<String, CurrentContract>,
	contract: CurrentContract,
) -> Result<(), MigrationError> {
	let key = contract.identity.key();
	if contracts.insert(key.clone(), contract).is_some() {
		return Err(MigrationError::DuplicateIdentity { identity: key });
	}
	Ok(())
}

pub(super) fn process_contract(instruction: &InstructionIr) -> ProcessContract {
	ProcessContract {
		accounts: instruction
			.accounts
			.iter()
			.map(|account| {
				ProcessAccount {
					name: account.name.clone(),
					writable: account.is_writable,
					signer: account.is_signer,
					optional: account.is_optional,
					default_value: account.default_value.as_ref().map(|value| {
						match value {
							DefaultValueIr::ProgramId(value) => format!("program:{value}"),
							DefaultValueIr::PublicKey(value) => format!("publicKey:{value}"),
						}
					}),
					pda: account.pda_name.clone(),
					constraints: account.constraints.clone(),
				}
			})
			.collect(),
	}
}

pub(super) fn validate_program_configuration(
	project: &Project,
	program_id: &str,
	manifest: &MigrationManifest,
) -> Result<(), MigrationError> {
	if manifest.program_id != program_id {
		return Err(MigrationError::ProgramIdentityChanged {
			expected: program_id.to_owned(),
			found: manifest.program_id.clone(),
		});
	}
	if manifest.version_type != project.migration_version_type {
		return Err(MigrationError::VersionTypeChanged {
			expected: project.migration_version_type.to_string(),
			found: manifest.version_type.to_string(),
		});
	}
	Ok(())
}
