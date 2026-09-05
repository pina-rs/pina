//! Codama Rust renderer that generates Pinocchio CPI clients.
//!
//! Point the renderer at a Codama root node — the output of `@codama/nodes-from-anchor`
//! for Anchor IDLs, or `pina generate` for Pina programs — and it renders a
//! standalone, `no_std` CPI crate: one builder per instruction, each owning its
//! discriminator bytes, argument encoding, and account metadata, ready to be
//! consumed from another program via `pinocchio::cpi::invoke_signed`.
//!
//! The renderer refuses rather than guess: optional accounts, optional
//! signers, optional arguments, non-little-endian numbers, and unsupported
//! argument or discriminator types are rejected with errors naming the exact
//! node, instead of generating instruction data that would never dispatch.
//! Accounts the IDL derives from PDA seeds stay ordinary builder fields — at
//! CPI time the caller passes the derived account explicitly anyway, because
//! the runtime resolves CPI accounts against the executing program's own
//! account list.
//!
//! # Examples
//!
//! ```no_run
//! use std::path::Path;
//!
//! use pina_cpi_renderer::RenderConfig;
//! use pina_cpi_renderer::render_idl_file;
//!
//! render_idl_file(
//! 	Path::new("codama/idls/vesting_program.json"),
//! 	Path::new("clients/vesting-cpi"),
//! 	&RenderConfig::default(),
//! )
//! .unwrap_or_else(|error| panic!("render failed: {error}"));
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codama_nodes::ProgramNode;
use codama_nodes::RootNode;
pub use error::RenderError;
pub use error::Result;
use render::helpers::GENERATED_HEADER;
use render::helpers::canonical_pubkey;
use render::helpers::page;
use render::helpers::program_id_const_name;
use render::helpers::rust_string_literal;
use render::helpers::snake;
use render::instructions::render_instruction_page;
use render::instructions::render_instructions_mod;
use render::mods::render_programs_mod;
use render::mods::render_root_mod;
use render::scaffold::ensure_crate_scaffold;
use render::scaffold::write_files;

mod error;
mod render;

#[cfg(test)]
mod __tests;

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

	write_files(&generated_dir, &files)
}

pub fn render_program(
	program: &ProgramNode,
	crate_dir: &Path,
	config: &RenderConfig,
) -> Result<()> {
	let root = RootNode::new(program.clone());
	render_root_node(&root, crate_dir, config)
}

/// Renders the program into an in-memory file map without touching disk.
///
/// Useful for inspecting or post-processing the generated sources; use
/// [`render_root_node`] to write them to a crate directory.
pub fn render_program_to_files(root: &RootNode) -> Result<BTreeMap<PathBuf, String>> {
	let program = &root.program;
	let mut files = BTreeMap::new();

	let mut program_constants = Vec::new();
	for program in std::iter::once(program).chain(root.additional_programs.iter()) {
		let docs = program.docs.iter().cloned().collect::<Vec<_>>().join("\n");
		let public_key = canonical_pubkey(&program.public_key, "program node")?;
		program_constants.push((
			program_id_const_name(program.name.as_ref()),
			rust_string_literal(&public_key),
			docs,
		));
	}

	files.insert(PathBuf::from("mod.rs"), page(&render_root_mod(program)));
	files.insert(
		PathBuf::from("programs.rs"),
		page(&render_programs_mod(&program_constants)),
	);

	if !program.instructions.is_empty() {
		files.insert(
			PathBuf::from("instructions/mod.rs"),
			page(&render_instructions_mod(&program.instructions)),
		);

		for instruction in &program.instructions {
			let filename = format!("instructions/{}.rs", snake(instruction.name.as_ref()));
			let instruction_content = render_instruction_page(instruction)?;

			files.insert(PathBuf::from(filename), page(&instruction_content));
		}
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
		let metadata = match fs::symlink_metadata(&current) {
			Ok(metadata) => metadata,
			Err(source) if source.kind() == std::io::ErrorKind::NotFound => continue,
			Err(source) => {
				return Err(RenderError::ReadFile {
					path: current.clone(),
					source,
				});
			}
		};
		if metadata.is_symlink() {
			return Err(RenderError::UnsafeOutputPath {
				path: current.clone(),
				reason: "the generated output path must not traverse symlinks".to_string(),
			});
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
		let metadata = fs::symlink_metadata(&entry_path).map_err(|source| {
			RenderError::ReadFile {
				path: entry_path.clone(),
				source,
			}
		})?;
		if metadata.is_symlink() {
			return Err(RenderError::UnsafeOutputPath {
				path: entry_path,
				reason: "the generated output directory must not contain symlinks".to_string(),
			});
		}
		if metadata.is_dir() {
			validate_tree_has_no_symlinks(&entry_path)?;
		}
	}

	Ok(())
}
