//! Module and root page rendering.

use codama_nodes::ProgramNode;

use super::helpers::pascal;

pub(crate) fn render_root_mod(
	program: &ProgramNode,
	has_types: bool,
	has_accounts: bool,
) -> String {
	let mut lines = vec!["mod programs;".to_string()];

	if has_accounts {
		lines.insert(0, "pub mod accounts;".to_string());
	}

	if !has_types {
		// An empty module keeps the instruction pages' import path valid.
		lines.insert(0, "pub(crate) mod generated_types {}".to_string());
	}

	if has_types {
		lines.insert(0, "pub(crate) mod types;".to_string());
		lines.push(String::new());
		lines.push("pub use types::*;".to_string());
		// Instruction pages import the generated types through a name that
		// resolves whether or not this IDL needed any.
		lines.push(String::new());
		lines.push("pub(crate) use types as generated_types;".to_string());
	}

	if !program.instructions.is_empty() {
		lines.insert(0, "mod instructions;".to_string());
		lines.push(String::new());
		lines.push("pub use instructions::*;".to_string());
	}

	lines.push(String::new());
	lines.push("pub use programs::*;".to_string());

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
	lines.push(String::new());
	lines.push(format!(
		"/// Whether `address` is the `{}` program this crate calls.",
		program.name.as_ref()
	));
	lines.push("///".to_string());
	lines.push(
		"/// Check this before a CPI when the address arrives from caller input, so a\n/// call \
		 can never be redirected to a program this crate was not imported\n/// for."
			.to_string(),
	);
	lines.push("#[inline(always)]".to_string());
	lines.push("pub fn is_expected_program(address: &Address) -> bool {".to_string());
	lines.push(format!("\t*address == {primary_id}"));
	lines.push("}".to_string());
	lines.push(String::new());
	lines.push("#[cfg(test)]".to_string());
	lines.push("mod tests {".to_string());
	lines.push("\tuse super::*;".to_string());
	lines.push(String::new());
	lines.push("\t/// Binds the compiled-in ID to a literal.".to_string());
	lines.push("\t///".to_string());
	lines.push(
		"\t/// A swapped dependency could otherwise retarget every CPI in this crate\n\t/// \
		 without the source changing, so the expected address is asserted here in\n\t/// full \
		 rather than only through the constant."
			.to_string(),
	);
	lines.push("\t#[test]".to_string());
	lines.push("\tfn binds_the_expected_program_id() {".to_string());
	for (name, literal, _) in constants {
		lines.push(format!(
			"\t\tassert_eq!({name}, pina::address!({literal}));"
		));
	}
	lines.push(format!("\t\tassert_eq!({marker}::ID, {primary_id});"));
	lines.push(format!("\t\tassert!(is_expected_program(&{primary_id}));"));
	lines.push("\t}".to_string());
	lines.push(String::new());
	lines.push("\t#[test]".to_string());
	lines.push("\tfn rejects_a_foreign_program_id() {".to_string());
	lines.push(
		"\t\tlet foreign = pina::address!(\"11111111111111111111111111111111\");".to_string(),
	);
	lines.push("\t\tassert!(!is_expected_program(&foreign));".to_string());
	lines.push("\t}".to_string());
	lines.push("}".to_string());

	lines.join("\n")
}

fn render_constant_docs(docs: &str) -> Vec<String> {
	if docs.is_empty() {
		Vec::new()
	} else {
		docs.lines().map(|line| format!("/// {line}")).collect()
	}
}
