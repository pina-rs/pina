#![allow(missing_docs)]
mod error;
mod generation_manifest;
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
use render::accounts::migration_envelope;
use render::*;

/// Configuration for rendering a Codama IDL into a client crate.
#[derive(Clone, Debug)]
pub struct RenderConfig {
	/// Remove the generated folder before writing new sources. Defaults to `true`.
	pub delete_folder_before_rendering: bool,
	/// Destination for generated sources, relative to the client crate root. Defaults to `src/generated`.
	pub generated_folder: PathBuf,
	/// Destination policy applied before any files are written. Defaults to [`RenderMode::Auto`].
	pub mode: RenderMode,
	/// Create missing manifests and entrypoints around the generated folder. Defaults to `true`.
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
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

/// Read and parse a Codama IDL JSON file into a [`RootNode`].
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

/// Render a Codama IDL JSON file into the client crate at `crate_dir`.
pub fn render_idl_file(path: &Path, crate_dir: &Path, config: &RenderConfig) -> Result<()> {
	let root = read_root_node(path)?;
	render_root_node(&root, crate_dir, config)
}

/// Render an already-parsed Codama [`RootNode`] into the client crate at `crate_dir`.
///
/// Honors [`RenderConfig::mode`] against the destination current state, writes the
/// generated sources, and scaffolds manifests when [`RenderConfig::scaffold`] is set.
/// Cleanup is bounded by the tracked-files manifest at `crate_dir`: only files
/// a previous render recorded are removed.
pub fn render_root_node(root: &RootNode, crate_dir: &Path, config: &RenderConfig) -> Result<()> {
	validate_output_path_components(crate_dir)?;
	let mode = resolve_render_mode(crate_dir, config.mode)?;
	let manifest = generation_manifest::GenerationManifest::load(crate_dir);

	if mode == RenderMode::Overwrite {
		remove_tracked_crate(crate_dir, manifest.as_ref())?;
	}

	let generated_dir = validate_generated_dir(crate_dir, &config.generated_folder)?;
	let files = render_program_to_files(root)?;
	validate_generated_sources(&files)?;
	validate_existing_generated_dir(&generated_dir, config.delete_folder_before_rendering)?;

	let mut tracked = std::collections::BTreeSet::new();

	if config.scaffold {
		let uses_compact_accounts = root.program.accounts.iter().any(is_compact_account);
		tracked.extend(ensure_crate_scaffold(
			crate_dir,
			root.program.name.as_ref(),
			uses_compact_accounts,
		)?);
	}

	if config.delete_folder_before_rendering && generated_dir.exists() {
		match &manifest {
			// Only files the previous render recorded are removed, so a
			// file the developer dropped into the generated tree survives.
			Some(previous) => previous.remove_under(crate_dir, &config.generated_folder)?,
			// Trees older than tracked manifests keep the historical
			// whole-directory replacement so stale files do not linger.
			None => remove_generated_dir(&generated_dir)?,
		}
	}

	tracked.extend(
		files
			.keys()
			.map(|path| config.generated_folder.join(path))
			.collect::<Vec<_>>(),
	);

	write_files(&generated_dir, files)?;
	generation_manifest::write(crate_dir, &tracked)
}

/// Remove a generated tree as a whole.
fn remove_generated_dir(path: &Path) -> Result<()> {
	fs::remove_dir_all(path).map_err(|source| write_file_error(path, source))
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

/// Remove a crate directory's tracked files for `overwrite`.
///
/// A nonempty destination without a manifest predates tracked cleanup, and
/// removing it wholesale could delete files Pina never wrote, so the render
/// is refused with a remedy instead. The historical guards still apply even
/// though deletion is manifest-bounded: filesystem roots, the working
/// directory, repository trees, and symlinked components are refused.
fn remove_tracked_crate(
	crate_dir: &Path,
	manifest: Option<&generation_manifest::GenerationManifest>,
) -> Result<()> {
	if !crate_dir.exists() {
		return Ok(());
	}

	let Some(tracked) = manifest else {
		return Err(RenderError::InvalidGenerationState {
			path: crate_dir.to_path_buf(),
			mode: "overwrite",
			reason: "the destination predates tracked manifests; remove it by hand, or generate \
				once without `overwrite` to record its files",
		});
	};

	validate_output_path_components(crate_dir)?;

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

	validate_tree_has_no_symlinks(crate_dir)?;
	tracked.remove_under(crate_dir, Path::new(""))
}

fn validate_output_path_components(path: &Path) -> Result<()> {
	let absolute = std::path::absolute(path).map_err(|source| read_file_error(path, source))?;
	let mut current = PathBuf::new();
	let mut depth = 0_usize;

	for component in absolute.components() {
		current.push(component);

		if matches!(component, Component::Prefix(_) | Component::RootDir) {
			continue;
		}

		depth += 1;

		let metadata = match fs::symlink_metadata(&current) {
			Ok(metadata) => metadata,
			Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
			Err(source) => return Err(read_file_error(&current, source)),
		};

		if is_untrusted_link(&metadata, depth == 1) {
			return Err(RenderError::UnsafeOutputPath {
				path: path.to_path_buf(),
				reason: format!(
					"client destinations cannot traverse symbolic link {}",
					current.display()
				),
			});
		}
	}

	Ok(())
}

fn is_link_like(metadata: &fs::Metadata) -> bool {
	#[cfg(windows)]
	{
		use std::os::windows::fs::MetadataExt;

		const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

		metadata.file_type().is_symlink()
			|| metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
	}

	#[cfg(not(windows))]
	metadata.file_type().is_symlink()
}

/// Return whether a link-like path component could redirect a write.
///
/// Every symbolic link and reparse point is untrusted except a system alias: a
/// root-owned link directly below the filesystem root, such as macOS's `/var`
/// and `/tmp` or a merged-`/usr` system's `/bin`. Only root can create or
/// replace an entry there, and no repository or project tree occupies that
/// depth, so the exception cannot admit a project's own links even when the
/// renderer runs as root. Windows reparse points are always untrusted because
/// their owner is not available through the portable metadata API.
fn is_untrusted_link(metadata: &fs::Metadata, top_level: bool) -> bool {
	is_link_like(metadata) && !(top_level && is_owned_by_root(metadata))
}

#[cfg(unix)]
fn is_owned_by_root(metadata: &fs::Metadata) -> bool {
	use std::os::unix::fs::MetadataExt as _;

	metadata.uid() == 0
}

#[cfg(not(unix))]
fn is_owned_by_root(_metadata: &fs::Metadata) -> bool {
	false
}

fn read_file_error(path: &Path, source: std::io::Error) -> RenderError {
	RenderError::ReadFile {
		path: path.to_path_buf(),
		source,
	}
}

pub(crate) fn write_file_error(path: &Path, source: std::io::Error) -> RenderError {
	RenderError::WriteFile {
		path: path.to_path_buf(),
		source,
	}
}

/// Render one program node into the client crate at `crate_dir`.
///
/// Convenience wrapper that wraps the program in a [`RootNode`] and delegates to
/// [`render_root_node`].
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
	let compact_capacities = CompactCapacityIndex::read(program)?;
	let public_defined_types = program
		.defined_types
		.iter()
		.filter(|defined_type| !compact_capacities.is_marker(defined_type.name.as_ref()))
		.cloned()
		.collect::<Vec<_>>();
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
	files.insert(
		PathBuf::from("mod.rs"),
		page(&render_root_mod(program, !public_defined_types.is_empty())),
	);
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
			let account_content =
				render_account_page(account, &primary_program_const, pda, &compact_capacities)?;

			files.insert(PathBuf::from(filename), page(&account_content));
		}
	}

	// Instruction files
	let extra_instruction_modules: Vec<&str> = if program
		.accounts
		.iter()
		.any(|account| migration_envelope(account).is_some())
	{
		vec!["migrate"]
	} else {
		Vec::new()
	};
	if !program.instructions.is_empty() {
		files.insert(
			PathBuf::from("instructions/mod.rs"),
			page(&render_instructions_mod(
				&program.instructions,
				&extra_instruction_modules,
			)),
		);

		for instruction in &program.instructions {
			let filename = format!("instructions/{}.rs", snake(instruction.name.as_ref()));
			let instruction_content =
				render_instruction_page(instruction, program, &primary_program_const)?;

			files.insert(PathBuf::from(filename), page(&instruction_content));
		}
	}

	// The framework-owned `Migrate` instruction, for programs with migratable
	// accounts.
	if program
		.accounts
		.iter()
		.any(|account| migration_envelope(account).is_some())
	{
		files.insert(
			PathBuf::from("instructions/migrate.rs"),
			page(&render_migrate_instruction_page(
				program,
				&primary_program_const,
			)),
		);
	}

	// Event files
	if !program.events.is_empty() {
		files.insert(
			PathBuf::from("events/mod.rs"),
			page(&render_events_mod(&program.events)),
		);

		for event in &program.events {
			let filename = format!("events/{}.rs", snake(event.name.as_ref()));
			let event_content = render_event_page(event)?;

			files.insert(PathBuf::from(filename), page(&event_content));
		}
	}

	// Type definitions
	if !public_defined_types.is_empty() {
		files.insert(
			PathBuf::from("types/mod.rs"),
			page(&render_defined_types_mod(&public_defined_types)),
		);

		for defined_type in public_defined_types {
			let filename = format!("types/{}.rs", snake(defined_type.name.as_ref()));
			let defined_type_content = render_defined_type_page(&defined_type)?;

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
		if metadata.file_type().is_symlink() {
			return Err(RenderError::UnsafeOutputPath {
				path: current,
				reason: "generated output path must not traverse symbolic links".to_string(),
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

			validate_renderer_managed_tree(path)?;
		}
	}

	Ok(())
}

fn validate_renderer_managed_tree(path: &Path) -> Result<()> {
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

		if metadata.is_dir() {
			validate_renderer_managed_tree(&entry_path)?;
			continue;
		}

		if !metadata.is_file() {
			return Err(RenderError::UnsafeOutputPath {
				path: entry_path,
				reason: "generated output tree contains an unmanaged entry".to_string(),
			});
		}

		let source = fs::read_to_string(&entry_path).map_err(|source| {
			RenderError::ReadFile {
				path: entry_path.clone(),
				source,
			}
		})?;
		if !source.starts_with(GENERATED_HEADER) {
			return Err(RenderError::UnsafeOutputPath {
				path: entry_path,
				reason: "refusing to delete a generated directory containing unmanaged files"
					.to_string(),
			});
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
