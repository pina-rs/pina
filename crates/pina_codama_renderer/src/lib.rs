mod error;
mod render;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use codama_nodes::ProgramNode;
use codama_nodes::RootNode;
pub use error::RenderError;
pub use error::Result;
use render::*;

#[derive(Clone, Debug)]
pub struct RenderConfig {
	pub delete_folder_before_rendering: bool,
	pub generated_folder: PathBuf,
}

impl Default for RenderConfig {
	fn default() -> Self {
		Self {
			delete_folder_before_rendering: true,
			generated_folder: PathBuf::from("src/generated"),
		}
	}
}

pub fn read_root_node(path: &Path) -> Result<RootNode> {
	let idl = fs::read_to_string(path).map_err(|source| {
		RenderError::ReadFile {
			path: path.to_path_buf(),
			source,
		}
	})?;
	serde_json::from_str(&idl).map_err(|source| {
		RenderError::ParseIdl {
			path: path.to_path_buf(),
			source,
		}
	})
}

pub fn render_idl_file(path: &Path, crate_dir: &Path, config: &RenderConfig) -> Result<()> {
	let root = read_root_node(path)?;
	render_root_node(&root, crate_dir, config)
}

pub fn render_root_node(root: &RootNode, crate_dir: &Path, config: &RenderConfig) -> Result<()> {
	ensure_crate_scaffold(crate_dir, root.program.name.as_ref())?;
	let generated_dir = crate_dir.join(&config.generated_folder);

	if config.delete_folder_before_rendering && generated_dir.exists() {
		fs::remove_dir_all(&generated_dir).map_err(|source| {
			RenderError::WriteFile {
				path: generated_dir.clone(),
				source,
			}
		})?;
	}

	let files = render_program_to_files(root)?;
	write_files(&generated_dir, files)
}

pub fn render_program(
	program: &ProgramNode,
	crate_dir: &Path,
	config: &RenderConfig,
) -> Result<()> {
	let root = RootNode::new(program.clone());
	render_root_node(&root, crate_dir, config)
}

fn render_program_to_files(root: &RootNode) -> Result<BTreeMap<PathBuf, String>> {
	let program = &root.program;
	let mut files = BTreeMap::new();

	let program_constants = std::iter::once(&root.program)
		.chain(root.additional_programs.iter())
		.collect::<Vec<_>>();
	let primary_program_const = program_id_const_name(program.name.as_ref());

	let pdas_by_name = program
		.pdas
		.iter()
		.map(|pda| (pda.name.as_ref().to_string(), pda))
		.collect::<BTreeMap<_, _>>();

	files.insert(PathBuf::from("mod.rs"), page(&render_root_mod(program)));
	files.insert(
		PathBuf::from("programs.rs"),
		page(&render_programs_mod(&program_constants)),
	);

	if !program.accounts.is_empty() {
		files.insert(
			PathBuf::from("accounts/mod.rs"),
			page(&render_accounts_mod(&program.accounts)),
		);

		for account in &program.accounts {
			let filename = format!("accounts/{}.rs", snake(account.name.as_ref()));
			let account_content = render_account_page(
				account,
				&primary_program_const,
				pdas_by_name
					.get(account.pda.as_ref().map_or("", |p| p.name.as_ref()))
					.copied(),
			)?;
			files.insert(PathBuf::from(filename), page(&account_content));
		}
	}

	if !program.instructions.is_empty() {
		files.insert(
			PathBuf::from("instructions/mod.rs"),
			page(&render_instructions_mod(&program.instructions)),
		);
		for instruction in &program.instructions {
			let filename = format!("instructions/{}.rs", snake(instruction.name.as_ref()));
			let instruction_content =
				render_instruction_page(instruction, program, &primary_program_const)?;
			files.insert(PathBuf::from(filename), page(&instruction_content));
		}
	}

	if !program.defined_types.is_empty() {
		files.insert(
			PathBuf::from("types/mod.rs"),
			page(&render_defined_types_mod(&program.defined_types)),
		);
		for defined_type in &program.defined_types {
			let filename = format!("types/{}.rs", snake(defined_type.name.as_ref()));
			let defined_type_content = render_defined_type_page(defined_type)?;
			files.insert(PathBuf::from(filename), page(&defined_type_content));
		}
	}

	if !program.errors.is_empty() {
		files.insert(
			PathBuf::from("errors/mod.rs"),
			page(&render_errors_mod(program)),
		);
		files.insert(
			PathBuf::from(format!("errors/{}.rs", snake(program.name.as_ref()))),
			page(&render_errors_page(program)),
		);
	}

	Ok(files)
}

#[cfg(test)]
#[path = "__tests.rs"]
mod tests;

