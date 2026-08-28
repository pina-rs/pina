pub mod build;
pub mod codama;
pub mod codegen;
pub mod error;
pub mod idl_metadata;
pub mod init;
pub mod ir;
pub mod parse;
pub mod project;
pub mod verification;
pub mod workflow;

mod dart_client;
mod js_client;
mod verifiable;

use std::path::Path;

use codama_nodes::RootNode;

pub use crate::codama::CodamaGenerateOptions;
pub use crate::codama::ProjectGenerateOptions;
pub use crate::codama::ProjectGenerateOutput;
pub use crate::codama::generate_codama;
pub use crate::codama::generate_project_clients;
use crate::codegen::try_ir_to_root_node;
use crate::error::IdlError;
pub use crate::init::init_project;
pub use crate::init::print_next_steps;
use crate::parse::parse_program;

/// Generate a Codama IDL `RootNode` from a Pina program crate.
///
/// `program_path` should point to the crate root (the directory containing
/// `Cargo.toml`). If `name_override` is provided it replaces the package name
/// from `Cargo.toml`.
pub fn generate_idl(
	program_path: &Path,
	name_override: Option<&str>,
) -> Result<RootNode, IdlError> {
	let ir = parse_program(program_path, name_override)?;
	try_ir_to_root_node(&ir)
}
