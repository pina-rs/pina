//! Module and root page rendering.

use codama_nodes::ProgramNode;

use super::helpers::pascal;

pub(crate) fn render_root_mod(program: &ProgramNode) -> String {
	let mut lines = vec![
		"mod instructions;".to_string(),
		"mod programs;".to_string(),
		String::new(),
		"pub use instructions::*;".to_string(),
		"pub use programs::*;".to_string(),
	];

	if !program.instructions.is_empty() {
		lines.push(String::new());
		lines.push(format!(
			"/// Number of instructions rendered for the `{}` program.",
			program.name.as_ref()
		));
		lines.push(format!(
			"pub const INSTRUCTION_COUNT: usize = {};",
			program.instructions.len()
		));
	}

	lines.join("\n")
}

pub(crate) fn render_programs_mod(
	program: &ProgramNode,
	constants: &[(String, String, String)],
) -> String {
	let marker = pascal(program.name.as_ref());
	let primary_id = &constants[0].0;
	let mut lines = vec![
		"use pina::Address;".to_string(),
		"use pina::CpiProgramId;".to_string(),
		"use pina::Program;".to_string(),
		String::new(),
	];

	for (name, literal, docs) in constants {
		lines.extend(render_constant_docs(docs));
		lines.push(format!(
			"pub const {name}: Address = pina::address!({literal});"
		));
		lines.push(String::new());
	}

	lines.push(format!(
		"/// Marker for the `{}` program used by generated CPI builders.",
		program.name.as_ref()
	));
	lines.push("#[derive(Clone, Copy, Debug)]".to_string());
	lines.push(format!("pub struct {marker};"));
	lines.push(String::new());
	lines.push(format!("impl CpiProgramId for {marker} {{"));
	lines.push(format!("\tconst ID: Address = {primary_id};"));
	lines.push("}".to_string());
	lines.push(String::new());
	lines.push(format!(
		"/// A validated executable account for the `{}` program.",
		program.name.as_ref()
	));
	lines.push(format!(
		"pub type ProgramAccount<'a> = Program<'a, {marker}>;"
	));

	lines.join("\n")
}

fn render_constant_docs(docs: &str) -> Vec<String> {
	if docs.is_empty() {
		Vec::new()
	} else {
		docs.lines().map(|line| format!("/// {line}")).collect()
	}
}
