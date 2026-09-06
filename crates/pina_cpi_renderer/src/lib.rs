//! Codama Rust renderer that generates Pinocchio CPI clients.
//!
//! Point the renderer at a Codama root node — the output of `@codama/nodes-from-anchor`
//! for Anchor IDLs, or `pina generate` for Pina programs — and it renders a
//! standalone, `no_std` CPI crate: one call struct per instruction with direct
//! account references and a typed instruction-data struct. Generated builders
//! expose Pina-native `invoke` and `invoke_signed` methods.
//!
//! Program-ID placeholder optional accounts and runtime-selected signers are
//! represented directly in the generated API. The renderer refuses rather
//! than guess for omitted optional accounts, optional arguments,
//! non-little-endian numbers, and unsupported argument or discriminator types,
//! with errors naming the exact node.
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

use cap_std::fs::Dir;
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
use render::scaffold::open_crate_dir;
use render::scaffold::write_files;

mod error;
mod render;

#[cfg(test)]
mod __tests;

#[derive(Clone, Debug)]
pub struct RenderConfig {
	pub delete_folder_before_rendering: bool,
	pub generated_folder: PathBuf,
	pub mode: RenderMode,
	pub scaffold: bool,
}

impl Default for RenderConfig {
	fn default() -> Self {
		Self {
			delete_folder_before_rendering: true,
			generated_folder: PathBuf::from("src/generated"),
			mode: RenderMode::Auto,
			scaffold: true,
		}
	}
}

/// Controls how a renderer treats the client destination.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RenderMode {
	/// Create an empty destination or update an existing client.
	#[default]
	Auto,
	/// Require an empty or nonexistent destination.
	Create,
	/// Require an existing, nonempty destination.
	Update,
	/// Remove the complete destination before rendering.
	Overwrite,
}

impl RenderMode {
	const fn as_str(self) -> &'static str {
		match self {
			Self::Auto => "automatically generate",
			Self::Create => "create",
			Self::Update => "update",
			Self::Overwrite => "overwrite",
		}
	}
}

pub fn read_root_node(path: &Path) -> Result<RootNode> {
	let idl = fs::read_to_string(path).map_err(|source| read_file_error(path, source))?;
	serde_json::from_str(&idl).map_err(|source| parse_idl_error(path, source))
}

pub fn render_idl_file(path: &Path, crate_dir: &Path, config: &RenderConfig) -> Result<()> {
	let root = read_root_node(path)?;
	render_root_node(&root, crate_dir, config)
}

pub fn render_root_node(root: &RootNode, crate_dir: &Path, config: &RenderConfig) -> Result<()> {
	let mode = resolve_render_mode(crate_dir, config.mode)?;

	if mode == RenderMode::Overwrite {
		remove_crate_dir(crate_dir)?;
	}

	validate_generated_folder(&config.generated_folder)?;
	let files = render_program_to_files(root)?;
	validate_generated_sources(&files)?;
	let crate_handle = open_crate_dir(crate_dir)?;
	validate_generated_path(&crate_handle, crate_dir, &config.generated_folder)?;
	validate_existing_generated_dir(
		&crate_handle,
		crate_dir,
		&config.generated_folder,
		config.delete_folder_before_rendering,
	)?;
	if config.scaffold {
		ensure_crate_scaffold(
			&crate_handle,
			crate_dir,
			root.program.name.as_ref(),
			&config.generated_folder,
		)?;
	}

	if config.delete_folder_before_rendering {
		remove_generated_dir(&crate_handle, crate_dir, &config.generated_folder)?;
	}

	write_files(&crate_handle, crate_dir, &config.generated_folder, &files)
}

fn resolve_render_mode(crate_dir: &Path, requested: RenderMode) -> Result<RenderMode> {
	let metadata = match fs::symlink_metadata(crate_dir) {
		Ok(metadata) => Some(metadata),
		Err(source) if source.kind() == std::io::ErrorKind::NotFound => None,
		Err(source) => return Err(read_file_error(crate_dir, source)),
	};
	let is_empty = match metadata {
		None => true,
		Some(metadata) if metadata.file_type().is_symlink() => {
			return Err(RenderError::UnsafeOutputPath {
				path: crate_dir.to_path_buf(),
				reason: "client destinations cannot be symbolic links".to_string(),
			});
		}
		Some(metadata) if !metadata.is_dir() => {
			return Err(RenderError::UnsafeOutputPath {
				path: crate_dir.to_path_buf(),
				reason: "client destinations must be directories".to_string(),
			});
		}
		Some(_) => {
			fs::read_dir(crate_dir)
				.map_err(|source| read_file_error(crate_dir, source))?
				.next()
				.transpose()
				.map_err(|source| read_file_error(crate_dir, source))?
				.is_none()
		}
	};

	match (requested, is_empty) {
		(RenderMode::Auto, true) => Ok(RenderMode::Create),
		(RenderMode::Auto, false) => Ok(RenderMode::Update),
		(RenderMode::Create, false) => {
			Err(RenderError::InvalidGenerationState {
				path: crate_dir.to_path_buf(),
				mode: requested.as_str(),
				reason: "the destination is not empty",
			})
		}
		(RenderMode::Update, true) => {
			Err(RenderError::InvalidGenerationState {
				path: crate_dir.to_path_buf(),
				mode: requested.as_str(),
				reason: "the destination is empty or does not exist",
			})
		}
		(mode, _) => Ok(mode),
	}
}

fn remove_crate_dir(crate_dir: &Path) -> Result<()> {
	if !crate_dir.exists() {
		return Ok(());
	}

	let absolute =
		fs::canonicalize(crate_dir).map_err(|source| read_file_error(crate_dir, source))?;
	let current_dir =
		std::env::current_dir().map_err(|source| read_file_error(Path::new("."), source))?;
	let current =
		fs::canonicalize(&current_dir).map_err(|source| read_file_error(&current_dir, source))?;

	if absolute.parent().is_none()
		|| current.starts_with(&absolute)
		|| absolute.join(".git").exists()
	{
		return Err(RenderError::UnsafeOutputPath {
			path: absolute,
			reason: "refusing to overwrite a filesystem root or working tree".to_string(),
		});
	}

	validate_ambient_tree_has_no_symlinks(crate_dir)?;
	fs::remove_dir_all(crate_dir).map_err(|source| write_file_error(crate_dir, source))
}

fn validate_ambient_tree_has_no_symlinks(path: &Path) -> Result<()> {
	for entry in fs::read_dir(path).map_err(|source| read_file_error(path, source))? {
		let entry = entry.map_err(|source| read_file_error(path, source))?;
		let entry_path = entry.path();
		let file_type = entry
			.file_type()
			.map_err(|source| read_file_error(&entry_path, source))?;

		if file_type.is_symlink() {
			return Err(RenderError::UnsafeOutputPath {
				path: entry_path,
				reason: "client destinations cannot contain symbolic links".to_string(),
			});
		}

		if file_type.is_dir() {
			validate_ambient_tree_has_no_symlinks(&entry_path)?;
		}
	}

	Ok(())
}

pub fn render_program(
	program: &ProgramNode,
	crate_dir: &Path,
	config: &RenderConfig,
) -> Result<()> {
	let root = RootNode::new(program.clone());
	render_root_node(&root, crate_dir, config)
}

/// Renders a program into an in-memory file map without touching disk.
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
		page(&render_programs_mod(program, &program_constants)),
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

fn validate_generated_folder(generated_folder: &Path) -> Result<()> {
	let mut component_count = 0usize;
	for component in generated_folder.components() {
		component_count += 1;
		if !matches!(component, Component::Normal(_)) {
			return Err(RenderError::UnsafeOutputPath {
				path: generated_folder.to_path_buf(),
				reason: "expected a non-empty relative path without `.` or `..` components"
					.to_string(),
			});
		}
	}

	if component_count < 2 || !generated_folder.starts_with("src") {
		return Err(RenderError::UnsafeOutputPath {
			path: generated_folder.to_path_buf(),
			reason: "expected a non-empty relative path below `src`".to_string(),
		});
	}

	Ok(())
}

fn validate_generated_path(
	crate_dir: &Dir,
	crate_path: &Path,
	generated_folder: &Path,
) -> Result<()> {
	let mut current = PathBuf::new();
	for component in generated_folder.components() {
		current.push(component.as_os_str());
		let metadata = match crate_dir.symlink_metadata(&current) {
			Ok(metadata) => metadata,
			Err(source) if source.kind() == std::io::ErrorKind::NotFound => continue,
			Err(source) => return Err(read_file_error(&crate_path.join(&current), source)),
		};
		if metadata.file_type().is_symlink() {
			return Err(RenderError::UnsafeOutputPath {
				path: crate_path.join(&current),
				reason: "the generated output path must not traverse symlinks".to_string(),
			});
		}
	}

	Ok(())
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

fn validate_existing_generated_dir(
	crate_dir: &Dir,
	crate_path: &Path,
	path: &Path,
	require_managed: bool,
) -> Result<()> {
	let display_path = crate_path.join(path);
	let metadata = match crate_dir.symlink_metadata(path) {
		Ok(metadata) => metadata,
		Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
		Err(source) => return Err(read_file_error(&display_path, source)),
	};
	if !metadata.is_dir() {
		return Err(RenderError::UnsafeOutputPath {
			path: display_path,
			reason: "generated output path exists but is not a directory".to_string(),
		});
	}

	validate_tree_has_no_symlinks(crate_dir, crate_path, path)?;

	if require_managed {
		let mut entries = crate_dir
			.read_dir(path)
			.map_err(|source| read_file_error(&display_path, source))?;
		if entries
			.next()
			.transpose()
			.map_err(|source| read_file_error(&display_path, source))?
			.is_some()
		{
			let marker_path = path.join("mod.rs");
			let marker_is_file = crate_dir
				.symlink_metadata(&marker_path)
				.is_ok_and(|metadata| metadata.is_file());
			if !marker_is_file {
				return Err(RenderError::UnsafeOutputPath {
					path: display_path.clone(),
					reason: "refusing to delete a directory not created by this renderer"
						.to_string(),
				});
			}
			let marker = crate_dir
				.read_to_string(&marker_path)
				.map_err(|source| read_file_error(&crate_path.join(&marker_path), source))?;
			if !marker.starts_with(GENERATED_HEADER) {
				return Err(RenderError::UnsafeOutputPath {
					path: display_path,
					reason: "refusing to delete a directory not created by this renderer"
						.to_string(),
				});
			}
		}
	}

	Ok(())
}

fn validate_tree_has_no_symlinks(crate_dir: &Dir, crate_path: &Path, path: &Path) -> Result<()> {
	let display_path = crate_path.join(path);
	for entry in crate_dir
		.read_dir(path)
		.map_err(|source| read_file_error(&display_path, source))?
	{
		let entry = entry.map_err(|source| read_file_error(&display_path, source))?;
		let entry_path = path.join(entry.file_name());
		let entry_display_path = crate_path.join(&entry_path);
		let metadata = crate_dir
			.symlink_metadata(&entry_path)
			.map_err(|source| read_file_error(&entry_display_path, source))?;
		if metadata.file_type().is_symlink() {
			return Err(RenderError::UnsafeOutputPath {
				path: entry_display_path,
				reason: "the generated output directory must not contain symlinks".to_string(),
			});
		}
		if metadata.is_dir() {
			validate_tree_has_no_symlinks(crate_dir, crate_path, &entry_path)?;
		}
	}

	Ok(())
}

fn remove_generated_dir(crate_dir: &Dir, crate_path: &Path, path: &Path) -> Result<()> {
	match crate_dir.symlink_metadata(path) {
		Ok(_) => {
			crate_dir
				.remove_dir_all(path)
				.map_err(|source| write_file_error(&crate_path.join(path), source))
		}
		Err(source) => missing_generated_dir(&crate_path.join(path), source),
	}
}

fn missing_generated_dir(path: &Path, source: std::io::Error) -> Result<()> {
	if source.kind() == std::io::ErrorKind::NotFound {
		Ok(())
	} else {
		Err(read_file_error(path, source))
	}
}

fn read_file_error(path: &Path, source: std::io::Error) -> RenderError {
	RenderError::ReadFile {
		path: path.to_path_buf(),
		source,
	}
}

fn write_file_error(path: &Path, source: std::io::Error) -> RenderError {
	RenderError::WriteFile {
		path: path.to_path_buf(),
		source,
	}
}

fn parse_idl_error(path: &Path, source: serde_json::Error) -> RenderError {
	RenderError::ParseIdl {
		path: path.to_path_buf(),
		source,
	}
}
