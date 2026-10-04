#![allow(missing_docs)]
pub mod abi;
pub mod build;
mod client_compute_units;
mod client_manifest;
mod client_migrations;
pub mod codama;
pub mod codegen;
mod compact_capacity;
pub mod compute_units;
pub mod cpi;
pub mod deploy;
pub mod doctor;
pub mod error;
pub mod explain;
pub mod idl_metadata;
pub mod import_idl;
pub mod init;
pub mod ir;
pub mod keys;
pub mod lint;
pub mod lint_catalog;
pub mod lint_driver;
pub mod lint_reference;
pub mod lint_toolchain;
pub mod migrations;
pub mod parse;
mod path_security;
pub mod profile;
pub mod project;
pub mod skill;
pub mod verification;
pub mod workflow;

mod dart_client;
mod dart_events;
mod js_client;
mod js_events;
#[cfg(all(test, unix))]
#[path = "../tests/support/mod.rs"]
mod test_support;
mod verifiable;

use std::path::Path;

use codama_nodes::RootNode;
pub use pina_abi::MigrationVersionType;

pub use crate::codama::ProjectGenerateOptions;
pub use crate::codama::ProjectGenerateOutput;
pub use crate::codama::generate_project_clients;
use crate::codegen::try_ir_to_root_node_with_migrations;
use crate::compute_units::MeasurementUse;
pub use crate::cpi::CpiGenerateOptions;
pub use crate::cpi::generate_cpi_crate;
pub use crate::cpi::generate_cpi_crate_from_reader;
pub use crate::cpi::generate_cpi_crate_from_reader_with_config;
use crate::error::IdlError;
pub use crate::init::init_project;
pub use crate::init::print_next_steps;
use crate::parse::parse_program_with_auto;
pub use crate::project::GenerationMode;

/// Generate a Codama IDL `RootNode` from a Pina program crate.
///
/// `program_path` should point to the crate root (the directory containing
/// `Cargo.toml`). If `name_override` is provided it replaces the package name
/// from `Cargo.toml`.
///
/// A `compute-units.json` beside the manifest attaches each recorded
/// instruction's measured compute units and requested limit as its
/// `pinaComputeUnits` plugin. A measurement for an instruction the program no
/// longer declares is reported on stderr and ignored, so renaming an
/// instruction never stops IDL generation.
pub fn generate_idl(
	program_path: &Path,
	name_override: Option<&str>,
) -> Result<RootNode, IdlError> {
	generate_idl_with(program_path, name_override, MeasurementUse::Lenient)
}

/// Generate an IDL, choosing what happens to recorded compute unit
/// measurements.
pub(crate) fn generate_idl_with(
	program_path: &Path,
	name_override: Option<&str>,
	measurements: MeasurementUse,
) -> Result<RootNode, IdlError> {
	let ir = parse_program_ir(program_path, name_override)?;
	let needs_migration_constants = ir.accounts.iter().any(ir::AccountIr::is_migratable)
		|| ir.instructions.iter().any(ir::InstructionIr::is_migratable)
		|| ir.events.iter().any(ir::EventIr::is_migratable);
	let migrations = needs_migration_constants
		.then(|| migrations::idl_migration_metadata(program_path))
		.transpose()
		.map_err(|error| IdlError::Other(error.to_string()))?
		.flatten();
	let mut root = try_ir_to_root_node_with_migrations(&ir, migrations.as_ref())?;
	let warning = compute_units::attach_compute_unit_budgets(program_path, &mut root, measurements)
		.map_err(|error| IdlError::Other(error.to_string()))?;

	if let Some(warning) = warning {
		eprintln!("warning: {warning}");
	}

	Ok(root)
}

/// Parse a Pina program crate into the intermediate representation that IDL
/// generation lowers.
pub(crate) fn parse_program_ir(
	program_path: &Path,
	name_override: Option<&str>,
) -> Result<ir::ProgramIr, IdlError> {
	// The manifest is the checked-in policy source, so IDL discovery marks the
	// same contracts the macros enveloped.
	let auto = migrations::manifest_auto_policy(program_path);

	parse_program_with_auto(program_path, name_override, &auto)
}
