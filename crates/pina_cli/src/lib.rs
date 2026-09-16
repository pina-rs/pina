#![allow(missing_docs)]
pub mod build;
mod client_events;
mod client_migrations;
pub mod codama;
pub mod codegen;
mod compact_capacity;
pub mod cpi;
pub mod deploy;
pub mod doctor;
pub mod error;
pub mod idl_metadata;
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
pub mod verification;
pub mod workflow;

mod dart_client;
mod dart_events;
mod js_client;
mod js_events;
mod verifiable;

use std::path::Path;

use codama_nodes::RootNode;
pub use pina_abi::MigrationVersionType;

pub use crate::codama::CodamaGenerateOptions;
pub use crate::codama::ProjectGenerateOptions;
pub use crate::codama::ProjectGenerateOutput;
pub use crate::codama::generate_codama;
pub use crate::codama::generate_project_clients;
use crate::codegen::try_ir_to_root_node_with_migrations;
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
pub fn generate_idl(
	program_path: &Path,
	name_override: Option<&str>,
) -> Result<RootNode, IdlError> {
	// The manifest is the checked-in policy source, so IDL discovery marks the
	// same contracts the macros enveloped.
	let auto = migrations::manifest_auto_policy(program_path);
	let ir = parse_program_with_auto(program_path, name_override, &auto)?;
	let needs_migration_constants = ir.accounts.iter().any(ir::AccountIr::is_migratable)
		|| ir.instructions.iter().any(ir::InstructionIr::is_migratable)
		|| ir.events.iter().any(ir::EventIr::is_migratable);
	let migrations = needs_migration_constants
		.then(|| migrations::idl_migration_metadata(program_path))
		.transpose()
		.map_err(|error| IdlError::Other(error.to_string()))?
		.flatten();
	try_ir_to_root_node_with_migrations(&ir, migrations.as_ref())
}
