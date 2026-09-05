//! Module and root page rendering.

use codama_nodes::ProgramNode;

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

pub(crate) fn render_programs_mod(constants: &[(String, String, String)]) -> String {
	let mut lines = vec!["use pinocchio::Address;".to_string(), String::new()];

	for (name, literal, docs) in constants {
		lines.extend(render_constant_docs(docs));
		lines.push(format!(
			"pub const {name}: Address = Address::from_str_const({literal});"
		));
		lines.push(String::new());
	}

	while lines.last().is_some_and(String::is_empty) {
		lines.pop();
	}

	lines.join("\n")
}

fn render_constant_docs(docs: &str) -> Vec<String> {
	if docs.is_empty() {
		Vec::new()
	} else {
		docs.lines().map(|line| format!("/// {line}")).collect()
	}
}
