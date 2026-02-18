pub mod codegen;
pub mod error;
pub mod ir;
pub mod parse;

use std::path::Path;

use codama_nodes::RootNode;

use crate::codegen::ir_to_root_node;
use crate::error::IdlError;
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
	Ok(ir_to_root_node(&ir))
}
