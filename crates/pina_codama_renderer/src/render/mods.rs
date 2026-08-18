use codama_nodes::ProgramNode;

use super::helpers::canonical_pubkey;
use super::helpers::program_id_const_name;
use crate::error::Result;

pub(crate) fn render_root_mod(program: &ProgramNode) -> String {
	let mut lines = Vec::new();

	if !program.accounts.is_empty() {
		lines.push("pub mod accounts;".to_string());
	}
	if !program.errors.is_empty() {
		lines.push("pub mod errors;".to_string());
	}
	if !program.instructions.is_empty() {
		lines.push("pub mod instructions;".to_string());
	}
	lines.push("pub mod programs;".to_string());
	if !program.defined_types.is_empty() {
		lines.push("pub mod types;".to_string());
	}
	lines.push(String::new());
	lines.push("#[allow(unused_imports)]".to_string());
	lines.push("pub(crate) use programs::*;".to_string());

	lines.join("\n")
}

pub(crate) fn render_programs_mod(programs: &[&ProgramNode]) -> Result<String> {
	let mut lines = Vec::new();
	lines.push("use solana_pubkey::{pubkey, Pubkey};".to_string());
	lines.push(String::new());

	for program in programs {
		let program_name = program.name.as_ref();
		let public_key = canonical_pubkey(
			&program.public_key,
			&format!("program `{program_name}` public key"),
		)?;
		lines.push(format!(
			"pub const {}: Pubkey = pubkey!(\"{}\");",
			program_id_const_name(program_name),
			public_key
		));
	}

	Ok(lines.join("\n"))
}
