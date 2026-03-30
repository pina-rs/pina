use codama_nodes::ProgramNode;

use super::helpers::escape_rust_str;
use super::helpers::pascal;
use super::helpers::render_docs;
use super::helpers::snake;

pub(crate) fn render_errors_mod(program: &ProgramNode) -> String {
	let mut lines = Vec::new();
	lines.push(format!("pub(crate) mod {};", snake(program.name.as_ref())));
	lines.push(format!(
		"pub use self::{}::{}Error;",
		snake(program.name.as_ref()),
		pascal(program.name.as_ref())
	));
	lines.join("\n")
}

pub(crate) fn render_errors_page(program: &ProgramNode) -> String {
	let mut lines = Vec::new();
	lines.push("use num_derive::FromPrimitive;".to_string());
	lines.push("use thiserror::Error;".to_string());
	lines.push(String::new());
	lines.push("#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]".to_string());
	lines.push(format!(
		"pub enum {}Error {{",
		pascal(program.name.as_ref())
	));
	for error in &program.errors {
		for doc_line in render_docs(&error.docs, 1) {
			lines.push(doc_line);
		}
		lines.push(format!("\t/// {} - {}", error.code, error.message));
		lines.push(format!(
			"\t#[error(\"{}\")]",
			escape_rust_str(&error.message)
		));
		lines.push(format!(
			"\t{} = 0x{:X},",
			pascal(error.name.as_ref()),
			error.code
		));
	}
	lines.push("}".to_string());
	lines.push(String::new());
	lines.push(format!(
		"impl From<{}Error> for solana_program_error::ProgramError {{",
		pascal(program.name.as_ref())
	));
	lines.push(format!(
		"\tfn from(value: {}Error) -> Self {{",
		pascal(program.name.as_ref())
	));
	lines.push("\t\tsolana_program_error::ProgramError::Custom(value as u32)".to_string());
	lines.push("\t}".to_string());
	lines.push("}".to_string());

	lines.join("\n")
}
