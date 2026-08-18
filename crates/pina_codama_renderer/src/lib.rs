mod error;
mod render;

use std::collections::BTreeMap;
use std::fs;
use std::path::Component;
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
	let generated_dir = validate_generated_dir(crate_dir, &config.generated_folder)?;
	let files = render_program_to_files(root)?;
	validate_generated_sources(&files)?;
	validate_existing_generated_dir(&generated_dir, config.delete_folder_before_rendering)?;

	ensure_crate_scaffold(crate_dir, root.program.name.as_ref())?;

	if config.delete_folder_before_rendering && generated_dir.exists() {
		fs::remove_dir_all(&generated_dir).map_err(|source| {
			RenderError::WriteFile {
				path: generated_dir.clone(),
				source,
			}
		})?;
	}

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

	// Build program metadata
	let program_constants = std::iter::once(&root.program)
		.chain(root.additional_programs.iter())
		.collect::<Vec<_>>();
	let primary_program_const = program_id_const_name(program.name.as_ref());

	let pdas_by_name = program
		.pdas
		.iter()
		.map(|pda| (pda.name.as_ref().to_string(), pda))
		.collect::<BTreeMap<_, _>>();

	// Core module files
	files.insert(PathBuf::from("mod.rs"), page(&render_root_mod(program)));
	files.insert(
		PathBuf::from("programs.rs"),
		page(&render_programs_mod(&program_constants)?),
	);

	// Account files
	if !program.accounts.is_empty() {
		files.insert(
			PathBuf::from("accounts/mod.rs"),
			page(&render_accounts_mod(&program.accounts)),
		);

		for account in &program.accounts {
			let filename = format!("accounts/{}.rs", snake(account.name.as_ref()));
			let pda = pdas_by_name
				.get(account.pda.as_ref().map_or("", |p| p.name.as_ref()))
				.copied();
			let account_content = render_account_page(account, &primary_program_const, pda)?;

			files.insert(PathBuf::from(filename), page(&account_content));
		}
	}

	// Instruction files
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

	// Type definitions
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

	// Error definitions
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

fn validate_generated_dir(crate_dir: &Path, generated_folder: &Path) -> Result<PathBuf> {
	let mut has_component = false;
	for component in generated_folder.components() {
		has_component = true;
		if !matches!(component, Component::Normal(_)) {
			return Err(RenderError::UnsafeOutputPath {
				path: generated_folder.to_path_buf(),
				reason: "expected a non-empty relative path without `.` or `..` components"
					.to_string(),
			});
		}
	}

	if !has_component {
		return Err(RenderError::UnsafeOutputPath {
			path: generated_folder.to_path_buf(),
			reason: "expected a non-empty relative path".to_string(),
		});
	}

	let generated_dir = crate_dir.join(generated_folder);
	let mut current = crate_dir.to_path_buf();
	for component in generated_folder.components() {
		let Component::Normal(component) = component else {
			unreachable!("components were validated above");
		};
		current.push(component);
		if current.exists() {
			let metadata = fs::symlink_metadata(&current).map_err(|source| {
				RenderError::ReadFile {
					path: current.clone(),
					source,
				}
			})?;
			if metadata.file_type().is_symlink() {
				return Err(RenderError::UnsafeOutputPath {
					path: current,
					reason: "generated output path must not traverse symbolic links".to_string(),
				});
			}
		}
	}

	Ok(generated_dir)
}

fn validate_generated_sources(files: &BTreeMap<PathBuf, String>) -> Result<()> {
	for (path, source) in files {
		syn::parse_file(source).map_err(|error| {
			RenderError::InvalidGeneratedSource {
				path: path.clone(),
				reason: error.to_string(),
			}
		})?;
	}
	Ok(())
}

fn validate_existing_generated_dir(path: &Path, require_managed: bool) -> Result<()> {
	if !path.exists() {
		return Ok(());
	}

	let metadata = fs::symlink_metadata(path).map_err(|source| {
		RenderError::ReadFile {
			path: path.to_path_buf(),
			source,
		}
	})?;
	if !metadata.is_dir() {
		return Err(RenderError::UnsafeOutputPath {
			path: path.to_path_buf(),
			reason: "generated output path exists but is not a directory".to_string(),
		});
	}

	validate_tree_has_no_symlinks(path)?;

	if require_managed {
		let mut entries = fs::read_dir(path).map_err(|source| {
			RenderError::ReadFile {
				path: path.to_path_buf(),
				source,
			}
		})?;
		if entries
			.next()
			.transpose()
			.map_err(|source| {
				RenderError::ReadFile {
					path: path.to_path_buf(),
					source,
				}
			})?
			.is_some()
		{
			let marker_path = path.join("mod.rs");
			if !marker_path.is_file() {
				return Err(RenderError::UnsafeOutputPath {
					path: path.to_path_buf(),
					reason: "refusing to delete a directory not created by this renderer"
						.to_string(),
				});
			}
			let marker = fs::read_to_string(&marker_path).map_err(|source| {
				RenderError::ReadFile {
					path: marker_path.clone(),
					source,
				}
			})?;
			if !marker.starts_with(GENERATED_HEADER) {
				return Err(RenderError::UnsafeOutputPath {
					path: path.to_path_buf(),
					reason: "refusing to delete a directory not created by this renderer"
						.to_string(),
				});
			}
		}
	}

	Ok(())
}

fn validate_tree_has_no_symlinks(path: &Path) -> Result<()> {
	for entry in fs::read_dir(path).map_err(|source| {
		RenderError::ReadFile {
			path: path.to_path_buf(),
			source,
		}
	})? {
		let entry = entry.map_err(|source| {
			RenderError::ReadFile {
				path: path.to_path_buf(),
				source,
			}
		})?;
		let entry_path = entry.path();
		let file_type = entry.file_type().map_err(|source| {
			RenderError::ReadFile {
				path: entry_path.clone(),
				source,
			}
		})?;
		if file_type.is_symlink() {
			return Err(RenderError::UnsafeOutputPath {
				path: entry_path,
				reason: "generated output tree must not contain symbolic links".to_string(),
			});
		}
		if file_type.is_dir() {
			validate_tree_has_no_symlinks(&entry_path)?;
		}
	}
	Ok(())
}

#[cfg(test)]
#[path = "__tests.rs"]
mod tests;
