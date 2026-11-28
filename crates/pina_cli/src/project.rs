//! Project-local configuration and Cargo workspace discovery.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use cargo_metadata::Metadata;
use cargo_metadata::MetadataCommand;
use cargo_metadata::Package;
use cargo_metadata::TargetKind;
use clap::ValueEnum;
use serde::Deserialize;
use serde::Serialize;

use crate::lint_catalog::LintCatalog;
use crate::lint_catalog::LintLevel;

/// Name of the project-local Pina configuration file.
pub const CONFIG_FILE_NAME: &str = "pina.toml";

/// A generated client ecosystem.
///
/// The CLI variants use their kebab-case spelling — the one [`Self::as_str`],
/// the CLI flag, and the generated directory name already use — so `languages`
/// needs no second spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientLanguage {
	Rust,
	Typescript,
	Dart,
	Cpi,
	#[serde(rename = "cli-rust")]
	CliRust,
	#[serde(rename = "cli-ts")]
	CliTs,
	#[serde(rename = "cli-dart")]
	CliDart,
	/// Off-chain Rust client against `solana_kit_core` (Codama Kit renderer).
	///
	/// Kept last so existing variant discriminants stay stable.
	#[serde(rename = "rust-kit")]
	RustKit,
}

impl ClientLanguage {
	/// Return the stable command-line and configuration spelling.
	#[must_use]
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Cpi => "cpi",
			Self::Rust => "rust",
			Self::RustKit => "rust-kit",
			Self::Typescript => "typescript",
			Self::Dart => "dart",
			Self::CliRust => "cli-rust",
			Self::CliTs => "cli-ts",
			Self::CliDart => "cli-dart",
		}
	}

	/// The client ecosystem a generated CLI application depends on.
	#[must_use]
	pub const fn base_client(self) -> Option<Self> {
		match self {
			Self::CliRust => Some(Self::Rust),
			Self::CliTs => Some(Self::Typescript),
			Self::CliDart => Some(Self::Dart),
			Self::Cpi | Self::Rust | Self::RustKit | Self::Typescript | Self::Dart => None,
		}
	}
}

/// How Pina publishes a generated client into its destination.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum GenerationMode {
	/// Create clients in empty destinations and update clients in existing ones.
	#[default]
	Auto,
	/// Require an empty or nonexistent destination.
	Create,
	/// Require an existing, nonempty destination and preserve its scaffold.
	Update,
	/// Remove the complete destination before generating it again.
	Overwrite,
}

impl GenerationMode {
	/// Return the stable command-line and configuration spelling.
	#[must_use]
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Auto => "auto",
			Self::Create => "create",
			Self::Update => "update",
			Self::Overwrite => "overwrite",
		}
	}
}

/// Resolved generation settings for one client ecosystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientGenerationConfig {
	/// Destination directory. Relative paths sit beneath the shared
	/// `clients.output` directory; `{{...}}`-anchored paths resolve to
	/// absolute destinations.
	pub output: PathBuf,
	/// Destination lifecycle policy.
	pub mode: GenerationMode,
	/// Whether initial generation may create package scaffold files.
	pub scaffold: bool,
}

/// Resolved paths and package metadata for one Pina program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
	pub root: PathBuf,
	pub program_dir: PathBuf,
	pub package_name: String,
	pub library_name: String,
	pub library_source: PathBuf,
	/// Crate types declared by the library target, in manifest order.
	pub library_crate_types: Vec<String>,
	pub target_dir: PathBuf,
	pub idl_dir: PathBuf,
	pub clients_dir: PathBuf,
	pub clients: Vec<ClientLanguage>,
	pub client_generation: BTreeMap<ClientLanguage, ClientGenerationConfig>,
	/// Persisted disambiguation answers from `[migrations.answers]`.
	pub migration_answers: MigrationsAnswersConfig,
	#[serde(skip)]
	pub lint_levels: BTreeMap<String, LintLevel>,
	/// Experimental: build with the upstream sbpf-linker toolchain, recorded
	/// as `sbpf-linker = true` under `[project]` in pina.toml (`pina init
	/// --sbpf-linker` writes it). Removing the key reverts `pina build` to
	/// the `cargo-build-sbf` driver.
	pub sbpf_linker: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectConfig {
	#[serde(default)]
	project: ProgramConfig,
	#[serde(default)]
	clients: ClientsConfig,
	#[serde(default)]
	migrations: MigrationsConfig,
	#[serde(default)]
	lints: LintsConfig,
	#[serde(default, alias = "compute-units")]
	compute_units: ComputeUnitsConfig,
	#[serde(default)]
	locks: LocksConfig,
}

/// Write-lock settings from the `[locks]` table, read by `pina locks`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct LocksConfig {
	/// Hotspots the program keeps on purpose, by account name, such as an
	/// admin configuration only admin instructions write.
	pub allow: Vec<String>,
}

/// How recorded compute unit measurements become the limits clients request,
/// configured through the `[compute_units]` table.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ComputeUnitsConfig {
	/// Percentage added to each measurement before rounding, so a limit
	/// absorbs inputs more expensive than the recorded fixtures.
	#[serde(alias = "margin-percent")]
	pub(crate) margin_percent: u32,
}

impl Default for ComputeUnitsConfig {
	fn default() -> Self {
		Self {
			margin_percent: crate::compute_units::DEFAULT_COMPUTE_UNIT_MARGIN_PERCENT,
		}
	}
}

/// Lint level overrides configured through the `[lints]` table.
///
/// Keys are Pina lint names; values are `allow`, `warn`, or `deny`. Lints
/// that are not listed keep their built-in default level.
#[derive(Debug, Default, Deserialize)]
struct LintsConfig(BTreeMap<String, String>);

/// Program-wide migration settings.
///
/// The version envelope width and the auto policy are recorded only in
/// `migrations/manifest.json`, the one source macros read.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct MigrationsConfig {
	/// Persisted disambiguation answers consulted before `pina migrations
	/// make` prompts; command-line flags override these per field.
	#[serde(default)]
	answers: MigrationsAnswersConfig,
}

/// Persisted disambiguation answers under `[migrations.answers]`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct MigrationsAnswersConfig {
	/// Persisted rename answers in `from:to` form.
	pub rename: Vec<String>,
	/// Persisted data-dropping acknowledgements.
	pub assume_removed: Vec<String>,
	/// Added fields whose conversion is written by hand rather than generated.
	pub manual: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ProgramConfig {
	program: String,
	idl_dir: Option<String>,
	/// Experimental: build with the upstream sbpf-linker binary for
	/// `bpfel-unknown-none` instead of the `cargo-build-sbf` driver. `pina
	/// init --sbpf-linker` records this; removing the key reverts every
	/// `pina build` to cargo-build-sbf.
	#[serde(default, alias = "sbpf-linker")]
	sbpf_linker: bool,
	/// Named path anchors declared as `[project.paths]`, usable as
	/// `{{ name }}` in any configured path. Values may only reference the
	/// built-in `{{root}}` repository anchor.
	paths: BTreeMap<String, String>,
}

impl Default for ProgramConfig {
	fn default() -> Self {
		Self {
			program: ".".to_owned(),
			idl_dir: None,
			sbpf_linker: false,
			paths: BTreeMap::new(),
		}
	}
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ClientsConfig {
	output: String,
	languages: Vec<ClientLanguage>,
	mode: GenerationMode,
	scaffold: bool,
	cpi: ClientGenerationOverride,
	rust: ClientGenerationOverride,
	rust_kit: ClientGenerationOverride,
	typescript: ClientGenerationOverride,
	dart: ClientGenerationOverride,
	cli_rust: ClientGenerationOverride,
	cli_ts: ClientGenerationOverride,
	cli_dart: ClientGenerationOverride,
}

impl Default for ClientsConfig {
	fn default() -> Self {
		Self {
			output: "clients".to_owned(),
			languages: vec![ClientLanguage::Rust, ClientLanguage::Typescript],
			mode: GenerationMode::Auto,
			scaffold: true,
			cpi: ClientGenerationOverride::default(),
			rust: ClientGenerationOverride::default(),
			rust_kit: ClientGenerationOverride::default(),
			typescript: ClientGenerationOverride::default(),
			dart: ClientGenerationOverride::default(),
			cli_rust: ClientGenerationOverride::default(),
			cli_ts: ClientGenerationOverride::default(),
			cli_dart: ClientGenerationOverride::default(),
		}
	}
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ClientGenerationOverride {
	output: Option<String>,
	mode: Option<GenerationMode>,
	scaffold: Option<bool>,
}

impl ClientsConfig {
	fn resolved_generation(
		&self,
		templates: &PathTemplates,
		clients_dir: &Path,
	) -> Result<BTreeMap<ClientLanguage, ClientGenerationConfig>, ProjectError> {
		let mut resolved = BTreeMap::new();

		for language in [
			ClientLanguage::Cpi,
			ClientLanguage::Rust,
			ClientLanguage::RustKit,
			ClientLanguage::Typescript,
			ClientLanguage::Dart,
			ClientLanguage::CliRust,
			ClientLanguage::CliTs,
			ClientLanguage::CliDart,
		] {
			let overrides = match language {
				ClientLanguage::Cpi => &self.cpi,
				ClientLanguage::Rust => &self.rust,
				ClientLanguage::RustKit => &self.rust_kit,
				ClientLanguage::Typescript => &self.typescript,
				ClientLanguage::Dart => &self.dart,
				ClientLanguage::CliRust => &self.cli_rust,
				ClientLanguage::CliTs => &self.cli_ts,
				ClientLanguage::CliDart => &self.cli_dart,
			};
			let field = match language {
				ClientLanguage::Cpi => "clients.cpi.output",
				ClientLanguage::Rust => "clients.rust.output",
				ClientLanguage::RustKit => "clients.rust_kit.output",
				ClientLanguage::Typescript => "clients.typescript.output",
				ClientLanguage::Dart => "clients.dart.output",
				ClientLanguage::CliRust => "clients.cli_rust.output",
				ClientLanguage::CliTs => "clients.cli_ts.output",
				ClientLanguage::CliDart => "clients.cli_dart.output",
			};
			let raw_output = overrides
				.output
				.clone()
				.unwrap_or_else(|| language.as_str().to_owned());
			resolved.insert(
				language,
				ClientGenerationConfig {
					output: templates.resolve_output(clients_dir, field, &raw_output)?,
					mode: overrides.mode.unwrap_or(self.mode),
					scaffold: overrides.scaffold.unwrap_or(self.scaffold),
				},
			);
		}

		Ok(resolved)
	}
}

/// Errors produced while locating or loading a Pina project.
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
	#[error("Could not inspect {path}: {source}")]
	InspectPath {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Could not read {path}: {source}")]
	ReadFile {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Could not parse {path}: {source}")]
	ParseToml {
		path: PathBuf,
		source: toml::de::Error,
	},

	#[error("Configured program directory does not contain Cargo.toml: {path}")]
	MissingManifest { path: PathBuf },

	#[error("Invalid `{field}` path `{path}`: {reason}")]
	InvalidConfigPath {
		field: &'static str,
		path: PathBuf,
		reason: &'static str,
	},

	#[error("Unknown path anchor `{{{variable}}}` in `{field}`; known anchors: {known}")]
	UnknownTemplateAnchor {
		field: &'static str,
		variable: String,
		known: String,
	},

	#[error("Invalid `[project.paths]` entry `{name}`: {reason}")]
	InvalidPathsEntry { name: String, reason: String },

	#[error("Cargo metadata discovery failed from {path}: {message}")]
	CargoMetadata { path: PathBuf, message: String },

	#[error(
		"Could not choose a program from the Cargo workspace at {path}. Run from a package \
		 directory or add pina.toml. Candidates: {candidates}"
	)]
	AmbiguousWorkspace { path: PathBuf, candidates: String },

	#[error("Cargo package {package} does not define a Rust library target")]
	MissingLibraryTarget { package: String },

	#[error(
		"Cargo library name `{name}` cannot be used as a path component. Library names must \
		 contain only ASCII letters, digits, underscores, and hyphens because pina derives file \
		 and directory names from them."
	)]
	InvalidLibraryName { name: String },

	#[error("Unknown pina lint `{name}` in [lints]; known lints: {known}")]
	UnknownLint { name: String, known: String },

	#[error(
		"Invalid level `{level}` for lint `{name}` in [lints]; expected `allow`, `warn`, or `deny`"
	)]
	InvalidLintLevel { name: String, level: String },
}

impl Project {
	/// Return the canonical SBF artifact published by `pina build`.
	#[must_use]
	pub fn sbf_artifact(&self) -> PathBuf {
		self.target_dir
			.join("deploy")
			.join(format!("{}.so", self.library_name))
	}

	/// Return the conventional local deployment keypair path.
	#[must_use]
	pub fn keypair(&self) -> PathBuf {
		self.target_dir
			.join("deploy")
			.join(format!("{}-keypair.json", self.library_name))
	}

	/// Discover a project from `start`, preferring the nearest `pina.toml` and
	/// falling back to Cargo metadata for existing projects.
	///
	/// # Errors
	///
	/// Returns an error when the start path cannot be inspected, configuration
	/// is invalid, or Cargo workspace discovery cannot identify one package.
	pub fn discover(start: &Path) -> Result<Self, ProjectError> {
		let start = normalize_start(start)?;

		if let Some(config_path) = find_ancestor_config(&start) {
			return Self::from_config(&config_path);
		}

		Self::from_cargo_metadata(&start)
	}

	/// Read the `[locks]` table from the project's configuration file.
	///
	/// A project discovered without a configuration file uses the defaults.
	///
	/// # Errors
	///
	/// Returns an error when the configuration file cannot be read or parsed.
	pub fn locks_config(&self) -> Result<LocksConfig, ProjectError> {
		let config_path = self.root.join(CONFIG_FILE_NAME);
		if !config_path.is_file() {
			return Ok(LocksConfig::default());
		}

		Ok(read_project_config(&config_path)?.locks)
	}

	fn from_config(config_path: &Path) -> Result<Self, ProjectError> {
		let root = config_path
			.parent()
			.map_or_else(|| PathBuf::from("."), Path::to_path_buf);
		let config = read_project_config(config_path)?;
		let templates = PathTemplates::new(root.clone(), config.project.paths.clone())?;
		let configured_program =
			templates.resolve_dir(&root, "project.program", &config.project.program)?;
		let program_dir = std::fs::canonicalize(&configured_program).map_err(|source| {
			ProjectError::InspectPath {
				path: configured_program,
				source,
			}
		})?;
		let manifest_path = program_dir.join("Cargo.toml");
		if !manifest_path.is_file() {
			return Err(ProjectError::MissingManifest {
				path: manifest_path,
			});
		}
		let fast = try_discover_package_fast(&root, &manifest_path)?;
		let (
			package_name,
			library_name,
			library_source,
			library_crate_types,
			workspace_root,
			target,
		) = if let Some(discovered) = fast {
			(
				discovered.package_name,
				discovered.library_name,
				discovered.library_source,
				discovered.library_crate_types,
				discovered.workspace_root,
				discovered.target_directory,
			)
		} else {
			// Cargo resolves relative environment paths, including CARGO_TARGET_DIR,
			// against its working directory. Build commands use the configuration root,
			// so metadata must use the same root to report the artifact directory.
			let metadata = cargo_metadata(&root, Some(&manifest_path))?;
			let package = package_for_manifest(&metadata, &manifest_path, &root)?;
			let (library_name, library_source, library_crate_types) = library_details(package)?;
			(
				package.name.to_string(),
				library_name,
				library_source,
				library_crate_types,
				metadata.workspace_root.as_std_path().to_path_buf(),
				metadata.target_directory.as_std_path().to_path_buf(),
			)
		};
		let configured_idl_dir = config
			.project
			.idl_dir
			.as_deref()
			.map(|raw| templates.resolve_dir(&root, "project.idl_dir", raw))
			.transpose()?;
		let (target_dir, idl_dir) = resolve_build_output_dirs_from_paths(
			&templates,
			&workspace_root,
			&target,
			&root,
			configured_idl_dir,
		)?;
		let clients_dir = templates.resolve_dir(&root, "clients.output", &config.clients.output)?;
		let client_generation = config
			.clients
			.resolved_generation(&templates, &clients_dir)?;
		let lint_levels = resolve_lint_levels(config.lints.0)?;

		Ok(Self {
			program_dir,
			package_name,
			library_name,
			library_source,
			library_crate_types,
			idl_dir,
			target_dir,
			clients_dir,
			clients: config.clients.languages,
			client_generation,
			migration_answers: config.migrations.answers,
			lint_levels,
			sbpf_linker: config.project.sbpf_linker,
			root,
		})
	}

	fn from_cargo_metadata(start: &Path) -> Result<Self, ProjectError> {
		if let Some(manifest_path) = find_ancestor_manifest(start)
			&& let Some(discovered) = try_discover_package_fast(start, &manifest_path)?
		{
			let root = discovered.program_dir.clone();
			let (target_dir, idl_dir) = resolve_build_output_dirs_from_paths(
				&PathTemplates::empty(root.clone()),
				&discovered.workspace_root,
				&discovered.target_directory,
				start,
				None,
			)?;
			let clients_dir = root.join("clients");
			let clients_config = ClientsConfig::default();
			let client_generation = clients_config
				.resolved_generation(&PathTemplates::empty(root.clone()), &clients_dir)?;

			return Ok(Self {
				program_dir: root.clone(),
				package_name: discovered.package_name,
				library_name: discovered.library_name,
				library_source: discovered.library_source,
				library_crate_types: discovered.library_crate_types,
				idl_dir,
				target_dir,
				clients_dir,
				clients: clients_config.languages,
				client_generation,
				migration_answers: MigrationsAnswersConfig::default(),
				lint_levels: BTreeMap::new(),
				sbpf_linker: false,
				root,
			});
		}

		let metadata = cargo_metadata(start, None)?;
		let mut candidates = metadata
			.packages
			.iter()
			.filter_map(|package| {
				let manifest_path = package.manifest_path.as_std_path();
				let program_dir = manifest_path.parent()?;
				let depth = start.strip_prefix(program_dir).ok()?.components().count();

				Some((depth, package, program_dir))
			})
			.collect::<Vec<_>>();

		candidates.sort_by_key(|(depth, ..)| *depth);
		let selected = if let Some(candidate) = candidates.first() {
			Some(*candidate)
		} else if metadata.packages.len() == 1 {
			let package = &metadata.packages[0];
			package
				.manifest_path
				.as_std_path()
				.parent()
				.map(|program_dir| (0, package, program_dir))
		} else {
			None
		};

		let Some((_, package, program_dir)) = selected else {
			let names = metadata
				.packages
				.iter()
				.map(|package| package.name.as_str())
				.collect::<Vec<_>>()
				.join(", ");

			return Err(ProjectError::AmbiguousWorkspace {
				path: start.to_path_buf(),
				candidates: names,
			});
		};

		let (library_name, library_source, library_crate_types) = library_details(package)?;
		let root = program_dir.to_path_buf();
		let (target_dir, idl_dir) = resolve_build_output_dirs_from_paths(
			&PathTemplates::empty(root.clone()),
			metadata.workspace_root.as_std_path(),
			metadata.target_directory.as_std_path(),
			start,
			None,
		)?;
		let clients_dir = root.join("clients");
		let clients_config = ClientsConfig::default();
		let client_generation = clients_config
			.resolved_generation(&PathTemplates::empty(root.clone()), &clients_dir)?;

		Ok(Self {
			program_dir: root.clone(),
			package_name: package.name.to_string(),
			library_name,
			library_source,
			library_crate_types,
			idl_dir,
			target_dir,
			clients_dir,
			clients: clients_config.languages,
			client_generation,
			migration_answers: MigrationsAnswersConfig::default(),
			lint_levels: BTreeMap::new(),
			sbpf_linker: false,
			root,
		})
	}

	/// Resolve the Cargo workspace root that owns this program.
	///
	/// # Errors
	///
	/// Returns an error when Cargo metadata cannot be loaded.
	pub fn workspace_root(&self) -> Result<PathBuf, ProjectError> {
		let manifest_path = self.program_dir.join("Cargo.toml");
		if let Some(root) = discover_workspace_root_from_manifest(&self.program_dir, &manifest_path)
		{
			return Ok(root);
		}
		cargo_metadata(&self.root, Some(&manifest_path))
			.map(|metadata| metadata.workspace_root.as_std_path().to_path_buf())
	}

	/// Resolve the Cargo workspace root and every member's package directory.
	///
	/// # Errors
	///
	/// Returns an error when Cargo metadata cannot be loaded.
	pub fn workspace_layout(&self) -> Result<WorkspaceLayout, ProjectError> {
		let manifest_path = self.program_dir.join("Cargo.toml");
		let metadata = cargo_metadata(&self.root, Some(&manifest_path))?;

		Ok(WorkspaceLayout {
			root: metadata.workspace_root.as_std_path().to_path_buf(),
			member_dirs: metadata
				.workspace_packages()
				.iter()
				.filter_map(|package| package.manifest_path.parent())
				.map(|directory| directory.as_std_path().to_path_buf())
				.collect(),
		})
	}
}

/// A Cargo workspace root and the package directories of its members.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceLayout {
	/// The workspace root directory.
	pub root: PathBuf,
	/// Each member package's directory.
	pub member_dirs: Vec<PathBuf>,
}

/// The builtin repository anchor and every declared `[project.paths]` name.
const BUILTIN_ANCHOR: &str = "root";

/// Resolves `{{ name }}` anchors in configured paths.
///
/// `{{root}}` expands to the git repository root (the working-tree top level,
/// so linked worktrees resolve to the worktree itself). Every `[project.paths]`
/// entry declares one additional anchor whose value may only reference
/// `{{root}}`. The git root is discovered lazily, so configurations without
/// anchors never invoke git.
struct PathTemplates {
	/// Directory used to discover the repository root; the `pina.toml` folder.
	discovery_dir: PathBuf,
	variables: BTreeMap<String, String>,
	git_root: std::cell::RefCell<Option<PathBuf>>,
}

impl PathTemplates {
	/// Build a resolver from declared `[project.paths]` entries, validating
	/// anchor names and values.
	fn new(
		discovery_dir: PathBuf,
		variables: BTreeMap<String, String>,
	) -> Result<Self, ProjectError> {
		for (name, value) in &variables {
			if name == BUILTIN_ANCHOR {
				return Err(ProjectError::InvalidPathsEntry {
					name: name.clone(),
					reason: format!(
						"`{BUILTIN_ANCHOR}` is reserved by the `{{{{{BUILTIN_ANCHOR}}}}}` \
						 repository anchor"
					),
				});
			}
			if !is_valid_anchor_name(name) {
				return Err(ProjectError::InvalidPathsEntry {
					name: name.clone(),
					reason: "anchor names must match [A-Za-z_][A-Za-z0-9_-]*".to_owned(),
				});
			}

			if value.trim().is_empty() {
				return Err(ProjectError::InvalidPathsEntry {
					name: name.clone(),
					reason: "anchor values must be non-empty".to_owned(),
				});
			}

			if has_root_or_prefix(Path::new(value)) {
				return Err(ProjectError::InvalidPathsEntry {
					name: name.clone(),
					reason: "anchor values must be relative or anchored with `{{root}}`".to_owned(),
				});
			}

			let anchors = extract_anchors(value).map_err(|reason| {
				ProjectError::InvalidPathsEntry {
					name: name.clone(),
					reason: reason.to_owned(),
				}
			})?;
			for anchor in anchors {
				if anchor != BUILTIN_ANCHOR {
					return Err(ProjectError::InvalidPathsEntry {
						name: name.clone(),
						reason: format!(
							"path anchors may only reference `{{{{{BUILTIN_ANCHOR}}}}}`"
						),
					});
				}
			}
		}

		Ok(Self {
			discovery_dir,
			variables,
			git_root: std::cell::RefCell::new(None),
		})
	}

	/// Build a resolver without declared anchors; used when no `pina.toml`
	/// exists and every path is a Pina default.
	fn empty(discovery_dir: PathBuf) -> Self {
		Self {
			discovery_dir,
			variables: BTreeMap::new(),
			git_root: std::cell::RefCell::new(None),
		}
	}

	fn known_anchors(&self) -> String {
		let mut names = Vec::with_capacity(self.variables.len() + 1);
		names.push(format!("`{BUILTIN_ANCHOR}`"));
		names.extend(self.variables.keys().map(|name| format!("`{name}`")));
		names.join(", ")
	}

	/// Expand every `{{ name }}` anchor in `raw`.
	fn substitute(&self, field: &'static str, raw: &str) -> Result<String, ProjectError> {
		self.expand(field, raw, true)
	}

	/// Expand anchors in `raw`; declared variables resolve to one nested
	/// expansion pass because their values may only reference `{{root}}`.
	fn expand(
		&self,
		field: &'static str,
		raw: &str,
		allow_variables: bool,
	) -> Result<String, ProjectError> {
		let mut result = String::with_capacity(raw.len());
		let mut rest = raw;

		while let Some(start) = rest.find("{{") {
			result.push_str(&rest[..start]);
			let after = &rest[start + 2..];
			let Some(end) = after.find("}}") else {
				return Err(ProjectError::InvalidConfigPath {
					field,
					path: PathBuf::from(raw),
					reason: "found `{{` without a closing `}}`",
				});
			};
			let name = after[..end].trim();
			if !is_valid_anchor_name(name) {
				return Err(ProjectError::InvalidConfigPath {
					field,
					path: PathBuf::from(raw),
					reason: "anchor names must match [A-Za-z_][A-Za-z0-9_-]*",
				});
			}

			if name == BUILTIN_ANCHOR {
				result.push_str(&self.git_root(field, raw)?.to_string_lossy());
			} else {
				let Some(value) = allow_variables.then(|| self.variables.get(name)).flatten()
				else {
					return Err(ProjectError::UnknownTemplateAnchor {
						field,
						variable: name.to_owned(),
						known: self.known_anchors(),
					});
				};
				result.push_str(&self.expand(field, value, false)?);
			}
			rest = &after[end + 2..];
		}

		result.push_str(rest);
		Ok(result)
	}

	/// Return the cached git repository root, discovering it on first use.
	fn git_root(&self, field: &'static str, raw: &str) -> Result<PathBuf, ProjectError> {
		self.discover_git_root().ok_or_else(|| {
			ProjectError::InvalidConfigPath {
				field,
				path: PathBuf::from(raw),
				reason: "the `{{root}}` anchor needs a git repository; run inside a repository or \
				         add a `.git` entry to an ancestor directory",
			}
		})
	}

	fn discover_git_root(&self) -> Option<PathBuf> {
		if let Some(root) = self.git_root.borrow().as_ref() {
			return Some(root.clone());
		}

		let discovered = discover_git_root(&self.discovery_dir)?;
		*self.git_root.borrow_mut() = Some(discovered.clone());
		Some(discovered)
	}

	/// Resolve a directory-valued path field to a normalized absolute path.
	///
	/// `raw` is relative to the configuration directory unless an anchor makes
	/// it absolute.
	fn resolve_dir(
		&self,
		base: &Path,
		field: &'static str,
		raw: &str,
	) -> Result<PathBuf, ProjectError> {
		let (_, joined, _) = self.resolve_raw(base, field, raw)?;
		let normalized = normalize_absolute(&joined, field, raw)?;
		self.reject_symlink_prefix(&normalized, field, raw)?;
		Ok(normalized)
	}

	/// Resolve a per-target client output path.
	///
	/// Relative results stay relative so generation joins them beneath the
	/// shared clients directory; anchored results are absolute.
	fn resolve_output(
		&self,
		base: &Path,
		field: &'static str,
		raw: &str,
	) -> Result<PathBuf, ProjectError> {
		let (substituted, joined, anchored) = self.resolve_raw(base, field, raw)?;
		let normalized = normalize_absolute(&joined, field, raw)?;
		self.reject_symlink_prefix(&normalized, field, raw)?;

		if anchored {
			Ok(normalized)
		} else {
			Ok(normalize_relative(&substituted))
		}
	}

	/// Substitute anchors and join the result onto `base` unless it is
	/// already absolute.
	///
	/// Returns the substituted path and the path joined onto `base`, plus
	/// whether substitution produced an absolute path.
	fn resolve_raw(
		&self,
		base: &Path,
		field: &'static str,
		raw: &str,
	) -> Result<(PathBuf, PathBuf, bool), ProjectError> {
		if raw.trim().is_empty() {
			return Err(ProjectError::InvalidConfigPath {
				field,
				path: PathBuf::from(raw),
				reason: "paths must be non-empty",
			});
		}
		if has_root_or_prefix(Path::new(raw)) {
			return Err(ProjectError::InvalidConfigPath {
				field,
				path: PathBuf::from(raw),
				reason: "paths must be relative or anchored with `{{root}}`",
			});
		}

		let substituted = self.substitute(field, raw)?;
		let substituted_path = PathBuf::from(&substituted);
		let anchored = substituted_path.is_absolute();
		let joined = if anchored {
			substituted_path.clone()
		} else {
			base.join(&substituted_path)
		};
		Ok((substituted_path, joined, anchored))
	}

	/// Reject symlinked components between the trusted walk base and the
	/// normalized path. Paths that leave the walk base lexically (`..`) are
	/// checked only up to that point; generation-time validation still
	/// refuses symlinked destinations.
	fn reject_symlink_prefix(
		&self,
		normalized: &Path,
		field: &'static str,
		raw: &str,
	) -> Result<(), ProjectError> {
		let walk_base = if self.git_root_is_known() {
			// Anchored paths are walked from the repository root they name;
			// the repository root itself is allowed to be reached through
			// links (macOS temp dirs, worktree indirection).
			self.git_root(field, raw)?
		} else {
			self.discovery_dir.clone()
		};

		let Ok(remainder) = normalized.strip_prefix(&walk_base) else {
			return Ok(());
		};

		let mut current = walk_base;
		for component in remainder.components() {
			current.push(component);
			match std::fs::symlink_metadata(&current) {
				Ok(metadata) if is_link_like(&metadata) => {
					return Err(ProjectError::InvalidConfigPath {
						field,
						path: PathBuf::from(raw),
						reason: "configured paths cannot traverse symbolic links",
					});
				}
				Ok(_) => {}
				Err(source) if source.kind() == std::io::ErrorKind::NotFound => break,
				Err(source) => {
					return Err(ProjectError::InspectPath {
						path: current,
						source,
					});
				}
			}
		}

		Ok(())
	}

	fn git_root_is_known(&self) -> bool {
		self.git_root.borrow().is_some()
	}
}

/// Return whether a configured path starts with a filesystem root or a
/// platform prefix such as a Windows drive or UNC share.
fn has_root_or_prefix(path: &Path) -> bool {
	matches!(
		path.components().next(),
		Some(Component::Prefix(_) | Component::RootDir)
	)
}

/// Return whether `name` can be used as a `{{ name }}` anchor.
fn is_valid_anchor_name(name: &str) -> bool {
	let mut characters = name.chars();
	let valid_start = characters
		.next()
		.is_some_and(|first| first.is_ascii_alphabetic() || first == '_');

	valid_start && characters.all(|rest| rest.is_ascii_alphanumeric() || rest == '_' || rest == '-')
}

/// Extract every `{{ name }}` anchor from a template value.
///
/// Returns an error message when a `{{` is never closed.
fn extract_anchors(raw: &str) -> Result<Vec<&str>, &'static str> {
	let mut anchors = Vec::new();
	let mut rest = raw;

	while let Some(start) = rest.find("{{") {
		let after = &rest[start + 2..];
		let Some(end) = after.find("}}") else {
			return Err("found `{{` without a closing `}}`");
		};
		anchors.push(after[..end].trim());
		rest = &after[end + 2..];
	}

	Ok(anchors)
}

/// Find the git repository root for `start`.
///
/// The nearest ancestor containing a `.git` entry owns the working tree:
/// a directory for repositories, a file for linked worktrees and submodules.
/// Discovery must not launch Git: this root is also used to remove untrusted
/// workspace executables from `PATH` before spawning package runners.
fn discover_git_root(start: &Path) -> Option<PathBuf> {
	let mut current = Some(start);
	while let Some(directory) = current {
		if directory.join(".git").symlink_metadata().is_ok() {
			return Some(directory.to_path_buf());
		}
		current = directory.parent();
	}

	None
}

/// Resolve build and IDL destinations before any output directories are created.
fn resolve_build_output_dirs_from_paths(
	templates: &PathTemplates,
	workspace_root: &Path,
	target: &Path,
	metadata_cwd: &Path,
	configured_idl_dir: Option<PathBuf>,
) -> Result<(PathBuf, PathBuf), ProjectError> {
	let output_root = output_containment_root(
		&templates.discovery_dir,
		Some(workspace_root),
		templates.discover_git_root(),
	);
	let normalized_target =
		normalize_absolute(target, "target_directory", &target.to_string_lossy())?;
	let normalized_root = normalize_absolute(
		&output_root,
		"target_directory",
		&output_root.to_string_lossy(),
	)?;
	let contained_target = strip_output_root(&normalized_root, &normalized_target).is_some();
	// The invoking environment and operator Cargo configuration outside the
	// repository are operator choices. Repository Cargo configuration inside
	// the tree cannot grant itself permission to write outside it.
	let operator_target = if contained_target {
		false
	} else if has_operator_target_env() {
		true
	} else {
		let normalized_cwd = normalize_absolute(
			metadata_cwd,
			"target_directory",
			&metadata_cwd.to_string_lossy(),
		)?;
		let home = current_user_home();
		let (trust_root, allow_ancestor_configs) =
			cargo_config_trust_boundary(&normalized_cwd, &normalized_root, home.as_deref());
		let cargo_home = resolve_cargo_home(&normalized_cwd);
		operator_cargo_config_target(
			&normalized_cwd,
			&trust_root,
			allow_ancestor_configs,
			cargo_home.as_deref(),
		)
		.is_some_and(|configured| paths_match(&configured, &normalized_target))
	};
	let target_root = if operator_target {
		target
	} else {
		&output_root
	};
	let target_dir = resolve_cargo_target_dir(target_root, target)?;
	let idl_dir = match configured_idl_dir {
		Some(path) => resolve_output_dir(&output_root, "project.idl_dir", &path)?,
		None => resolve_output_dir(&target_dir, "project.idl_dir", &target_dir.join("idl"))?,
	};

	Ok((target_dir, idl_dir))
}

struct FastPackageMetadata {
	program_dir: PathBuf,
	package_name: String,
	library_name: String,
	library_source: PathBuf,
	library_crate_types: Vec<String>,
	workspace_root: PathBuf,
	target_directory: PathBuf,
}

#[derive(Deserialize)]
struct FastManifest {
	package: Option<FastPackage>,
	lib: Option<FastLib>,
	workspace: Option<FastWorkspace>,
}

#[derive(Deserialize)]
struct FastPackage {
	name: String,
	workspace: Option<toml::Value>,
	autolib: Option<bool>,
}

#[derive(Deserialize)]
struct FastLib {
	name: Option<String>,
	path: Option<String>,
	#[serde(rename = "crate-type", alias = "crate_type")]
	crate_type: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct FastWorkspaceManifest {
	package: Option<FastWorkspacePackage>,
	workspace: Option<FastWorkspace>,
}

#[derive(Deserialize)]
struct FastWorkspacePackage {
	workspace: Option<String>,
}

#[derive(Deserialize)]
struct FastWorkspace {
	#[serde(default)]
	members: Vec<String>,
	#[serde(default)]
	exclude: Vec<String>,
}

fn find_ancestor_manifest(start: &Path) -> Option<PathBuf> {
	let mut current = Some(start);
	while let Some(dir) = current {
		let candidate = dir.join("Cargo.toml");
		if candidate.is_file() {
			return Some(candidate);
		}
		current = dir.parent();
	}
	None
}

fn is_custom_cargo_binary(cargo: Option<&std::ffi::OsStr>) -> bool {
	let Some(cargo) = cargo.filter(|value| !value.is_empty()) else {
		return false;
	};
	!matches!(
		Path::new(cargo).file_name().and_then(|name| name.to_str()),
		Some("cargo" | "cargo.exe")
	)
}

fn uses_custom_cargo_wrapper() -> bool {
	is_custom_cargo_binary(std::env::var_os("CARGO").as_deref())
}

fn try_discover_package_fast(
	start_dir: &Path,
	manifest_path: &Path,
) -> Result<Option<FastPackageMetadata>, ProjectError> {
	if uses_custom_cargo_wrapper() {
		return Ok(None);
	}
	let Some(program_dir) = manifest_path.parent() else {
		return Ok(None);
	};
	let Ok(raw) = std::fs::read_to_string(manifest_path) else {
		return Ok(None);
	};
	let Ok(manifest) = toml::from_str::<FastManifest>(&raw) else {
		return Ok(None);
	};
	let Some(package) = manifest.package else {
		return Ok(None);
	};
	if package.workspace.is_some() || (manifest.lib.is_none() && package.autolib == Some(false)) {
		return Ok(None);
	}

	let default_lib_name = || package.name.replace('-', "_");
	let (library_name, library_source, library_crate_types) = match manifest.lib {
		Some(lib) => {
			let crate_types = lib.crate_type.unwrap_or_else(|| vec!["lib".to_owned()]);
			if !crate_types.iter().any(|t| t == "lib" || t == "cdylib") {
				return Ok(None);
			}
			(
				lib.name.unwrap_or_else(default_lib_name),
				program_dir.join(lib.path.as_deref().unwrap_or("src/lib.rs")),
				crate_types,
			)
		}
		None => {
			(
				default_lib_name(),
				program_dir.join("src/lib.rs"),
				vec!["lib".to_owned()],
			)
		}
	};

	if !library_source.is_file() {
		return Ok(None);
	}
	validate_library_name(&library_name)?;

	let Some(workspace_root) =
		discover_workspace_root_from_flag(program_dir, manifest.workspace.is_some())
	else {
		return Ok(None);
	};
	if has_cargo_config_target_dir(start_dir) {
		return Ok(None);
	}

	let target_override = ["CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR"]
		.into_iter()
		.find_map(|key| std::env::var_os(key).filter(|val| !val.is_empty()));
	let target_directory = resolve_target_directory(start_dir, &workspace_root, target_override);

	Ok(Some(FastPackageMetadata {
		program_dir: program_dir.to_path_buf(),
		package_name: package.name,
		library_name,
		library_source,
		library_crate_types,
		workspace_root,
		target_directory,
	}))
}

fn resolve_target_directory(
	start_dir: &Path,
	workspace_root: &Path,
	override_dir: Option<OsString>,
) -> PathBuf {
	match override_dir {
		Some(override_dir) => {
			let path = PathBuf::from(override_dir);
			if path.is_absolute() {
				path
			} else {
				start_dir.join(path)
			}
		}
		None => workspace_root.join("target"),
	}
}

fn discover_workspace_root_from_manifest(
	program_dir: &Path,
	manifest_path: &Path,
) -> Option<PathBuf> {
	if uses_custom_cargo_wrapper() {
		return None;
	}
	let raw = std::fs::read_to_string(manifest_path).ok()?;
	let manifest = toml::from_str::<FastWorkspaceManifest>(&raw).ok()?;
	if manifest
		.package
		.as_ref()
		.and_then(|pkg| pkg.workspace.as_deref())
		.is_some()
	{
		return None;
	}
	discover_workspace_root_from_flag(program_dir, manifest.workspace.is_some())
}

fn discover_workspace_root_from_flag(
	program_dir: &Path,
	has_own_workspace: bool,
) -> Option<PathBuf> {
	if has_own_workspace {
		return Some(program_dir.to_path_buf());
	}

	let mut ancestor = program_dir.parent();
	while let Some(dir) = ancestor {
		let candidate = dir.join("Cargo.toml");
		if candidate.is_file() {
			let raw = std::fs::read_to_string(&candidate).ok()?;
			let parsed = toml::from_str::<FastWorkspaceManifest>(&raw).ok()?;
			if let Some(ws) = parsed.workspace {
				let rel = program_dir.strip_prefix(dir).ok()?;
				let rel_posix = rel
					.components()
					.map(|c| c.as_os_str().to_string_lossy())
					.collect::<Vec<_>>()
					.join("/");
				for pat in &ws.exclude {
					match workspace_pattern_matches(pat, &rel_posix) {
						Some(true) => return Some(program_dir.to_path_buf()),
						Some(false) => {}
						None => return None,
					}
				}
				for pat in &ws.members {
					if workspace_pattern_matches(pat, &rel_posix) == Some(true) {
						return Some(dir.to_path_buf());
					}
				}
				return None;
			}
		}
		ancestor = dir.parent();
	}

	Some(program_dir.to_path_buf())
}

/// Matches simple Cargo workspace member/exclude patterns (`exact/path` or
/// single-segment `prefix/*`) without pulling in a glob crate. Returns `None`
/// for complex wildcards so the caller falls back to `cargo metadata`.
fn workspace_pattern_matches(pattern: &str, rel_posix: &str) -> Option<bool> {
	let pat = pattern.trim_end_matches('/');
	if !pat.contains(['*', '?', '[']) {
		return Some(pat == rel_posix);
	}
	if let Some(prefix) = pat.strip_suffix("/*")
		&& !prefix.contains(['*', '?', '['])
	{
		let Some(rest) = rel_posix.strip_prefix(prefix) else {
			return Some(false);
		};
		let Some(segment) = rest.strip_prefix('/') else {
			return Some(false);
		};
		return Some(!segment.is_empty() && !segment.contains('/'));
	}
	None
}

fn cargo_config_dir_needs_metadata(config_dir: &Path) -> bool {
	let Ok(dir_meta) = config_dir.symlink_metadata() else {
		return false;
	};
	if is_link_like(&dir_meta) {
		return true;
	}
	["config", "config.toml"]
		.into_iter()
		.any(|name| config_file_needs_metadata(&config_dir.join(name)))
}

fn config_file_needs_metadata(path: &Path) -> bool {
	let Ok(meta) = path.symlink_metadata() else {
		return false;
	};
	if !is_plain_file(&meta) {
		return true;
	}
	match std::fs::read_to_string(path) {
		Ok(text) => {
			text.contains("target-dir") || text.contains("target_dir") || text.contains("include")
		}
		Err(_) => true,
	}
}

fn current_user_home() -> Option<PathBuf> {
	["HOME", "USERPROFILE"]
		.into_iter()
		.find_map(|name| std::env::var_os(name).filter(|value| !value.is_empty()))
		.map(PathBuf::from)
}

fn has_cargo_config_target_dir_with_home(start_dir: &Path, cargo_home: Option<&Path>) -> bool {
	start_dir
		.ancestors()
		.any(|dir| cargo_config_dir_needs_metadata(&dir.join(".cargo")))
		|| cargo_home.is_some_and(cargo_config_dir_needs_metadata)
}

fn has_cargo_config_target_dir(start_dir: &Path) -> bool {
	let cargo_home = resolve_cargo_home(start_dir);
	has_cargo_config_target_dir_with_home(start_dir, cargo_home.as_deref())
}

fn has_operator_target_env() -> bool {
	["CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR"]
		.into_iter()
		.any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()))
}

fn resolve_cargo_home(metadata_cwd: &Path) -> Option<PathBuf> {
	resolve_cargo_home_from(
		metadata_cwd,
		std::env::var_os("CARGO_HOME"),
		current_user_home().map(PathBuf::into_os_string),
	)
}

fn resolve_cargo_home_from(
	metadata_cwd: &Path,
	cargo_home: Option<OsString>,
	user_home: Option<OsString>,
) -> Option<PathBuf> {
	let path = if let Some(home) = cargo_home.filter(|value| !value.is_empty()) {
		let home = PathBuf::from(home);
		if home.is_absolute() {
			home
		} else {
			metadata_cwd.join(home)
		}
	} else {
		PathBuf::from(user_home.filter(|value| !value.is_empty())?).join(".cargo")
	};
	normalize_absolute(&path, "target_directory", "").ok()
}

#[derive(Debug, PartialEq, Eq)]
enum CargoConfigTarget {
	None,
	Configured(PathBuf),
	Untrusted,
}

/// Inspect one Cargo configuration directory for a `build.target-dir` setting.
///
/// Inside the repository (`in_tree == true`), any `build.target-dir`,
/// `include`, symbolic link, or unreadable/malformed config fails closed so
/// repository-controlled Cargo configuration cannot bypass containment.
/// Outside the repository (`in_tree == false`), Cargo checks `config` before
/// `config.toml` and ignores `config.toml` when `config` exists; a
/// `build.target-dir` setting represents an operator-level configuration (such
/// as a shared worktree parent `.cargo/config.toml` or `$CARGO_HOME/config.toml`).
fn read_cargo_config_target(config_dir: &Path, in_tree: bool) -> CargoConfigTarget {
	let Ok(dir_meta) = config_dir.symlink_metadata() else {
		return CargoConfigTarget::None;
	};
	if in_tree && is_link_like(&dir_meta) {
		return CargoConfigTarget::Untrusted;
	}

	for filename in ["config", "config.toml"] {
		let config_path = config_dir.join(filename);
		let Ok(meta) = config_path.symlink_metadata() else {
			continue;
		};
		if in_tree {
			if !is_plain_file(&meta) {
				return CargoConfigTarget::Untrusted;
			}
		} else if !config_path.is_file() {
			return CargoConfigTarget::Untrusted;
		}

		let Some(table) = std::fs::read_to_string(&config_path)
			.ok()
			.and_then(|source| toml::from_str::<toml::Table>(&source).ok())
		else {
			return CargoConfigTarget::Untrusted;
		};
		if in_tree && table.contains_key("include") {
			return CargoConfigTarget::Untrusted;
		}
		if let Some(target_dir) = table
			.get("build")
			.and_then(toml::Value::as_table)
			.and_then(|build| build.get("target-dir"))
		{
			if in_tree {
				return CargoConfigTarget::Untrusted;
			}
			let Some(raw) = target_dir.as_str().filter(|value| !value.is_empty()) else {
				return CargoConfigTarget::Untrusted;
			};
			let base = config_dir.parent().unwrap_or(config_dir);
			let resolved = if Path::new(raw).is_absolute() {
				PathBuf::from(raw)
			} else {
				base.join(raw)
			};
			return CargoConfigTarget::Configured(resolved);
		}
		if !in_tree {
			return CargoConfigTarget::None;
		}
	}

	CargoConfigTarget::None
}

/// Compute the trust boundary separating repository-controlled Cargo configs
/// from operator-level parent configs.
///
/// Any enclosing non-home Git repository (for submodules or vendored repos) or
/// enclosing non-home `Cargo.toml` workspace/package keeps its configs inside
/// the untrusted tree. Ancestor `.cargo/config[.toml]` directories strictly
/// above the boundary are accepted only when an enclosing non-home Git
/// repository defines that boundary; `$CARGO_HOME` remains accepted everywhere
/// as long as it lies outside the trust root.
fn cargo_config_trust_boundary(
	metadata_cwd: &Path,
	output_root: &Path,
	home: Option<&Path>,
) -> (PathBuf, bool) {
	let mut trust_root = output_root.to_path_buf();
	let mut has_git_boundary = false;

	for directory in metadata_cwd.ancestors() {
		if home.is_some_and(|home| is_within_root(directory, home)) {
			break;
		}
		let has_git = directory.join(".git").symlink_metadata().is_ok();
		if has_git {
			has_git_boundary = true;
		}
		if (has_git || directory.join("Cargo.toml").symlink_metadata().is_ok())
			&& strip_output_root(directory, &trust_root).is_some()
		{
			trust_root = directory.to_path_buf();
		}
	}

	(trust_root, has_git_boundary)
}

/// Return whether `path` sits at or below `root`, checking both normalized
/// lexical prefixes and canonicalized targets so symlinks, platform aliases
/// (such as macOS `/tmp` -> `/private/tmp`), and case differences cannot place
/// an in-tree directory outside `root`.
fn is_within_root(root: &Path, path: &Path) -> bool {
	if strip_output_root(root, path).is_some() {
		return true;
	}

	matches!(
		(std::fs::canonicalize(root), std::fs::canonicalize(path)),
		(Ok(root), Ok(path)) if strip_output_root(&root, &path).is_some()
	)
}

/// Find the winning operator-level `build.target-dir` outside `trust_root`.
///
/// Mirrors Cargo's config discovery order: ancestors of `metadata_cwd` up to
/// the filesystem root (only above `trust_root` when `allow_ancestor_configs`
/// is true), then `cargo_home`. Any in-tree override or untrusted config stops
/// discovery immediately and returns `None`.
fn operator_cargo_config_target(
	metadata_cwd: &Path,
	trust_root: &Path,
	allow_ancestor_configs: bool,
	cargo_home: Option<&Path>,
) -> Option<PathBuf> {
	for directory in metadata_cwd.ancestors() {
		let in_tree = strip_output_root(trust_root, directory).is_some();
		if !in_tree && !allow_ancestor_configs {
			break;
		}
		let config_dir = directory.join(".cargo");
		match read_cargo_config_target(&config_dir, in_tree) {
			CargoConfigTarget::None => {}
			CargoConfigTarget::Configured(path) => return Some(path),
			CargoConfigTarget::Untrusted => return None,
		}
	}

	let cargo_home = cargo_home?;
	if is_within_root(trust_root, cargo_home) {
		return None;
	}

	match read_cargo_config_target(cargo_home, false) {
		CargoConfigTarget::Configured(path) => Some(path),
		CargoConfigTarget::None | CargoConfigTarget::Untrusted => None,
	}
}

fn paths_match(configured: &Path, normalized_target: &Path) -> bool {
	let Ok(configured) = normalize_absolute(configured, "target_directory", "") else {
		return false;
	};
	if strip_output_root(&configured, normalized_target)
		.is_some_and(|rest| rest.as_os_str().is_empty())
	{
		return true;
	}
	match (
		std::fs::canonicalize(&configured),
		std::fs::canonicalize(normalized_target),
	) {
		(Ok(left), Ok(right)) => left == right,
		_ => false,
	}
}

/// Normalize an output and require containment beneath its trusted root.
///
/// Existing components below the root cannot be links, so lexical containment
/// also holds on disk, including when the destination has a nonexistent suffix.
/// The root itself may use platform aliases such as macOS's `/var` link.
pub(crate) fn resolve_output_dir(
	root: &Path,
	field: &'static str,
	path: &Path,
) -> Result<PathBuf, ProjectError> {
	let raw = path.to_string_lossy();
	let normalized = normalize_within(root, field, path)?.ok_or_else(|| {
		ProjectError::InvalidConfigPath {
			field,
			path: path.to_path_buf(),
			reason: "configured outputs must stay inside the Git worktree or Cargo workspace",
		}
	})?;

	PathTemplates::empty(root.to_path_buf()).reject_symlink_prefix(&normalized, field, &raw)?;
	Ok(normalized)
}

/// Resolve the target directory Cargo reports.
///
/// Cargo owns this path, and a shared build cache commonly redirects `target/`
/// through a symbolic link (mr boxington, CI caches, tmpfs). The path must
/// still be lexically contained by `root`, so repository Cargo configuration
/// cannot name a destination outside the tree, but links along it are Cargo's
/// choice and are accepted. An operator who wants an external directory sets
/// `CARGO_TARGET_DIR` explicitly.
fn resolve_cargo_target_dir(root: &Path, path: &Path) -> Result<PathBuf, ProjectError> {
	normalize_within(root, "target_directory", path)?.ok_or_else(|| {
		ProjectError::InvalidConfigPath {
			field: "target_directory",
			path: path.to_path_buf(),
			reason: "Cargo's target directory must stay inside the Git worktree or Cargo workspace; \
			         set CARGO_TARGET_DIR to build in an external directory",
		}
	})
}

/// Normalize a path and join it beneath `root` when it stays inside it.
///
/// Returns `None` when the normalized path leaves `root`, so each caller can
/// describe where the rejected path came from.
fn normalize_within(
	root: &Path,
	field: &'static str,
	path: &Path,
) -> Result<Option<PathBuf>, ProjectError> {
	let normalized = normalize_absolute(path, field, &path.to_string_lossy())?;
	let root = normalize_absolute(root, field, &root.to_string_lossy())?;

	Ok(strip_output_root(&root, &normalized).map(|relative| root.join(relative)))
}

fn strip_output_root<'a>(root: &Path, path: &'a Path) -> Option<&'a Path> {
	let mut components = path.components();

	for root_component in root.components() {
		if !output_components_equal(&root_component, &components.next()?) {
			return None;
		}
	}

	Some(components.as_path())
}

fn output_components_equal(left: &Component<'_>, right: &Component<'_>) -> bool {
	#[cfg(windows)]
	if let (Component::Prefix(left), Component::Prefix(right)) = (left, right) {
		return output_prefixes_equal(left.kind(), right.kind());
	}

	left == right
}

/// Cargo's canonical paths use verbatim prefixes; Git reports ordinary ones.
#[cfg(any(windows, test))]
fn output_prefixes_equal(left: std::path::Prefix<'_>, right: std::path::Prefix<'_>) -> bool {
	use std::path::Prefix;

	match (left, right) {
		(
			Prefix::Disk(left) | Prefix::VerbatimDisk(left),
			Prefix::Disk(right) | Prefix::VerbatimDisk(right),
		) => left.eq_ignore_ascii_case(&right),
		(
			Prefix::UNC(left_server, left_share) | Prefix::VerbatimUNC(left_server, left_share),
			Prefix::UNC(right_server, right_share) | Prefix::VerbatimUNC(right_server, right_share),
		) => {
			left_server.eq_ignore_ascii_case(right_server)
				&& left_share.eq_ignore_ascii_case(right_share)
		}
		_ => left == right,
	}
}

/// The root that configured outputs must stay within.
///
/// A `pina.toml` file ships with the repository, so its outputs are
/// repository data: they may write anywhere inside the repository's own
/// working tree, and an operator picks a destination outside it with the
/// `--output` flag. The root is the Git working tree that owns
/// `project_root` — the same boundary the `{{root}}` anchor resolves to —
/// falling back to `project_root` itself outside a repository. A discovered
/// root that also contains the user's home directory (a `git init ~`
/// configuration) is rejected in favor of `project_root`, because
/// containment to `$HOME` would contain nothing.
pub(crate) fn generation_containment_root(project_root: &Path) -> PathBuf {
	output_containment_root(project_root, None, discover_git_root(project_root))
}

fn output_containment_root(
	project_root: &Path,
	cargo_workspace: Option<&Path>,
	git_root: Option<PathBuf>,
) -> PathBuf {
	let home = current_user_home();
	let workspace = cargo_workspace
		.filter(|workspace| project_root.starts_with(workspace))
		.map(Path::to_path_buf);

	containment_root(git_root, workspace, project_root, home.as_deref())
}

fn containment_root(
	git_root: Option<PathBuf>,
	cargo_workspace: Option<PathBuf>,
	project_root: &Path,
	home: Option<&Path>,
) -> PathBuf {
	for candidate in [git_root, cargo_workspace].into_iter().flatten() {
		if !home.is_some_and(|home| is_within_root(&candidate, home)) {
			return candidate;
		}
	}

	project_root.to_path_buf()
}

pub(crate) fn sanitize_git_environment(command: &mut Command) {
	for variable in [
		"GIT_ALTERNATE_OBJECT_DIRECTORIES",
		"GIT_COMMON_DIR",
		"GIT_CONFIG",
		"GIT_CONFIG_COUNT",
		"GIT_CONFIG_PARAMETERS",
		"GIT_DIR",
		"GIT_GRAFT_FILE",
		"GIT_IMPLICIT_WORK_TREE",
		"GIT_INDEX_FILE",
		"GIT_INTERNAL_SUPER_PREFIX",
		"GIT_NO_REPLACE_OBJECTS",
		"GIT_OBJECT_DIRECTORY",
		"GIT_PREFIX",
		"GIT_REPLACE_REF_BASE",
		"GIT_SHALLOW_FILE",
		"GIT_WORK_TREE",
	] {
		command.env_remove(variable);
	}
	for (variable, _) in std::env::vars_os() {
		if variable.to_str().is_some_and(|variable| {
			variable.starts_with("GIT_CONFIG_KEY_") || variable.starts_with("GIT_CONFIG_VALUE_")
		}) {
			command.env_remove(variable);
		}
	}
}

/// Collapse `.`, resolve `..`, and keep the path absolute.
///
/// Returns an error when `..` climbs above the filesystem root.
fn normalize_absolute(
	path: &Path,
	field: &'static str,
	raw: &str,
) -> Result<PathBuf, ProjectError> {
	let mut normalized = PathBuf::new();
	let mut floor = 0usize;

	for component in path.components() {
		match component {
			component @ (Component::Prefix(_) | Component::RootDir | Component::CurDir) => {
				normalized.push(component);
				floor = normalized.components().count();
			}
			Component::Normal(part) => normalized.push(part),
			Component::ParentDir => {
				if normalized.components().count() <= floor || !normalized.pop() {
					return Err(ProjectError::InvalidConfigPath {
						field,
						path: PathBuf::from(raw),
						reason: "the resolved path traverses above the filesystem root",
					});
				}
			}
		}
	}

	Ok(normalized)
}

/// Collapse `.` and resolve `..` in a relative path, keeping leading `..`
/// components so the result stays relative.
fn normalize_relative(path: &Path) -> PathBuf {
	enum Part {
		Parent,
		Name(OsString),
	}

	let mut parts: Vec<Part> = Vec::new();
	for component in path.components() {
		match component {
			Component::CurDir | Component::Prefix(_) | Component::RootDir => {}
			Component::ParentDir => {
				match parts.last() {
					Some(Part::Name(_)) => {
						parts.pop();
					}
					Some(Part::Parent) | None => parts.push(Part::Parent),
				}
			}
			Component::Normal(name) => parts.push(Part::Name(name.to_os_string())),
		}
	}

	let mut normalized = PathBuf::new();
	for part in parts {
		match part {
			Part::Parent => normalized.push(".."),
			Part::Name(name) => normalized.push(name),
		}
	}
	if normalized.as_os_str().is_empty() {
		normalized.push(".");
	}
	normalized
}

/// Validate the `[lints]` table and return the resolved level overrides.
fn resolve_lint_levels(
	entries: BTreeMap<String, String>,
) -> Result<BTreeMap<String, LintLevel>, ProjectError> {
	let catalog = LintCatalog::global();
	let mut levels = BTreeMap::new();

	for (name, level) in entries {
		if !catalog.contains(&name) {
			return Err(ProjectError::UnknownLint {
				known: catalog.known_lints(),
				name,
			});
		}

		let level = match level.as_str() {
			"allow" => LintLevel::Allow,
			"warn" => LintLevel::Warn,
			"deny" => LintLevel::Deny,
			_ => return Err(ProjectError::InvalidLintLevel { level, name }),
		};
		levels.insert(name, level);
	}
	Ok(levels)
}

#[cfg(any(windows, test))]
const WINDOWS_REPARSE_POINT_ATTRIBUTE: u32 = 0x0400;

#[cfg(any(windows, test))]
const fn is_windows_link_like(is_symlink: bool, file_attributes: u32) -> bool {
	is_symlink || (file_attributes & WINDOWS_REPARSE_POINT_ATTRIBUTE) != 0
}

#[cfg(any(windows, test))]
const fn is_windows_plain_file(is_file: bool, is_symlink: bool, file_attributes: u32) -> bool {
	is_file && !is_windows_link_like(is_symlink, file_attributes)
}

fn is_link_like(metadata: &std::fs::Metadata) -> bool {
	#[cfg(windows)]
	{
		use std::os::windows::fs::MetadataExt;

		is_windows_link_like(
			metadata.file_type().is_symlink(),
			metadata.file_attributes(),
		)
	}

	#[cfg(not(windows))]
	metadata.file_type().is_symlink()
}

fn is_plain_file(metadata: &std::fs::Metadata) -> bool {
	#[cfg(windows)]
	{
		use std::os::windows::fs::MetadataExt;

		is_windows_plain_file(
			metadata.is_file(),
			metadata.file_type().is_symlink(),
			metadata.file_attributes(),
		)
	}

	#[cfg(not(windows))]
	metadata.is_file()
}

fn package_for_manifest<'a>(
	metadata: &'a Metadata,
	manifest_path: &Path,
	start: &Path,
) -> Result<&'a Package, ProjectError> {
	metadata
		.packages
		.iter()
		.find(|package| package.manifest_path.as_std_path() == manifest_path)
		.ok_or_else(|| {
			ProjectError::CargoMetadata {
				path: start.to_path_buf(),
				message: format!(
					"Cargo metadata did not include configured manifest {}",
					manifest_path.display()
				),
			}
		})
}

fn library_details(package: &Package) -> Result<(String, PathBuf, Vec<String>), ProjectError> {
	let target = package
		.targets
		.iter()
		.find(|target| {
			target
				.kind
				.iter()
				.any(|kind| matches!(kind, TargetKind::Lib | TargetKind::CDyLib))
		})
		.ok_or_else(|| {
			ProjectError::MissingLibraryTarget {
				package: package.name.to_string(),
			}
		})?;

	validate_library_name(&target.name)?;

	Ok((
		target.name.clone(),
		target.src_path.as_std_path().to_path_buf(),
		target.crate_types.iter().map(ToString::to_string).collect(),
	))
}

/// Reject a Cargo library name that cannot be a path component.
///
/// Cargo accepts any string as a `[lib] name` and reports it verbatim through
/// `cargo metadata`, so a manifest controls this value. Pina joins it into
/// output paths (`target/deploy`, generated IDL files, client crate
/// directories), where a name containing a separator or `..` would escape the
/// project directory. Validating at the single point where the name enters the
/// project model keeps every current and future path join safe.
fn validate_library_name(name: &str) -> Result<(), ProjectError> {
	let valid = !name.is_empty()
		&& name.chars().all(|character| {
			character.is_ascii_alphanumeric() || character == '_' || character == '-'
		});

	if valid {
		return Ok(());
	}

	Err(ProjectError::InvalidLibraryName {
		name: name.to_string(),
	})
}

fn cargo_metadata(start: &Path, manifest_path: Option<&Path>) -> Result<Metadata, ProjectError> {
	// Some environments (devenv's rust integration) export `CARGO` as an
	// empty string; treat that like an unset variable rather than spawning a
	// nameless executable.
	let cargo = std::env::var_os("CARGO")
		.filter(|cargo| !cargo.is_empty())
		.unwrap_or_else(|| OsString::from("cargo"));
	let mut command = MetadataCommand::new();
	command.cargo_path(cargo).current_dir(start).no_deps();

	if let Some(manifest_path) = manifest_path {
		command.manifest_path(manifest_path);
	}

	command.exec().map_err(|source| {
		ProjectError::CargoMetadata {
			path: start.to_path_buf(),
			message: source.to_string(),
		}
	})
}

fn read_project_config(config_path: &Path) -> Result<ProjectConfig, ProjectError> {
	let source = std::fs::read_to_string(config_path).map_err(|source| {
		ProjectError::ReadFile {
			path: config_path.to_path_buf(),
			source,
		}
	})?;

	toml::from_str(&source).map_err(|source| {
		ProjectError::ParseToml {
			path: config_path.to_path_buf(),
			source,
		}
	})
}

/// Read the `[compute_units]` settings governing the program at `start`.
///
/// The nearest `pina.toml` at or above `start` is the configuration
/// [`Project::discover`] would use; without one every setting keeps its
/// default. Unlike discovery this never runs Cargo, so IDL generation can call
/// it on every run.
///
/// # Errors
///
/// Returns an error when `start` cannot be inspected or the configuration
/// cannot be read or parsed.
pub(crate) fn compute_units_config(start: &Path) -> Result<ComputeUnitsConfig, ProjectError> {
	let start = normalize_start(start)?;
	let Some(config_path) = find_ancestor_config(&start) else {
		return Ok(ComputeUnitsConfig::default());
	};

	Ok(read_project_config(&config_path)?.compute_units)
}

fn normalize_start(start: &Path) -> Result<PathBuf, ProjectError> {
	let absolute = std::fs::canonicalize(start).map_err(|source| {
		ProjectError::InspectPath {
			path: start.to_path_buf(),
			source,
		}
	})?;
	if absolute.is_dir() {
		return Ok(absolute);
	}

	Ok(absolute
		.parent()
		.map_or_else(|| PathBuf::from("."), Path::to_path_buf))
}

fn find_ancestor_config(start: &Path) -> Option<PathBuf> {
	start.ancestors().find_map(find_config_in_directory)
}

fn find_config_in_directory(directory: &Path) -> Option<PathBuf> {
	for entry in std::fs::read_dir(directory).ok()?.flatten() {
		let name = entry.file_name();

		if name == CONFIG_FILE_NAME && entry.path().is_file() {
			return Some(entry.path());
		}
	}

	None
}

#[cfg(test)]
mod tests {
	use std::fs;

	use tempfile::TempDir;

	use super::*;

	fn write_program(root: &Path, package_name: &str) {
		write_program_with_lib(root, package_name, None);
	}

	fn write_program_with_lib(root: &Path, package_name: &str, library_name: Option<&str>) {
		fs::create_dir_all(root.join("src"))
			.unwrap_or_else(|error| panic!("failed to create fixture: {error}"));
		let library_name =
			library_name.map_or_else(String::new, |name| format!("name = \"{name}\"\n"));
		fs::write(
			root.join("Cargo.toml"),
			format!(
				r#"[package]
name = "{package_name}"
version = "0.1.0"
edition = "2024"

[lib]
{library_name}
crate-type = ["cdylib", "lib"]
"#
			),
		)
		.unwrap_or_else(|error| panic!("failed to write manifest: {error}"));
		fs::write(root.join("src/lib.rs"), "")
			.unwrap_or_else(|error| panic!("failed to write source: {error}"));
	}

	#[test]
	fn empty_config_uses_cargo_target_and_client_defaults() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		write_program(temp.path(), "counter-program");
		fs::write(temp.path().join(CONFIG_FILE_NAME), "")
			.unwrap_or_else(|error| panic!("failed to write config: {error}"));

		let project = Project::discover(temp.path())
			.unwrap_or_else(|error| panic!("discovery failed: {error}"));
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("failed to canonicalize root: {error}"));

		assert_eq!(project.target_dir, root.join("target"));
		assert_eq!(project.idl_dir, root.join("target/idl"));
		assert_eq!(
			project.clients,
			vec![ClientLanguage::Rust, ClientLanguage::Typescript]
		);
		assert_eq!(
			project.client_generation[&ClientLanguage::Rust],
			ClientGenerationConfig {
				output: PathBuf::from("rust"),
				mode: GenerationMode::Auto,
				scaffold: true,
			}
		);
	}

	/// Write `contents` to `path`, failing the test on error.
	fn write_file(path: &Path, contents: &[u8]) {
		let written = fs::write(path, contents);
		written.unwrap_or_else(|error| panic!("failed to write {}: {error}", path.display()));
	}

	/// The `[locks]` table of the project discovered at `path`.
	fn discovered_locks(path: &Path) -> LocksConfig {
		let project = Project::discover(path);
		let project = project.unwrap_or_else(|error| panic!("discovery failed: {error}"));
		let locks = project.locks_config();
		locks.unwrap_or_else(|error| panic!("locks config failed: {error}"))
	}

	#[test]
	fn locks_config_reads_the_allow_list() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		write_program(temp.path(), "config-program");
		write_file(
			&temp.path().join(CONFIG_FILE_NAME),
			b"[locks]\nallow = [\"program_config\", \"fee_vault\"]\n",
		);

		let locks = discovered_locks(temp.path());

		assert_eq!(locks.allow, ["program_config", "fee_vault"]);
	}

	#[test]
	fn locks_config_defaults_without_a_config_file() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		write_program(temp.path(), "bare-program");

		let locks = discovered_locks(temp.path());

		assert_eq!(locks, LocksConfig::default());
	}

	#[test]
	fn locks_config_rejects_unknown_fields_and_reports_unreadable_files() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		write_program(temp.path(), "typo-program");
		let config_path = temp.path().join(CONFIG_FILE_NAME);
		write_file(&config_path, b"[locks]\nallowed = [\"state\"]\n");

		let error = Project::discover(temp.path())
			.expect_err("an unknown `[locks]` field must fail discovery");
		assert!(matches!(error, ProjectError::ParseToml { .. }), "{error}");

		write_file(&config_path, b"");
		let project = Project::discover(temp.path());
		let project = project.unwrap_or_else(|error| panic!("discovery failed: {error}"));
		write_file(&config_path, &[0xff, 0xfe]);
		let error = project
			.locks_config()
			.expect_err("a config that is not UTF-8 cannot be read");

		assert!(matches!(error, ProjectError::ReadFile { .. }), "{error}");
	}

	#[test]
	fn config_preserves_custom_library_target_name() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		write_program_with_lib(temp.path(), "hyphen-package", Some("custom_program"));
		fs::write(temp.path().join(CONFIG_FILE_NAME), "")
			.unwrap_or_else(|error| panic!("failed to write config: {error}"));

		let project = Project::discover(temp.path())
			.unwrap_or_else(|error| panic!("discovery failed: {error}"));

		assert_eq!(project.package_name, "hyphen-package");
		assert_eq!(project.library_name, "custom_program");
	}

	#[test]
	fn discovery_rejects_traversing_library_names() {
		// Cargo accepts all of these as `[lib] name` values and reports them
		// through `cargo metadata` verbatim. The empty name is absent because
		// cargo rejects it before metadata reaches pina.
		for library_name in ["../../pwned", "a/b", "a\\b", "..", "a b", "a.b"] {
			let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
			write_program_with_lib(temp.path(), "counter-program", Some(library_name));
			fs::write(temp.path().join(CONFIG_FILE_NAME), "")
				.unwrap_or_else(|error| panic!("failed to write config: {error}"));

			let error = Project::discover(temp.path())
				.expect_err("a traversal-capable library name must be rejected");

			assert!(
				matches!(error, ProjectError::InvalidLibraryName { .. }),
				"expected InvalidLibraryName for {library_name:?}, got {error}"
			);
			// Render the message too: the operator reads this text, so it must
			// state the rule that rejected the name.
			let message = error.to_string();
			assert!(
				message.contains("ASCII letters") && message.contains("cannot be used as a path"),
				"the message must state the rule that rejected the name: {message}"
			);
		}
	}

	#[test]
	fn library_name_validation_accepts_cargo_shaped_names() {
		for name in ["counter_program", "counter-program", "program2", "a"] {
			assert!(
				validate_library_name(name).is_ok(),
				"expected {name:?} to be accepted"
			);
		}
		for name in [
			"../../pwned",
			"a/b",
			"a\\b",
			"..",
			"",
			".hidden",
			"a b",
			"a.b",
		] {
			assert!(
				validate_library_name(name).is_err(),
				"expected {name:?} to be rejected"
			);
		}
	}

	#[test]
	fn config_discovery_works_from_nested_directory() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let program = temp.path().join("programs/counter");
		let nested = program.join("src/instructions");
		write_program(&program, "counter-program");
		fs::create_dir_all(&nested)
			.unwrap_or_else(|error| panic!("failed to create nested dir: {error}"));
		fs::write(
			temp.path().join(CONFIG_FILE_NAME),
			r#"[project]
program = "programs/counter"
idl_dir = "artifacts/idls"

[clients]
output = "generated"
languages = ["rust", "dart"]
mode = "update"
scaffold = false

[clients.rust]
output = "native"
scaffold = true

[clients.dart]
mode = "overwrite"
"#,
		)
		.unwrap_or_else(|error| panic!("failed to write config: {error}"));

		let project =
			Project::discover(&nested).unwrap_or_else(|error| panic!("discovery failed: {error}"));
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("failed to canonicalize root: {error}"));
		let program = root.join("programs/counter");

		assert_eq!(project.root, root);
		assert_eq!(project.program_dir, program);
		assert_eq!(project.package_name, "counter-program");
		assert_eq!(project.library_name, "counter_program");
		assert_eq!(project.target_dir, program.join("target"));
		assert_eq!(project.idl_dir, root.join("artifacts/idls"));
		assert_eq!(project.clients_dir, root.join("generated"));
		assert_eq!(
			project.clients,
			vec![ClientLanguage::Rust, ClientLanguage::Dart]
		);
		assert_eq!(
			project.client_generation[&ClientLanguage::Rust],
			ClientGenerationConfig {
				output: PathBuf::from("native"),
				mode: GenerationMode::Update,
				scaffold: true,
			}
		);
		assert_eq!(
			project.client_generation[&ClientLanguage::Dart],
			ClientGenerationConfig {
				output: PathBuf::from("dart"),
				mode: GenerationMode::Overwrite,
				scaffold: false,
			}
		);
	}

	#[test]
	fn config_rejects_unknown_keys() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		write_program(temp.path(), "counter");
		fs::write(
			temp.path().join(CONFIG_FILE_NAME),
			"[project]\nprogram = \".\"\nunknown = true\n",
		)
		.unwrap_or_else(|error| panic!("failed to write config: {error}"));

		let error = Project::discover(temp.path())
			.expect_err("unknown project config keys should fail closed");

		assert!(error.to_string().contains("unknown field"));
	}

	#[test]
	fn compute_unit_settings_default_without_a_config_and_parse_from_one()
	-> Result<(), Box<dyn std::error::Error>> {
		let temp = TempDir::new()?;
		let nested = temp.path().join("src");
		write_program(temp.path(), "counter");

		assert_eq!(
			compute_units_config(&nested)?.margin_percent,
			crate::compute_units::DEFAULT_COMPUTE_UNIT_MARGIN_PERCENT
		);

		for config in [
			"[compute_units]\nmargin_percent = 35\n",
			"[compute-units]\nmargin-percent = 35\n",
		] {
			fs::write(
				temp.path().join(CONFIG_FILE_NAME),
				format!("[project]\nprogram = \".\"\n\n{config}"),
			)?;

			assert_eq!(compute_units_config(&nested)?.margin_percent, 35);
			Project::discover(temp.path())?;
		}

		Ok(())
	}

	#[test]
	fn compute_unit_settings_fail_closed_on_unknown_or_invalid_values()
	-> Result<(), Box<dyn std::error::Error>> {
		let temp = TempDir::new()?;
		write_program(temp.path(), "counter");

		for config in [
			"[compute_units]\nmargin = 20\n",
			"[compute_units]\nmargin_percent = -5\n",
			"[compute_units]\nmargin_percent = \"20\"\n",
		] {
			fs::write(temp.path().join(CONFIG_FILE_NAME), config)?;

			assert!(
				matches!(
					compute_units_config(temp.path()),
					Err(ProjectError::ParseToml { .. })
				),
				"{config:?} must be rejected"
			);
		}

		assert!(matches!(
			compute_units_config(&temp.path().join("missing")),
			Err(ProjectError::InspectPath { .. })
		));

		Ok(())
	}

	#[test]
	fn compute_unit_settings_report_an_unreadable_config() -> Result<(), Box<dyn std::error::Error>>
	{
		let temp = TempDir::new()?;
		fs::write(temp.path().join(CONFIG_FILE_NAME), [0xff, 0xfe, 0xfd])?;

		assert!(matches!(
			compute_units_config(temp.path()),
			Err(ProjectError::ReadFile { .. })
		));

		Ok(())
	}

	/// The manifest owns the version width and the auto policy, so their
	/// `pina.toml` keys are unknown fields that fail discovery.
	#[test]
	fn migration_policy_keys_are_rejected_as_unknown_fields() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		write_program(temp.path(), "counter");
		write_file(
			&temp.path().join(CONFIG_FILE_NAME),
			b"[project]\nprogram = \".\"\n\n[migrations]\nversion_type = \"u16\"\n",
		);

		let error = Project::discover(temp.path()).expect_err("retired keys must fail closed");

		assert!(
			error.to_string().contains("unknown field"),
			"unexpected message: {error}"
		);
	}

	/// Snake case is the canonical `pina.toml` spelling for `[migrations.answers]`.
	#[test]
	fn migrations_answers_read_snake_case_keys() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		discover_with_migrations(
			temp.path(),
			"\n[migrations.answers]\nassume_removed = [\"dropped\"]\n",
		);
		let project = Project::discover(temp.path())
			.unwrap_or_else(|error| panic!("snake_case must parse: {error}"));
		assert_eq!(project.migration_answers.assume_removed, vec!["dropped"]);
	}

	#[test]
	fn migrations_answers_reject_unknown_keys() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		write_program(temp.path(), "counter");
		fs::write(
			temp.path().join(CONFIG_FILE_NAME),
			"[project]\nprogram = \".\"\n\n[migrations.answers]\nassume_removedx = []\n",
		)
		.unwrap_or_else(|error| panic!("failed to write config: {error}"));

		let error = Project::discover(temp.path())
			.expect_err("aliases must not weaken unknown-key rejection");

		assert!(error.to_string().contains("unknown field"), "{error}");
	}

	/// The CLI ecosystem spellings must match the rest of the CLI surface: the
	/// `--client` flag, the generated directory name, and `ClientLanguage::as_str`.
	#[test]
	fn client_languages_read_the_kebab_case_spelling() {
		fn languages(entries: &str) -> Vec<ClientLanguage> {
			let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
			write_program(temp.path(), "counter");
			fs::write(
				temp.path().join(CONFIG_FILE_NAME),
				format!("[project]\nprogram = \".\"\n\n[clients]\nlanguages = [{entries}]\n"),
			)
			.unwrap_or_else(|error| panic!("failed to write config: {error}"));

			Project::discover(temp.path())
				.unwrap_or_else(|error| panic!("`{entries}` must parse: {error}"))
				.clients
		}

		let expected = vec![
			ClientLanguage::CliRust,
			ClientLanguage::CliTs,
			ClientLanguage::CliDart,
		];
		assert_eq!(
			languages("\"cli-rust\", \"cli-ts\", \"cli-dart\""),
			expected
		);
		assert_eq!(ClientLanguage::CliRust.as_str(), "cli-rust");
		assert_eq!(
			languages("\"rust\", \"rust-kit\""),
			vec![ClientLanguage::Rust, ClientLanguage::RustKit]
		);
		assert_eq!(ClientLanguage::RustKit.as_str(), "rust-kit");
		assert_eq!(ClientLanguage::RustKit.base_client(), None);
	}

	#[test]
	fn rust_kit_client_override_resolves_under_clients_rust_kit() {
		let temp = TempDir::new().expect("temp dir failed");
		write_program(temp.path(), "counter");
		fs::write(
			temp.path().join(CONFIG_FILE_NAME),
			"[project]\nprogram = \".\"\n\n[clients]\nlanguages = [\"rust-kit\"]\n\n\
			 [clients.rust_kit]\noutput = \"kit\"\nscaffold = false\n",
		)
		.expect("failed to write config");

		let project = Project::discover(temp.path()).expect("rust-kit config must parse");
		assert_eq!(project.clients, vec![ClientLanguage::RustKit]);
		assert_eq!(
			project.client_generation[&ClientLanguage::RustKit],
			ClientGenerationConfig {
				output: PathBuf::from("kit"),
				mode: GenerationMode::Auto,
				scaffold: false,
			}
		);
	}

	#[test]
	fn config_rejects_absolute_paths_for_every_path_field() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		write_program(temp.path(), "counter");
		let cases = [
			("project.program", "[project]\nprogram = \"/outside\"\n"),
			(
				"project.idl_dir",
				"[project]\nprogram = \".\"\nidl_dir = \"/outside\"\n",
			),
			("clients.output", "[clients]\noutput = \"/outside\"\n"),
			(
				"clients.cpi.output",
				"[clients.cpi]\noutput = \"/outside\"\n",
			),
		];

		for (field, config) in cases {
			fs::write(temp.path().join(CONFIG_FILE_NAME), config)
				.unwrap_or_else(|error| panic!("failed to write config: {error}"));
			let error = Project::discover(temp.path())
				.expect_err("unsafe configured paths should fail closed");
			assert!(
				matches!(error, ProjectError::InvalidConfigPath { field: actual, .. } if actual == field),
				"unexpected error for {field}: {error}"
			);
		}
	}

	#[test]
	fn config_allows_parent_directory_paths() {
		let temp = TempDir::new().expect("create temp dir");
		let project_dir = temp.path().join("proj");
		let sibling_program = temp.path().join("other");
		write_program(&project_dir, "configured-program");
		write_program(&sibling_program, "sibling-program");
		fs::create_dir(temp.path().join(".git")).expect("create repository boundary");
		fs::write(
			project_dir.join(CONFIG_FILE_NAME),
			"[project]\nprogram = \"../other\"\n\n[clients]\noutput = \"../clients\"\n",
		)
		.expect("write config");

		let project = Project::discover(&project_dir).expect("discover project");
		let temp = fs::canonicalize(temp.path()).expect("canonicalize temp dir");

		assert_eq!(project.program_dir, temp.join("other"));
		assert_eq!(project.clients_dir, temp.join("clients"));
	}

	#[test]
	fn config_resolves_root_anchor_to_repository_root() {
		let temp = TempDir::new().expect("create temp dir");
		let project_dir = temp.path().join("proj");
		write_program(&project_dir, "counter");
		// A `.git` entry in the nearest ancestor anchors `{{root}}` without
		// requiring a real git repository.
		fs::create_dir(temp.path().join(".git")).expect("create .git directory");
		fs::write(
			project_dir.join(CONFIG_FILE_NAME),
			"[project]\nprogram = \".\"\nidl_dir = \"{{root}}/idls\"\n\n[clients]\noutput = \
			 \"{{root}}/clients\"\n\n[clients.rust]\noutput = \"{{root}}/clients/rust\"\n",
		)
		.expect("write config");

		let project = Project::discover(&project_dir).expect("discover project");
		let temp = fs::canonicalize(temp.path()).expect("canonicalize temp dir");

		assert_eq!(project.idl_dir, temp.join("idls"));
		assert_eq!(project.clients_dir, temp.join("clients"));
		assert_eq!(
			project.client_generation[&ClientLanguage::Rust].output,
			temp.join("clients/rust")
		);
	}

	#[test]
	fn config_resolves_declared_path_anchors() {
		let temp = TempDir::new().expect("create temp dir");
		let project_dir = temp.path().join("proj");
		write_program(&project_dir, "counter");
		fs::create_dir(temp.path().join(".git")).expect("create .git directory");
		fs::write(
			project_dir.join(CONFIG_FILE_NAME),
			"[project]\nprogram = \".\"\n\n[project.paths]\ncrates = \"{{root}}/crates\"\nshared \
			 = \"clients/shared\"\n\n[clients]\noutput = \
			 \"{{root}}/clients\"\n\n[clients.rust]\noutput = \"{{ crates \
			 }}\"\n\n[clients.typescript]\noutput = \"{{ shared }}\"\n",
		)
		.expect("write config");

		let project = Project::discover(&project_dir).expect("discover project");
		let temp = fs::canonicalize(temp.path()).expect("canonicalize temp dir");

		assert_eq!(
			project.client_generation[&ClientLanguage::Rust].output,
			temp.join("crates")
		);
		// Anchors may also hold plain relative paths.
		assert_eq!(
			project.client_generation[&ClientLanguage::Typescript].output,
			PathBuf::from("clients/shared")
		);
	}

	#[test]
	fn config_rejects_unknown_path_anchors() {
		let temp = TempDir::new().expect("create temp dir");
		write_program(temp.path(), "counter");
		fs::write(
			temp.path().join(CONFIG_FILE_NAME),
			"[project]\nprogram = \".\"\n\n[project.paths]\ncrates = \
			 \"{{root}}/crates\"\n\n[clients]\noutput = \"{{ nope }}\"\n",
		)
		.expect("write config");

		let error = Project::discover(temp.path()).expect_err("unknown anchors should fail closed");
		let message = error.to_string();

		assert!(
			matches!(error, ProjectError::UnknownTemplateAnchor { variable, .. } if variable == "nope"),
			"unexpected error: {message}"
		);
		assert!(
			message.contains("`root`") && message.contains("`crates`"),
			"error lists the known anchors: {message}"
		);
	}

	#[test]
	fn config_rejects_unterminated_path_anchors() {
		let temp = TempDir::new().expect("create temp dir");
		write_program(temp.path(), "counter");
		fs::write(
			temp.path().join(CONFIG_FILE_NAME),
			"[project]\nprogram = \".\"\n\n[clients]\noutput = \"{{root}/clients\"\n",
		)
		.expect("write config");

		let error =
			Project::discover(temp.path()).expect_err("unterminated anchors should fail closed");

		assert!(matches!(
			error,
			ProjectError::InvalidConfigPath {
				field: "clients.output",
				..
			}
		));
	}

	#[test]
	fn config_rejects_invalid_path_anchor_names() {
		let temp = TempDir::new().expect("create temp dir");
		write_program(temp.path(), "counter");
		fs::write(
			temp.path().join(CONFIG_FILE_NAME),
			"[project]\nprogram = \".\"\n\n[clients]\noutput = \"{{ 1bad }}\"\n",
		)
		.expect("write config");

		let error =
			Project::discover(temp.path()).expect_err("invalid anchor names should fail closed");

		assert!(matches!(
			error,
			ProjectError::InvalidConfigPath {
				field: "clients.output",
				..
			}
		));
	}

	#[test]
	fn config_rejects_empty_paths() {
		let temp = TempDir::new().expect("create temp dir");
		write_program(temp.path(), "counter");
		fs::write(
			temp.path().join(CONFIG_FILE_NAME),
			"[project]\nprogram = \"\"\n",
		)
		.expect("write config");

		let error = Project::discover(temp.path()).expect_err("empty paths should fail closed");

		assert!(matches!(
			error,
			ProjectError::InvalidConfigPath {
				field: "project.program",
				..
			}
		));
	}

	#[test]
	fn config_rejects_root_anchor_outside_a_git_repository() {
		let temp = TempDir::new().expect("create temp dir");
		write_program(temp.path(), "counter");
		fs::write(
			temp.path().join(CONFIG_FILE_NAME),
			"[project]\nprogram = \".\"\n\n[clients]\noutput = \"{{root}}/clients\"\n",
		)
		.expect("write config");

		let error =
			Project::discover(temp.path()).expect_err("root anchor without git should fail closed");

		assert!(matches!(
			error,
			ProjectError::InvalidConfigPath {
				field: "clients.output",
				..
			}
		));
	}

	#[test]
	fn config_rejects_invalid_paths_entries() {
		let temp = TempDir::new().expect("create temp dir");
		write_program(temp.path(), "counter");
		let cases = [
			(
				"`root` is reserved",
				"[project.paths]\nroot = \"{{root}}/x\"\n",
			),
			(
				"anchor names must match",
				"[project.paths]\n\"1bad\" = \"{{root}}/x\"\n",
			),
			(
				"may only reference",
				"[project.paths]\ncrates = \"{{ packages }}/x\"\n",
			),
			(
				"anchor values must be non-empty",
				"[project.paths]\nempty = \"\"\n",
			),
			(
				"anchor values must be relative or anchored",
				"[project.paths]\nabsolute = \"/outside\"\n",
			),
			("closing `}}`", "[project.paths]\ncrates = \"{{root}/x\"\n"),
		];

		for (expected, config) in cases {
			fs::write(temp.path().join(CONFIG_FILE_NAME), config).expect("write config");
			let error = Project::discover(temp.path())
				.expect_err("invalid `[project.paths]` entries should fail closed");
			let message = error.to_string();

			assert!(
				matches!(error, ProjectError::InvalidPathsEntry { .. }),
				"unexpected error kind: {message}"
			);
			assert!(
				message.contains(expected),
				"error should mention {expected}: {message}"
			);
		}
	}

	#[test]
	fn relative_normalization_keeps_leading_parent_components() {
		assert_eq!(normalize_relative(Path::new("a/../b")), PathBuf::from("b"));
		assert_eq!(normalize_relative(Path::new("../x")), PathBuf::from("../x"));
		assert_eq!(
			normalize_relative(Path::new("a/../../b")),
			PathBuf::from("../b")
		);
		assert_eq!(normalize_relative(Path::new("./a/.")), PathBuf::from("a"));
	}

	#[test]
	fn absolute_normalization_rejects_traversal_above_root() {
		let error = normalize_absolute(Path::new("/../outside"), "test.path", "/../outside")
			.expect_err("traversal above the filesystem root should fail closed");

		assert!(matches!(
			error,
			ProjectError::InvalidConfigPath {
				field: "test.path",
				..
			}
		));
	}

	#[test]
	fn git_root_discovery_finds_the_current_worktree() {
		let root = discover_git_root(Path::new(env!("CARGO_MANIFEST_DIR")))
			.expect("current crate belongs to a git worktree");

		assert!(root.join(".git").exists());
		assert!(root.join("Cargo.toml").is_file());
	}

	#[test]
	fn git_root_discovery_uses_the_nearest_repository_or_worktree_marker() {
		let temp = TempDir::new().expect("create temp dir");
		let repository = temp.path().join("repository");
		let worktree = repository.join("nested-worktree");
		let project = worktree.join("crates/program");
		std::fs::create_dir_all(repository.join(".git")).expect("create repository marker");
		std::fs::create_dir_all(&project).expect("create nested project");
		assert_eq!(discover_git_root(&project), Some(repository));

		std::fs::write(worktree.join(".git"), "gitdir: /unused/worktrees/nested\n")
			.expect("write worktree marker");
		assert_eq!(discover_git_root(&project), Some(worktree));
		assert_eq!(discover_git_root(temp.path()), None);
	}

	#[test]
	fn git_root_discovery_ignores_parent_repository_environment() {
		const CHILD_MARKER: &str = "PINA_TEST_GIT_ENVIRONMENT_ISOLATION";

		if std::env::var_os(CHILD_MARKER).is_some() {
			let temp = TempDir::new().expect("create temp dir");
			assert_eq!(discover_git_root(temp.path()), None);
			return;
		}

		let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
		let git_dir = Command::new("git")
			.current_dir(crate_root)
			.args(["rev-parse", "--absolute-git-dir"])
			.output()
			.expect("discover git directory");
		let work_tree = Command::new("git")
			.current_dir(crate_root)
			.args(["rev-parse", "--show-toplevel"])
			.output()
			.expect("discover git worktree");
		assert!(git_dir.status.success());
		assert!(work_tree.status.success());

		let output = Command::new(std::env::current_exe().expect("locate test binary"))
			.args([
				"project::tests::git_root_discovery_ignores_parent_repository_environment",
				"--exact",
				"--nocapture",
			])
			.env(CHILD_MARKER, "1")
			.env("GIT_CONFIG_COUNT", "1")
			.env("GIT_CONFIG_KEY_0", "user.name")
			.env("GIT_CONFIG_VALUE_0", "Injected")
			.env("GIT_DIR", String::from_utf8_lossy(&git_dir.stdout).trim())
			.env(
				"GIT_WORK_TREE",
				String::from_utf8_lossy(&work_tree.stdout).trim(),
			)
			.output()
			.expect("run isolated git environment test");

		assert!(
			output.status.success(),
			"isolated child failed:\n{}\n{}",
			String::from_utf8_lossy(&output.stdout),
			String::from_utf8_lossy(&output.stderr)
		);
	}

	#[cfg(unix)]
	#[test]
	fn config_rejects_program_and_output_symlink_escapes() {
		use std::os::unix::fs::symlink;

		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let root = temp.path().join("project");
		let external = temp.path().join("external");
		write_program(&root, "counter");
		write_program(&external, "external");

		symlink(&external, root.join("linked-program"))
			.unwrap_or_else(|error| panic!("failed to link program: {error}"));
		fs::write(
			root.join(CONFIG_FILE_NAME),
			"[project]\nprogram = \"linked-program\"\n",
		)
		.unwrap_or_else(|error| panic!("failed to write config: {error}"));
		assert!(matches!(
			Project::discover(&root),
			Err(ProjectError::InvalidConfigPath {
				field: "project.program",
				..
			})
		));

		fs::remove_file(root.join("linked-program"))
			.unwrap_or_else(|error| panic!("failed to remove program link: {error}"));
		symlink(&external, root.join("linked-output"))
			.unwrap_or_else(|error| panic!("failed to link output: {error}"));
		for (field, config) in [
			(
				"project.idl_dir",
				"[project]\nprogram = \".\"\nidl_dir = \"linked-output\"\n",
			),
			("clients.output", "[clients]\noutput = \"linked-output\"\n"),
		] {
			fs::write(root.join(CONFIG_FILE_NAME), config)
				.unwrap_or_else(|error| panic!("failed to write config: {error}"));
			assert!(matches!(
				Project::discover(&root),
				Err(ProjectError::InvalidConfigPath { field: actual, .. }) if actual == field
			));
		}
	}

	#[cfg(unix)]
	#[test]
	fn cargo_target_symlinks_are_accepted_without_relaxing_configured_paths() {
		use std::os::unix::fs::symlink;

		let temp = TempDir::new().expect("temp dir");
		let root = fs::canonicalize(temp.path())
			.expect("canonicalizing the temp dir")
			.join("project");
		let shared = root.with_file_name("shared-target");
		write_program(&root, "counter");
		fs::create_dir(&shared).expect("creating the shared cache");
		symlink(&shared, root.join("target")).expect("linking the shared cache as target");

		// Cargo reports this link as its `target_directory`; a shared build
		// cache redirects it, and discovery accepts the path Cargo owns.
		let project = Project::discover(&root).expect("a symlinked target should resolve");
		assert_eq!(project.target_dir, root.join("target"));
		assert_eq!(project.idl_dir, root.join("target/idl"));

		// Only Cargo's own path is exempt: one `pina.toml` configures is still
		// refused when it crosses a symbolic link.
		symlink(&shared, root.join("linked-idl")).expect("linking the configured idl dir");
		fs::write(
			root.join(CONFIG_FILE_NAME),
			"[project]\nprogram = \".\"\nidl_dir = \"linked-idl\"\n",
		)
		.expect("writing the project config");

		assert!(matches!(
			Project::discover(&root),
			Err(ProjectError::InvalidConfigPath {
				field: "project.idl_dir",
				..
			})
		));
	}

	#[cfg(unix)]
	#[test]
	fn anchored_config_paths_reject_symlinks_under_repository_root() {
		use std::os::unix::fs::symlink;

		let temp = TempDir::new().expect("temp dir");
		let repo = fs::canonicalize(temp.path())
			.expect("canonicalize temp dir")
			.join("repo");
		let project_dir = repo.join("programs/counter");
		let external = temp.path().join("external");
		write_program(&project_dir, "counter");
		write_program(&external, "external");
		fs::create_dir(repo.join(".git")).expect("create .git directory");
		symlink(&external, repo.join("linked-program")).expect("link program at repo root");
		symlink(&external, repo.join("linked-clients")).expect("link clients at repo root");

		for (field, config) in [
			(
				"project.program",
				"[project]\nprogram = \"{{root}}/linked-program\"\n",
			),
			(
				"clients.output",
				"[project]\nprogram = \".\"\n\n[clients]\noutput = \"{{root}}/linked-clients\"\n",
			),
		] {
			fs::write(project_dir.join(CONFIG_FILE_NAME), config).expect("write config");
			assert!(
				matches!(
					Project::discover(&project_dir),
					Err(ProjectError::InvalidConfigPath { field: actual, .. }) if actual == field
				),
				"expected symlink rejection for {field}"
			);
		}
	}

	#[test]
	fn anchor_name_validation_and_extraction_cover_allowed_characters_and_short_anchors() {
		for valid in ["root", "x", "_leading", "my_anchor", "my-anchor", "a1_b-2"] {
			assert!(
				is_valid_anchor_name(valid),
				"expected {valid:?} to be a valid anchor name"
			);
		}
		for invalid in ["", "1bad", "-bad", "bad.name", "bad/name", "bad space"] {
			assert!(
				!is_valid_anchor_name(invalid),
				"expected {invalid:?} to be rejected as an anchor name"
			);
		}

		assert_eq!(extract_anchors("{{x}}/clients"), Ok(vec!["x"]));
		assert_eq!(
			extract_anchors("{{root}}/{{a}}/{{b}}"),
			Ok(vec!["root", "a", "b"])
		);
		assert!(extract_anchors("{{root/clients").is_err());

		let temp = TempDir::new().expect("temp dir");
		let project_dir = temp.path().join("proj");
		write_program(&project_dir, "counter");
		fs::create_dir(temp.path().join(".git")).expect("create .git directory");
		fs::write(
			project_dir.join(CONFIG_FILE_NAME),
			"[project]\nprogram = \".\"\n\n[project.paths]\nmy_sdk-1 = \"{{root}}/sdks\"\nx = \
			 \"{{root}}/x\"\n\n[clients]\noutput = \"{{ my_sdk-1 }}\"\n\n[clients.rust]\noutput \
			 = \"{{ x }}/rust\"\n",
		)
		.expect("write config");

		let project = Project::discover(&project_dir).expect("discover project");
		let root = fs::canonicalize(temp.path()).expect("canonicalize root");
		assert_eq!(project.clients_dir, root.join("sdks"));
		assert_eq!(
			project.client_generation[&ClientLanguage::Rust].output,
			root.join("x/rust")
		);
	}

	#[test]
	fn sanitize_git_environment_removes_static_and_dynamic_git_config_vars() {
		const CHILD_MARKER: &str = "PINA_TEST_SANITIZE_GIT_ENV";

		if std::env::var_os(CHILD_MARKER).is_some() {
			let mut command = Command::new("git");
			sanitize_git_environment(&mut command);
			let removed: Vec<_> = command
				.get_envs()
				.filter_map(|(key, value)| value.is_none().then_some(key.to_os_string()))
				.collect();
			for expected in [
				"GIT_DIR",
				"GIT_WORK_TREE",
				"GIT_CONFIG_COUNT",
				"GIT_CONFIG_KEY_0",
				"GIT_CONFIG_VALUE_0",
			] {
				assert!(
					removed.iter().any(|key| key == expected),
					"expected {expected} to be removed, got {removed:?}"
				);
			}
			return;
		}

		let output = Command::new(std::env::current_exe().expect("locate test binary"))
			.args([
				"project::tests::sanitize_git_environment_removes_static_and_dynamic_git_config_vars",
				"--exact",
				"--nocapture",
			])
			.env(CHILD_MARKER, "1")
			.env("GIT_DIR", "/untrusted/repository.git")
			.env("GIT_WORK_TREE", "/untrusted/worktree")
			.env("GIT_CONFIG_COUNT", "1")
			.env("GIT_CONFIG_KEY_0", "user.name")
			.env("GIT_CONFIG_VALUE_0", "Injected")
			.output()
			.expect("run sanitize_git_environment child test");

		assert!(
			output.status.success(),
			"sanitize_git_environment child failed:\n{}\n{}",
			String::from_utf8_lossy(&output.stdout),
			String::from_utf8_lossy(&output.stderr)
		);
	}

	#[test]
	fn windows_link_and_plain_file_predicates_classify_reparse_points() {
		assert!(!is_windows_link_like(false, 0));
		assert!(!is_windows_link_like(false, 0x0020));
		assert!(is_windows_link_like(false, WINDOWS_REPARSE_POINT_ATTRIBUTE));
		assert!(is_windows_link_like(
			false,
			WINDOWS_REPARSE_POINT_ATTRIBUTE | 0x0020
		));
		assert!(is_windows_link_like(true, 0));

		assert!(is_windows_plain_file(true, false, 0));
		assert!(is_windows_plain_file(true, false, 0x0020));
		assert!(!is_windows_plain_file(false, false, 0));
		assert!(!is_windows_plain_file(
			true,
			false,
			WINDOWS_REPARSE_POINT_ATTRIBUTE
		));
		assert!(!is_windows_plain_file(
			false,
			true,
			WINDOWS_REPARSE_POINT_ATTRIBUTE
		));
	}

	#[test]
	fn resolve_cargo_home_reads_process_cargo_home_and_user_home() {
		const CHILD_MODE: &str = "PINA_TEST_RESOLVE_CARGO_HOME_MODE";

		if let Some(mode) = std::env::var_os(CHILD_MODE) {
			let cwd = Path::new("/work/repo/pkg");
			let expected = PathBuf::from(
				std::env::var_os("PINA_TEST_EXPECTED_CARGO_HOME").expect("expected path"),
			);
			assert_eq!(resolve_cargo_home(cwd), Some(expected));
			if mode == "home" {
				let expected_home = PathBuf::from(
					std::env::var_os("PINA_TEST_EXPECTED_USER_HOME").expect("expected user home"),
				);
				assert_eq!(current_user_home(), Some(expected_home));
			}
			return;
		}

		let temp = TempDir::new().expect("temp dir");
		let base = fs::canonicalize(temp.path()).expect("canonicalize temp dir");
		let custom_cargo_home = base.join("custom-cargo-home");
		let user_home = base.join("user-home");
		let win_home = base.join("win-home");
		let current_exe = std::env::current_exe().expect("locate test binary");

		for (mode, cargo_home, home, userprofile, expected_cargo_home, expected_user_home) in [
			(
				"cargo_home",
				Some(custom_cargo_home.as_os_str()),
				Some(user_home.as_os_str()),
				None,
				custom_cargo_home.clone(),
				user_home.clone(),
			),
			(
				"home",
				None,
				Some(user_home.as_os_str()),
				None,
				user_home.join(".cargo"),
				user_home.clone(),
			),
			(
				"userprofile_fallback",
				Some(std::ffi::OsStr::new("")),
				Some(std::ffi::OsStr::new("")),
				Some(win_home.as_os_str()),
				win_home.join(".cargo"),
				win_home.clone(),
			),
		] {
			let mut command = Command::new(&current_exe);
			command
				.args([
					"project::tests::resolve_cargo_home_reads_process_cargo_home_and_user_home",
					"--exact",
					"--nocapture",
				])
				.env(CHILD_MODE, mode)
				.env("PINA_TEST_EXPECTED_CARGO_HOME", &expected_cargo_home)
				.env("PINA_TEST_EXPECTED_USER_HOME", &expected_user_home)
				.env_remove("CARGO_HOME")
				.env_remove("HOME")
				.env_remove("USERPROFILE");
			for (name, value) in [
				("CARGO_HOME", cargo_home),
				("HOME", home),
				("USERPROFILE", userprofile),
			] {
				if let Some(value) = value {
					command.env(name, value);
				}
			}

			let output = command.output().expect("run resolve_cargo_home child test");
			assert!(
				output.status.success(),
				"mode {mode} failed:\n{}\n{}",
				String::from_utf8_lossy(&output.stdout),
				String::from_utf8_lossy(&output.stderr)
			);
		}
	}

	#[test]
	fn cargo_metadata_discovers_single_package_without_config() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		write_program(temp.path(), "existing-program");

		let project = Project::discover(temp.path())
			.unwrap_or_else(|error| panic!("discovery failed: {error}"));
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("failed to canonicalize root: {error}"));

		assert_eq!(
			fs::canonicalize(&project.program_dir)
				.unwrap_or_else(|error| panic!("failed to canonicalize project root: {error}")),
			root
		);
		assert_eq!(project.package_name, "existing-program");
		assert_eq!(project.library_name, "existing_program");
	}

	#[test]
	fn cargo_metadata_discovers_package_from_nested_directory() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let nested = temp.path().join("src/instructions");
		write_program(temp.path(), "existing-program");
		fs::create_dir_all(&nested)
			.unwrap_or_else(|error| panic!("failed to create nested dir: {error}"));

		let project =
			Project::discover(&nested).unwrap_or_else(|error| panic!("discovery failed: {error}"));

		assert_eq!(project.package_name, "existing-program");
	}

	#[test]
	fn cargo_metadata_rejects_ambiguous_workspace_root() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		write_program(&temp.path().join("programs/one"), "one");
		write_program(&temp.path().join("programs/two"), "two");
		fs::write(
			temp.path().join("Cargo.toml"),
			"[workspace]\nmembers = [\"programs/one\", \"programs/two\"]\nresolver = \"3\"\n",
		)
		.unwrap_or_else(|error| panic!("failed to write workspace: {error}"));

		let error = Project::discover(temp.path())
			.expect_err("workspace root with multiple packages should be ambiguous");

		assert!(error.to_string().contains("Candidates: one, two"));
	}

	#[test]
	fn discovery_reports_invalid_paths_and_manifests() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let missing = temp.path().join("missing");
		assert!(matches!(
			Project::discover(&missing),
			Err(ProjectError::InspectPath { .. })
		));
		fs::write(
			temp.path().join(CONFIG_FILE_NAME),
			"[project]\nprogram = \"missing\"\n",
		)
		.unwrap_or_else(|error| panic!("failed to write config: {error}"));
		assert!(matches!(
			Project::discover(temp.path()),
			Err(ProjectError::InspectPath { .. })
		));

		let program = temp.path().join("program");
		fs::create_dir_all(&program)
			.unwrap_or_else(|error| panic!("failed to create program: {error}"));
		fs::write(
			temp.path().join(CONFIG_FILE_NAME),
			"[project]\nprogram = \"program\"\n",
		)
		.unwrap_or_else(|error| panic!("failed to write config: {error}"));
		assert!(matches!(
			Project::discover(&program),
			Err(ProjectError::MissingManifest { .. })
		));

		fs::write(program.join("Cargo.toml"), "not valid toml = [")
			.unwrap_or_else(|error| panic!("failed to write manifest: {error}"));
		assert!(matches!(
			Project::discover(&program),
			Err(ProjectError::CargoMetadata { .. })
		));
	}

	#[test]
	fn discovery_accepts_a_manifest_file_as_the_start_path() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		write_program(temp.path(), "file-start");

		let project = Project::discover(&temp.path().join("Cargo.toml"))
			.unwrap_or_else(|error| panic!("discovery failed: {error}"));

		assert_eq!(project.package_name, "file-start");
	}

	#[test]
	fn cargo_metadata_uses_the_only_workspace_package() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		write_program(&temp.path().join("program"), "only-program");
		fs::write(
			temp.path().join("Cargo.toml"),
			"[workspace]\nmembers = [\"program\"]\nresolver = \"3\"\n",
		)
		.unwrap_or_else(|error| panic!("failed to write workspace: {error}"));

		let project = Project::discover(temp.path())
			.unwrap_or_else(|error| panic!("discovery failed: {error}"));

		assert_eq!(project.package_name, "only-program");
	}

	#[test]
	fn cargo_metadata_failure_has_discovery_context() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		fs::write(temp.path().join("Cargo.toml"), "invalid = [")
			.unwrap_or_else(|error| panic!("failed to write manifest: {error}"));

		let error = Project::discover(temp.path()).expect_err("invalid metadata should fail");

		assert!(matches!(error, ProjectError::CargoMetadata { .. }));
	}

	#[test]
	fn cargo_metadata_rejects_packages_without_a_library_target() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		fs::create_dir_all(temp.path().join("src"))
			.unwrap_or_else(|error| panic!("failed to create source directory: {error}"));
		fs::write(
			temp.path().join("Cargo.toml"),
			"[package]\nname = \"binary-package\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
		)
		.unwrap_or_else(|error| panic!("failed to write manifest: {error}"));
		fs::write(temp.path().join("src/main.rs"), "fn main() {}")
			.unwrap_or_else(|error| panic!("failed to write binary: {error}"));

		let error = Project::discover(temp.path())
			.expect_err("a Pina program must define a library target");

		assert!(matches!(error, ProjectError::MissingLibraryTarget { .. }));
	}

	#[test]
	fn direct_helpers_report_unreadable_inputs() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let missing_config = temp.path().join(CONFIG_FILE_NAME);
		assert!(matches!(
			Project::from_config(&missing_config),
			Err(ProjectError::ReadFile { .. })
		));

		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("failed to canonicalize root: {error}"));
		let templates = PathTemplates::empty(root.clone());
		assert_eq!(
			templates
				.resolve_output(&root, "clients.output", ".")
				.unwrap_or_else(|error| panic!("dot output should resolve: {error}")),
			PathBuf::from(".")
		);
		let too_long = "x".repeat(32 * 1024);
		assert!(matches!(
			templates.resolve_output(&root, "clients.output", &too_long),
			Err(ProjectError::InspectPath { .. })
		));

		write_program(&root, "counter");
		let manifest_path = root.join("Cargo.toml");
		let mut metadata = cargo_metadata(&root, Some(&manifest_path))
			.unwrap_or_else(|error| panic!("metadata failed: {error}"));
		metadata.packages.clear();
		assert!(matches!(
			package_for_manifest(&metadata, &manifest_path, &root),
			Err(ProjectError::CargoMetadata { .. })
		));
	}

	#[test]
	fn output_prefixes_accept_only_equivalent_windows_namespaces() {
		use std::ffi::OsStr;
		use std::path::Prefix;

		let server = OsStr::new("server");
		let share = OsStr::new("share");
		let other = OsStr::new("other");
		let cases = [
			(Prefix::Disk(b'C'), Prefix::VerbatimDisk(b'c'), true),
			(Prefix::VerbatimDisk(b'C'), Prefix::Disk(b'D'), false),
			(
				Prefix::UNC(server, share),
				Prefix::VerbatimUNC(server, share),
				true,
			),
			(
				Prefix::VerbatimUNC(server, share),
				Prefix::UNC(server, other),
				false,
			),
			(
				Prefix::UNC(server, share),
				Prefix::VerbatimUNC(other, share),
				false,
			),
			(Prefix::DeviceNS(server), Prefix::DeviceNS(server), true),
			(Prefix::DeviceNS(server), Prefix::Verbatim(server), false),
			(Prefix::Disk(b'C'), Prefix::UNC(server, share), false),
		];

		for (left, right, expected) in cases {
			assert_eq!(output_prefixes_equal(left, right), expected);
			assert_eq!(output_prefixes_equal(right, left), expected);
		}
	}

	#[cfg(windows)]
	#[test]
	fn output_roots_accept_windows_namespace_aliases_without_widening_containment() {
		for (root, path) in [
			(r"D:\repository", r"\\?\D:\repository\target\idl"),
			(r"\\?\D:\repository", r"D:\repository\target\idl"),
			(
				r"\\server\share\repository",
				r"\\?\UNC\server\share\repository\target\idl",
			),
		] {
			assert_eq!(
				strip_output_root(Path::new(root), Path::new(path)),
				Some(Path::new(r"target\idl"))
			);
		}

		for path in [
			r"\\?\E:\repository\target",
			r"\\?\D:\repository-other\target",
		] {
			assert!(strip_output_root(Path::new(r"D:\repository"), Path::new(path)).is_none());
		}
	}

	#[test]
	fn output_directories_are_normalized_and_contained_before_creation() {
		let temp = TempDir::new().unwrap();
		let root = fs::canonicalize(temp.path()).unwrap();
		let path = root.join("new/../idls/nested");
		assert_eq!(
			resolve_output_dir(&root, "test.output", &path).unwrap(),
			root.join("idls/nested")
		);
		assert_eq!(
			resolve_output_dir(&root, "test.output", &root).unwrap(),
			root
		);

		for path in [
			root.join("new/../../outside/idls"),
			root.with_file_name("other").join("idls"),
		] {
			assert!(matches!(
				resolve_output_dir(&root, "test.output", &path),
				Err(ProjectError::InvalidConfigPath {
					field: "test.output",
					..
				})
			));
		}

		assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
	}

	#[test]
	fn cargo_target_directory_must_still_stay_inside_the_root() {
		let temp = TempDir::new().unwrap();
		let root = fs::canonicalize(temp.path()).unwrap();

		assert_eq!(
			resolve_cargo_target_dir(&root, &root.join("target")).unwrap(),
			root.join("target")
		);

		// Cargo may put links along its target directory, but it may not name
		// one outside the worktree.
		for path in [
			root.join("nested/../../outside/target"),
			root.with_file_name("other").join("target"),
		] {
			let error = resolve_cargo_target_dir(&root, &path)
				.expect_err("Cargo's target directory must stay inside the root");
			assert!(
				matches!(
					error,
					ProjectError::InvalidConfigPath {
						field: "target_directory",
						..
					}
				),
				"{error}"
			);
			let message = error.to_string();
			assert!(message.contains("must stay inside"), "{message}");
			assert!(message.contains("CARGO_TARGET_DIR"), "{message}");
			assert!(!message.contains("pina.toml"), "{message}");
		}
	}

	#[cfg(unix)]
	#[test]
	fn output_directories_reject_links_before_nonexistent_suffixes() {
		let temp = TempDir::new().unwrap();
		let root = fs::canonicalize(temp.path()).unwrap();
		fs::create_dir(root.join("inside")).unwrap();
		std::os::unix::fs::symlink(root.join("inside"), root.join("linked")).unwrap();

		let error = resolve_output_dir(&root, "test.output", &root.join("linked/new/idls"))
			.expect_err("links are refused even when their canonical target is contained");
		assert!(error.to_string().contains("symbolic links"), "{error}");
		assert!(!root.join("inside/new").exists());
	}

	#[test]
	fn containment_root_uses_the_repository_and_rejects_homes() {
		let project = Path::new("/work/repo/program");
		let repository = Path::new("/work/repo");
		let workspace = Path::new("/Users/dev/ws");
		let ws_project = Path::new("/Users/dev/ws/program");
		let home = Path::new("/Users/dev");

		assert_eq!(containment_root(None, None, project, None), project);
		assert_eq!(containment_root(None, None, project, Some(home)), project);
		assert_eq!(
			containment_root(Some(repository.to_path_buf()), None, project, None),
			repository
		);
		// A repository that contains the home directory would contain
		// everything, so containment falls back to the Cargo workspace when one
		// sits below home, and otherwise to the project directory.
		assert_eq!(
			containment_root(Some(home.to_path_buf()), None, project, Some(home)),
			project
		);
		assert_eq!(
			containment_root(
				Some(home.to_path_buf()),
				Some(workspace.to_path_buf()),
				ws_project,
				Some(home)
			),
			workspace
		);
		assert_eq!(
			containment_root(None, Some(home.to_path_buf()), ws_project, Some(home)),
			ws_project
		);
		assert_eq!(
			containment_root(
				Some(Path::new("/").to_path_buf()),
				None,
				project,
				Some(home)
			),
			project
		);
		// A repository beside or below the home directory contains nothing
		// special, so it stays the containment root.
		assert_eq!(
			containment_root(
				Some(Path::new("/Users/dev/code/repo").to_path_buf()),
				None,
				project,
				Some(home)
			),
			Path::new("/Users/dev/code/repo")
		);
	}

	#[test]
	fn generation_containment_root_falls_back_to_the_project_directory() {
		let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("{error}"));
		let project = temp.path().join("program");
		fs::create_dir_all(&project).unwrap_or_else(|error| panic!("{error}"));

		// The temporary directory sits outside any repository, so discovery
		// finds nothing and the project directory is the containment root.
		assert_eq!(generation_containment_root(&project), project);
	}

	#[test]
	fn resolve_cargo_home_from_handles_absolute_relative_and_user_home() {
		let cwd = Path::new("/work/repo/pkg");
		assert_eq!(
			resolve_cargo_home_from(cwd, Some(OsString::from("/opt/cargo")), None),
			Some(PathBuf::from("/opt/cargo"))
		);
		assert_eq!(
			resolve_cargo_home_from(cwd, Some(OsString::from("../cargo-home")), None),
			Some(PathBuf::from("/work/repo/cargo-home"))
		);
		assert_eq!(
			resolve_cargo_home_from(
				cwd,
				Some(OsString::new()),
				Some(OsString::from("/Users/dev"))
			),
			Some(PathBuf::from("/Users/dev/.cargo"))
		);
		assert_eq!(resolve_cargo_home_from(cwd, None, None), None);
	}

	#[test]
	fn cargo_config_trust_boundary_encloses_submodules_workspaces_and_stops_at_home() {
		let temp = TempDir::new().unwrap();
		let base = fs::canonicalize(temp.path()).unwrap();
		let home = base.join("home");
		let worktrees = home.join("worktrees");
		let repo = worktrees.join("repo");
		let deep = repo.join("sub/deep");
		fs::create_dir_all(home.join(".git")).unwrap();
		fs::create_dir_all(repo.join(".git")).unwrap();
		fs::create_dir_all(&deep).unwrap();
		fs::write(deep.join("Cargo.toml"), "[package]\nname = \"d\"\n").unwrap();

		// Normal repo below a `git init ~` home stops before home and keeps the
		// repository root rather than shrinking to the nested package manifest.
		assert_eq!(
			cargo_config_trust_boundary(&deep, &repo, Some(&home)),
			(repo.clone(), true)
		);

		// A submodule or vendored repo inside an outer repo without a root
		// Cargo.toml lifts the trust boundary to the outer `.git` repository.
		let outer = worktrees.join("outer-repo");
		let submodule = outer.join("vendor/sub");
		fs::create_dir_all(outer.join(".git")).unwrap();
		fs::create_dir_all(&submodule).unwrap();
		fs::write(submodule.join(".git"), "gitdir: ../../.git/modules/sub\n").unwrap();
		assert_eq!(
			cargo_config_trust_boundary(&submodule, &submodule, Some(&home)),
			(outer, true)
		);

		// A non-git workspace lifts the trust boundary to the enclosing
		// Cargo.toml and reports no git boundary so parent configs above it are
		// not trusted.
		let ws = worktrees.join("unpacked-ws");
		let member = ws.join("programs/app");
		fs::create_dir_all(&member).unwrap();
		fs::write(ws.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
		fs::write(member.join("Cargo.toml"), "[package]\nname = \"a\"\n").unwrap();
		assert_eq!(
			cargo_config_trust_boundary(&member, &member, Some(&home)),
			(ws, false)
		);
	}

	#[test]
	fn operator_cargo_config_target_distinguishes_in_tree_and_out_of_tree_configs() {
		let temp = TempDir::new().unwrap();
		let base = fs::canonicalize(temp.path()).unwrap();
		let worktrees = base.join("worktrees");
		let repo = worktrees.join("repo");
		let project = repo.join("project");
		let cargo_home = base.join("cargo-home");
		let in_tree_cargo_home = repo.join(".cargo-home");
		fs::create_dir_all(project.join(".cargo")).unwrap();
		fs::create_dir_all(worktrees.join(".cargo")).unwrap();
		fs::create_dir_all(&cargo_home).unwrap();
		fs::create_dir_all(&in_tree_cargo_home).unwrap();

		// A benign in-tree config without build.target-dir or include lets the
		// parent operator config win.
		fs::write(
			project.join(".cargo/config.toml"),
			"[alias]\ncheck-all = \"check\"\n",
		)
		.unwrap();
		fs::write(
			worktrees.join(".cargo/config.toml"),
			"[build]\ntarget-dir = \"../shared-target\"\n",
		)
		.unwrap();
		assert_eq!(
			operator_cargo_config_target(&project, &repo, true, Some(&cargo_home)),
			Some(worktrees.join("../shared-target"))
		);

		// Without a repository boundary (`allow_ancestor_configs = false`),
		// parent ancestor configs above the trust root are not trusted, while
		// out-of-tree `$CARGO_HOME` still works.
		assert_eq!(
			operator_cargo_config_target(&project, &repo, false, Some(&cargo_home)),
			None
		);

		// Extensionless `config` takes precedence over `config.toml` out of
		// tree, matching Cargo: if `config` sets `target-dir`, it wins even
		// when `config.toml` exists; if `config` omits `target-dir`, Cargo
		// ignores `config.toml` rather than falling through to it.
		let absolute_target = base.join("abs-target");
		fs::write(
			worktrees.join(".cargo/config"),
			format!(
				"[build]\ntarget-dir = \"{}\"\n",
				absolute_target.to_string_lossy().replace('\\', "\\\\")
			),
		)
		.unwrap();
		assert_eq!(
			operator_cargo_config_target(&project, &repo, true, Some(&cargo_home)),
			Some(absolute_target.clone())
		);
		fs::write(
			worktrees.join(".cargo/config"),
			"[alias]\ncheck-all = \"check\"\n",
		)
		.unwrap();
		assert_eq!(
			operator_cargo_config_target(&project, &repo, true, Some(&cargo_home)),
			None
		);

		// A non-file `config.toml` directory out-of-tree is ignored when a file
		// `config` exists, while a non-file `config` directory fails closed.
		fs::remove_file(worktrees.join(".cargo/config.toml")).unwrap();
		fs::create_dir(worktrees.join(".cargo/config.toml")).unwrap();
		fs::write(
			worktrees.join(".cargo/config"),
			format!(
				"[build]\ntarget-dir = \"{}\"\n",
				absolute_target.to_string_lossy().replace('\\', "\\\\")
			),
		)
		.unwrap();
		assert_eq!(
			operator_cargo_config_target(&project, &repo, true, Some(&cargo_home)),
			Some(absolute_target)
		);
		fs::remove_file(worktrees.join(".cargo/config")).unwrap();
		fs::create_dir(worktrees.join(".cargo/config")).unwrap();
		assert_eq!(
			operator_cargo_config_target(&project, &repo, true, Some(&cargo_home)),
			None
		);
		fs::remove_dir(worktrees.join(".cargo/config")).unwrap();

		// Empty or non-string target-dir fails closed.
		fs::write(
			worktrees.join(".cargo/config"),
			"[build]\ntarget-dir = \"\"\n",
		)
		.unwrap();
		assert_eq!(
			operator_cargo_config_target(&project, &repo, true, Some(&cargo_home)),
			None
		);

		// Falling back to CARGO_HOME when no parent sets target-dir works
		// (even when `allow_ancestor_configs` is false), unless CARGO_HOME is
		// inside the trust root.
		fs::remove_file(worktrees.join(".cargo/config")).unwrap();
		fs::remove_dir(worktrees.join(".cargo/config.toml")).unwrap();
		fs::write(
			cargo_home.join("config.toml"),
			"[build]\ntarget-dir = \"home-target\"\n",
		)
		.unwrap();
		fs::write(
			in_tree_cargo_home.join("config.toml"),
			"[build]\ntarget-dir = \"../../escaped\"\n",
		)
		.unwrap();
		assert_eq!(
			operator_cargo_config_target(&project, &repo, true, Some(&cargo_home)),
			Some(base.join("home-target"))
		);
		assert_eq!(
			operator_cargo_config_target(&project, &repo, false, Some(&cargo_home)),
			Some(base.join("home-target"))
		);
		assert_eq!(
			operator_cargo_config_target(&project, &repo, true, Some(&worktrees.join(".cargo"))),
			None
		);
		assert_eq!(
			operator_cargo_config_target(&project, &repo, true, Some(&in_tree_cargo_home)),
			None
		);
		assert_eq!(
			operator_cargo_config_target(&project, &repo, true, None),
			None
		);
		fs::remove_file(cargo_home.join("config.toml")).unwrap();
		assert_eq!(
			operator_cargo_config_target(&project, &repo, true, Some(&cargo_home)),
			None
		);

		// Any in-tree override, include, malformed TOML, or non-file fails closed
		// even when a valid parent operator config or `$CARGO_HOME` exists,
		// including when an in-tree `config` is benign and `config.toml` is not.
		fs::write(
			worktrees.join(".cargo/config.toml"),
			"[build]\ntarget-dir = \"../shared-target\"\n",
		)
		.unwrap();
		fs::write(
			cargo_home.join("config.toml"),
			"[build]\ntarget-dir = \"home-target\"\n",
		)
		.unwrap();
		fs::write(
			project.join(".cargo/config"),
			"[alias]\ncheck = \"check\"\n",
		)
		.unwrap();
		for untrusted_toml in [
			"[build]\ntarget-dir = \"../../outside\"\n",
			"include = [\"extra.toml\"]\n",
			"not valid toml =\n",
		] {
			fs::write(project.join(".cargo/config.toml"), untrusted_toml).unwrap();
			assert_eq!(
				operator_cargo_config_target(&project, &repo, true, Some(&cargo_home)),
				None,
				"in-tree config must fail closed: {untrusted_toml}"
			);
			assert_eq!(
				operator_cargo_config_target(&project, &repo, false, Some(&cargo_home)),
				None,
				"in-tree config must fail closed without ancestor configs: {untrusted_toml}"
			);
		}
		fs::remove_file(project.join(".cargo/config")).unwrap();
		fs::remove_file(project.join(".cargo/config.toml")).unwrap();
		fs::create_dir(project.join(".cargo/config.toml")).unwrap();
		assert_eq!(
			operator_cargo_config_target(&project, &repo, true, Some(&cargo_home)),
			None
		);
	}

	#[cfg(unix)]
	#[test]
	fn in_tree_cargo_config_symlinks_fail_closed_and_paths_match_canonicalizes() {
		use std::os::unix::fs::symlink;

		let temp = TempDir::new().unwrap();
		let base = fs::canonicalize(temp.path()).unwrap();
		let worktrees = base.join("worktrees");
		let repo = worktrees.join("repo");
		let project = repo.join("project");
		let external = base.join("external");
		let in_tree_cargo_home = repo.join(".cargo-home");
		fs::create_dir_all(&project).unwrap();
		fs::create_dir_all(worktrees.join(".cargo")).unwrap();
		fs::create_dir_all(&external).unwrap();
		fs::create_dir_all(&in_tree_cargo_home).unwrap();
		fs::write(
			worktrees.join(".cargo/config.toml"),
			"[build]\ntarget-dir = \"../shared-target\"\n",
		)
		.unwrap();

		// Symlinked in-tree `.cargo` directory fails closed.
		symlink(&external, project.join(".cargo")).unwrap();
		assert_eq!(
			operator_cargo_config_target(&project, &repo, true, None),
			None
		);
		fs::remove_file(project.join(".cargo")).unwrap();

		// Symlinked in-tree `.cargo/config.toml` fails closed.
		fs::create_dir(project.join(".cargo")).unwrap();
		fs::write(external.join("config.toml"), "").unwrap();
		symlink(
			external.join("config.toml"),
			project.join(".cargo/config.toml"),
		)
		.unwrap();
		assert_eq!(
			operator_cargo_config_target(&project, &repo, true, None),
			None
		);

		// A symlinked `CARGO_HOME` outside the repo whose target is inside the
		// repo still fails closed via canonical containment.
		fs::remove_file(project.join(".cargo/config.toml")).unwrap();
		fs::remove_file(worktrees.join(".cargo/config.toml")).unwrap();
		fs::write(
			in_tree_cargo_home.join("config.toml"),
			"[build]\ntarget-dir = \"../../escaped\"\n",
		)
		.unwrap();
		let linked_cargo_home = base.join("linked-cargo-home");
		symlink(&in_tree_cargo_home, &linked_cargo_home).unwrap();
		assert_eq!(
			operator_cargo_config_target(&project, &repo, true, Some(&linked_cargo_home)),
			None
		);

		// `paths_match` handles lexical normalization, symlink canonicalization,
		// mismatches, and invalid paths traversing above the root.
		let alias = base.join("alias-external");
		symlink(&external, &alias).unwrap();
		assert!(paths_match(&worktrees.join("../external"), &external));
		assert!(paths_match(&alias, &external));
		assert!(!paths_match(&base.join("other"), &external));
		assert!(!paths_match(Path::new("/../above-root"), &external));
	}

	#[test]
	fn generation_mode_spellings_are_stable() {
		assert_eq!(GenerationMode::Auto.as_str(), "auto");
		assert_eq!(GenerationMode::Create.as_str(), "create");
		assert_eq!(GenerationMode::Update.as_str(), "update");
		assert_eq!(GenerationMode::Overwrite.as_str(), "overwrite");
	}

	fn discover_with_migrations(root: &Path, migrations: &str) {
		write_program(root, "counter");
		fs::write(
			root.join(CONFIG_FILE_NAME),
			format!("[project]\nprogram = \".\"\n\n[migrations]\n{migrations}"),
		)
		.unwrap_or_else(|error| panic!("failed to write config: {error}"));
	}

	#[test]
	fn workspace_pattern_matches_exact_glob_and_complex_patterns() {
		assert_eq!(
			workspace_pattern_matches("examples/counter", "examples/counter"),
			Some(true)
		);
		assert_eq!(
			workspace_pattern_matches("examples/counter/", "examples/counter"),
			Some(true)
		);
		assert_eq!(
			workspace_pattern_matches("examples/other", "examples/counter"),
			Some(false)
		);
		assert_eq!(
			workspace_pattern_matches("examples/*", "examples/counter"),
			Some(true)
		);
		assert_eq!(
			workspace_pattern_matches("examples/*", "crates/counter"),
			Some(false)
		);
		assert_eq!(
			workspace_pattern_matches("examples/*", "examples"),
			Some(false)
		);
		assert_eq!(
			workspace_pattern_matches("examples/*", "examples/nested/counter"),
			Some(false)
		);
		assert_eq!(
			workspace_pattern_matches("examples/*/sub", "examples/a/sub"),
			None
		);
		assert_eq!(workspace_pattern_matches("examples/?", "examples/a"), None);
	}

	#[test]
	fn fast_package_discovery_handles_standalone_workspace_and_fallbacks() {
		let temp = TempDir::new().unwrap();
		let root = fs::canonicalize(temp.path()).unwrap();

		// Standalone crate with custom [lib] name/path/crate-type.
		let standalone = root.join("standalone");
		fs::create_dir_all(standalone.join("src")).unwrap();
		fs::write(standalone.join("src/entry.rs"), "").unwrap();
		fs::write(
			standalone.join("Cargo.toml"),
			"[package]\nname = \"my-prog\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
			 [lib]\nname = \"custom_lib\"\npath = \"src/entry.rs\"\ncrate-type = [\"cdylib\", \"lib\"]\n",
		)
		.unwrap();
		let manifest = standalone.join("Cargo.toml");
		assert_eq!(
			find_ancestor_manifest(&standalone.join("src")),
			Some(manifest.clone())
		);
		let fast = try_discover_package_fast(&standalone, &manifest)
			.unwrap()
			.expect("standalone package should use fast path");
		assert_eq!(fast.package_name, "my-prog");
		assert_eq!(fast.library_name, "custom_lib");
		assert_eq!(fast.library_source, standalone.join("src/entry.rs"));
		assert_eq!(fast.library_crate_types, vec!["cdylib", "lib"]);
		assert_eq!(fast.workspace_root, standalone);

		// Package without an explicit [lib] table (implicit src/lib.rs) and a
		// package with autolib = false plus an explicit [lib] both use the fast
		// path.
		let implicit_lib = root.join("implicit_lib");
		fs::create_dir_all(implicit_lib.join("src")).unwrap();
		fs::write(implicit_lib.join("src/lib.rs"), "").unwrap();
		fs::write(
			implicit_lib.join("Cargo.toml"),
			"[package]\nname = \"implicit-prog\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
		)
		.unwrap();
		let implicit_fast =
			try_discover_package_fast(&implicit_lib, &implicit_lib.join("Cargo.toml"))
				.unwrap()
				.expect("implicit lib package should use fast path");
		assert_eq!(implicit_fast.library_name, "implicit_prog");
		assert_eq!(
			implicit_fast.library_source,
			implicit_lib.join("src/lib.rs")
		);
		assert_eq!(implicit_fast.library_crate_types, vec!["lib"]);

		let explicit_no_autolib = root.join("explicit_no_autolib");
		fs::create_dir_all(explicit_no_autolib.join("src")).unwrap();
		fs::write(explicit_no_autolib.join("src/lib.rs"), "").unwrap();
		fs::write(
			explicit_no_autolib.join("Cargo.toml"),
			"[package]\nname = \"explicit-prog\"\nversion = \"0.1.0\"\nautolib = false\n\n\
			 [lib]\npath = \"src/lib.rs\"\n",
		)
		.unwrap();
		assert!(
			try_discover_package_fast(
				&explicit_no_autolib,
				&explicit_no_autolib.join("Cargo.toml"),
			)
			.unwrap()
			.is_some()
		);

		// Package with own [workspace] uses its own directory as workspace root.
		let own_ws = root.join("own_ws");
		fs::create_dir_all(own_ws.join("src")).unwrap();
		fs::write(own_ws.join("src/lib.rs"), "").unwrap();
		fs::write(
			own_ws.join("Cargo.toml"),
			"[package]\nname = \"own-ws\"\nversion = \"0.1.0\"\n[workspace]\n",
		)
		.unwrap();
		assert_eq!(
			discover_workspace_root_from_manifest(&own_ws, &own_ws.join("Cargo.toml")),
			Some(own_ws.clone())
		);

		// Ancestor workspace member, excluded crate, unlisted crate, and complex exclude pattern.
		let ws_root = root.join("ws");
		let member = ws_root.join("members/alpha");
		let excluded = ws_root.join("excluded/beta");
		let unlisted = ws_root.join("other/gamma");
		for dir in [&member, &excluded, &unlisted] {
			fs::create_dir_all(dir.join("src")).unwrap();
			fs::write(dir.join("src/lib.rs"), "").unwrap();
			fs::write(
				dir.join("Cargo.toml"),
				"[package]\nname = \"pkg\"\nversion = \"0.1.0\"\n",
			)
			.unwrap();
		}
		fs::write(
			ws_root.join("Cargo.toml"),
			"[workspace]\nmembers = [\"members/*\"]\nexclude = [\"excluded/*\"]\n",
		)
		.unwrap();
		assert_eq!(
			discover_workspace_root_from_manifest(&member, &member.join("Cargo.toml")),
			Some(ws_root.clone())
		);
		assert_eq!(
			discover_workspace_root_from_manifest(&excluded, &excluded.join("Cargo.toml")),
			Some(excluded.clone())
		);
		assert_eq!(
			discover_workspace_root_from_manifest(&unlisted, &unlisted.join("Cargo.toml")),
			None
		);
		assert!(
			try_discover_package_fast(&unlisted, &unlisted.join("Cargo.toml"))
				.unwrap()
				.is_none()
		);

		// Complex exclude glob triggers fallback (None).
		fs::write(
			ws_root.join("Cargo.toml"),
			"[workspace]\nmembers = [\"members/*\"]\nexclude = [\"excluded/*/deep\"]\n",
		)
		.unwrap();
		assert_eq!(
			discover_workspace_root_from_manifest(&member, &member.join("Cargo.toml")),
			None
		);

		// Fallback cases: missing parent, unreadable, invalid TOML, virtual workspace,
		// autolib=false, non-lib crate-type, missing lib.rs, and .cargo/config.toml target-dir.
		assert!(
			try_discover_package_fast(&root, Path::new("/"))
				.unwrap()
				.is_none()
		);
		assert!(
			try_discover_package_fast(&root, &root.join("missing/Cargo.toml"))
				.unwrap()
				.is_none()
		);

		let invalid_toml = root.join("invalid");
		fs::create_dir_all(&invalid_toml).unwrap();
		fs::write(invalid_toml.join("Cargo.toml"), "not = [valid").unwrap();
		assert!(
			try_discover_package_fast(&invalid_toml, &invalid_toml.join("Cargo.toml"))
				.unwrap()
				.is_none()
		);

		let virtual_ws = root.join("virtual_ws");
		fs::create_dir_all(&virtual_ws).unwrap();
		fs::write(virtual_ws.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
		assert!(
			try_discover_package_fast(&virtual_ws, &virtual_ws.join("Cargo.toml"))
				.unwrap()
				.is_none()
		);

		let no_autolib = root.join("no_autolib");
		fs::create_dir_all(no_autolib.join("src")).unwrap();
		fs::write(no_autolib.join("src/lib.rs"), "").unwrap();
		fs::write(
			no_autolib.join("Cargo.toml"),
			"[package]\nname = \"x\"\nautolib = false\n",
		)
		.unwrap();
		assert!(
			try_discover_package_fast(&no_autolib, &no_autolib.join("Cargo.toml"))
				.unwrap()
				.is_none()
		);

		let proc_macro_only = root.join("proc_macro_only");
		fs::create_dir_all(proc_macro_only.join("src")).unwrap();
		fs::write(proc_macro_only.join("src/lib.rs"), "").unwrap();
		fs::write(
			proc_macro_only.join("Cargo.toml"),
			"[package]\nname = \"x\"\n[lib]\ncrate-type = [\"proc-macro\"]\n",
		)
		.unwrap();
		assert!(
			try_discover_package_fast(&proc_macro_only, &proc_macro_only.join("Cargo.toml"))
				.unwrap()
				.is_none()
		);

		let no_src = root.join("no_src");
		fs::create_dir_all(&no_src).unwrap();
		fs::write(no_src.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
		assert!(
			try_discover_package_fast(&no_src, &no_src.join("Cargo.toml"))
				.unwrap()
				.is_none()
		);

		let with_cfg = root.join("with_cfg");
		fs::create_dir_all(with_cfg.join("src")).unwrap();
		fs::create_dir_all(with_cfg.join(".cargo")).unwrap();
		fs::write(with_cfg.join("src/lib.rs"), "").unwrap();
		fs::write(with_cfg.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
		fs::write(
			with_cfg.join(".cargo/config.toml"),
			"[build]\ntarget-dir = \"out\"\n",
		)
		.unwrap();
		assert!(has_cargo_config_target_dir(&with_cfg));
		assert!(
			try_discover_package_fast(&with_cfg, &with_cfg.join("Cargo.toml"))
				.unwrap()
				.is_none()
		);

		assert!(find_ancestor_manifest(Path::new("/")).is_none());

		assert!(!is_custom_cargo_binary(None));
		assert!(!is_custom_cargo_binary(Some(std::ffi::OsStr::new(""))));
		assert!(!is_custom_cargo_binary(Some(std::ffi::OsStr::new("cargo"))));
		assert!(!is_custom_cargo_binary(Some(std::ffi::OsStr::new(
			"/usr/bin/cargo"
		))));
		assert!(!is_custom_cargo_binary(Some(std::ffi::OsStr::new(
			"C:/toolchain/cargo.exe"
		))));
		assert!(is_custom_cargo_binary(Some(std::ffi::OsStr::new(
			"/tmp/fake-cargo.sh"
		))));

		// Relative, absolute, and default target directory resolution.
		assert_eq!(
			resolve_target_directory(&root, &ws_root, None),
			ws_root.join("target")
		);
		assert_eq!(
			resolve_target_directory(&root, &ws_root, Some(OsString::from("custom-out"))),
			root.join("custom-out")
		);
		let abs_out = root.join("abs-out");
		assert_eq!(
			resolve_target_directory(&root, &ws_root, Some(abs_out.as_os_str().to_os_string())),
			abs_out
		);

		// Ancestor Cargo.toml without a [workspace] table is skipped while walking up.
		let parent_pkg = root.join("parent_pkg");
		let child_pkg = parent_pkg.join("child_pkg");
		fs::create_dir_all(child_pkg.join("src")).unwrap();
		fs::write(child_pkg.join("src/lib.rs"), "").unwrap();
		fs::write(
			parent_pkg.join("Cargo.toml"),
			"[package]\nname = \"parent\"\nversion = \"0.1.0\"\n",
		)
		.unwrap();
		fs::write(
			child_pkg.join("Cargo.toml"),
			"[package]\nname = \"child\"\nversion = \"0.1.0\"\n",
		)
		.unwrap();
		assert_eq!(
			discover_workspace_root_from_manifest(&child_pkg, &child_pkg.join("Cargo.toml")),
			Some(child_pkg)
		);

		// Explicit `package.workspace` declines the fast workspace shortcut.
		let external_ws = root.join("external_ws_member");
		fs::create_dir_all(external_ws.join("src")).unwrap();
		fs::write(external_ws.join("src/lib.rs"), "").unwrap();
		fs::write(
			external_ws.join("Cargo.toml"),
			"[package]\nname = \"ext\"\nversion = \"0.1.0\"\nworkspace = \"../workspace\"\n",
		)
		.unwrap();
		assert_eq!(
			discover_workspace_root_from_manifest(&external_ws, &external_ws.join("Cargo.toml")),
			None
		);

		// Default Cargo home and benign vs risky config handling for the fast
		// discovery path.
		let default_cargo = root.join("fake_user_home/.cargo");
		fs::create_dir_all(&default_cargo).unwrap();
		assert!(!has_cargo_config_target_dir_with_home(
			&standalone,
			Some(&default_cargo)
		));
		assert!(!has_cargo_config_target_dir_with_home(&standalone, None));

		// A benign regular config without target-dir or include keeps the fast path.
		fs::write(
			default_cargo.join("config.toml"),
			"[alias]\ncheck-all = \"check\"\n",
		)
		.unwrap();
		assert!(!has_cargo_config_target_dir_with_home(
			&standalone,
			Some(&default_cargo)
		));

		// target-dir, target_dir, include, non-UTF8, and non-file configs force
		// fallback to `cargo metadata`.
		fs::write(
			default_cargo.join("config.toml"),
			"[build]\ntarget-dir = \"/work/custom-target\"\n",
		)
		.unwrap();
		assert!(has_cargo_config_target_dir_with_home(
			&standalone,
			Some(&default_cargo)
		));
		fs::write(
			default_cargo.join("config.toml"),
			"include = [\"extra.toml\"]\n",
		)
		.unwrap();
		assert!(has_cargo_config_target_dir_with_home(
			&standalone,
			Some(&default_cargo)
		));
		fs::write(default_cargo.join("config.toml"), [0xff, 0xfe]).unwrap();
		assert!(has_cargo_config_target_dir_with_home(
			&standalone,
			Some(&default_cargo)
		));
		fs::remove_file(default_cargo.join("config.toml")).unwrap();
		fs::create_dir(default_cargo.join("config.toml")).unwrap();
		assert!(has_cargo_config_target_dir_with_home(
			&standalone,
			Some(&default_cargo)
		));
		fs::remove_dir(default_cargo.join("config.toml")).unwrap();
		fs::write(
			default_cargo.join("config"),
			"[build]\ntarget_dir = \"/work/custom-target\"\n",
		)
		.unwrap();
		assert!(has_cargo_config_target_dir_with_home(
			&standalone,
			Some(&default_cargo)
		));

		#[cfg(unix)]
		{
			let linked_dir = root.join("linked-cargo-dir");
			std::os::unix::fs::symlink(&default_cargo, &linked_dir).unwrap();
			assert!(cargo_config_dir_needs_metadata(&linked_dir));
		}
	}
}
