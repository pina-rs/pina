//! Project-local configuration and Cargo workspace discovery.

use std::ffi::OsString;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use cargo_metadata::Metadata;
use cargo_metadata::MetadataCommand;
use cargo_metadata::Package;
use cargo_metadata::TargetKind;
use serde::Deserialize;
use serde::Serialize;

/// Name of the project-local Pina configuration file.
pub const CONFIG_FILE_NAME: &str = "Pina.toml";

/// A generated client ecosystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientLanguage {
	Rust,
	Typescript,
	Dart,
}

impl ClientLanguage {
	/// Return the stable command-line and configuration spelling.
	#[must_use]
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Rust => "rust",
			Self::Typescript => "typescript",
			Self::Dart => "dart",
		}
	}
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
	pub target_dir: PathBuf,
	pub idl_dir: PathBuf,
	pub clients_dir: PathBuf,
	pub clients: Vec<ClientLanguage>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectConfig {
	#[serde(default)]
	project: ProgramConfig,
	#[serde(default)]
	clients: ClientsConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ProgramConfig {
	program: PathBuf,
	idl_dir: Option<PathBuf>,
}

impl Default for ProgramConfig {
	fn default() -> Self {
		Self {
			program: PathBuf::from("."),
			idl_dir: None,
		}
	}
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ClientsConfig {
	output: PathBuf,
	languages: Vec<ClientLanguage>,
}

impl Default for ClientsConfig {
	fn default() -> Self {
		Self {
			output: PathBuf::from("clients"),
			languages: vec![ClientLanguage::Rust, ClientLanguage::Typescript],
		}
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

	#[error("Invalid `{field}` path `{path}` in Pina.toml: {reason}")]
	InvalidConfigPath {
		field: &'static str,
		path: PathBuf,
		reason: &'static str,
	},

	#[error("Cargo metadata discovery failed from {path}: {message}")]
	CargoMetadata { path: PathBuf, message: String },

	#[error(
		"Could not choose a program from the Cargo workspace at {path}. Run from a package \
		 directory or add Pina.toml. Candidates: {candidates}"
	)]
	AmbiguousWorkspace { path: PathBuf, candidates: String },

	#[error("Cargo package {package} does not define a Rust library target")]
	MissingLibraryTarget { package: String },
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

	/// Discover a project from `start`, preferring the nearest `Pina.toml` and
	/// falling back to Cargo metadata for existing projects.
	///
	/// # Errors
	///
	/// Returns an error when the start path cannot be inspected, configuration
	/// is invalid, or Cargo workspace discovery cannot identify one package.
	pub fn discover(start: &Path) -> Result<Self, ProjectError> {
		let start = normalize_start(start)?;

		if let Some(config_path) = find_ancestor_file(&start, CONFIG_FILE_NAME) {
			return Self::from_config(&config_path);
		}

		Self::from_cargo_metadata(&start)
	}

	fn from_config(config_path: &Path) -> Result<Self, ProjectError> {
		let root = config_path
			.parent()
			.map_or_else(|| PathBuf::from("."), Path::to_path_buf);
		let source = std::fs::read_to_string(config_path).map_err(|source| {
			ProjectError::ReadFile {
				path: config_path.to_path_buf(),
				source,
			}
		})?;
		let config: ProjectConfig = toml::from_str(&source).map_err(|source| {
			ProjectError::ParseToml {
				path: config_path.to_path_buf(),
				source,
			}
		})?;
		let configured_program =
			resolve_config_path(&root, "project.program", &config.project.program)?;
		let program_dir = std::fs::canonicalize(&configured_program).map_err(|source| {
			ProjectError::InspectPath {
				path: configured_program,
				source,
			}
		})?;
		if !program_dir.starts_with(&root) {
			return Err(ProjectError::InvalidConfigPath {
				field: "project.program",
				path: config.project.program,
				reason: "the resolved program directory must remain inside the project root",
			});
		}
		let manifest_path = program_dir.join("Cargo.toml");
		if !manifest_path.is_file() {
			return Err(ProjectError::MissingManifest {
				path: manifest_path,
			});
		}
		// Cargo resolves relative environment paths, including CARGO_TARGET_DIR,
		// against its working directory. Build commands use the configuration root,
		// so metadata must use the same root to report the artifact directory.
		let metadata = cargo_metadata(&root, Some(&manifest_path))?;
		let package = package_for_manifest(&metadata, &manifest_path, &root)?;
		let target_dir = metadata.target_directory.as_std_path().to_path_buf();
		let (library_name, library_source) = library_details(package)?;
		let idl_dir = config
			.project
			.idl_dir
			.as_deref()
			.map(|path| resolve_output_config_path(&root, "project.idl_dir", path))
			.transpose()?
			.unwrap_or_else(|| target_dir.join("idl"));
		let clients_dir =
			resolve_output_config_path(&root, "clients.output", &config.clients.output)?;

		Ok(Self {
			program_dir,
			package_name: package.name.to_string(),
			library_name,
			library_source,
			idl_dir,
			target_dir,
			clients_dir,
			clients: config.clients.languages,
			root,
		})
	}

	fn from_cargo_metadata(start: &Path) -> Result<Self, ProjectError> {
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

		let (library_name, library_source) = library_details(package)?;
		let root = program_dir.to_path_buf();
		let target_dir = metadata.target_directory.as_std_path().to_path_buf();

		Ok(Self {
			program_dir: root.clone(),
			package_name: package.name.to_string(),
			library_name,
			library_source,
			idl_dir: target_dir.join("idl"),
			target_dir,
			clients_dir: root.join("clients"),
			clients: ClientsConfig::default().languages,
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
		cargo_metadata(&self.root, Some(&manifest_path))
			.map(|metadata| metadata.workspace_root.as_std_path().to_path_buf())
	}
}

fn resolve_config_path(
	root: &Path,
	field: &'static str,
	path: &Path,
) -> Result<PathBuf, ProjectError> {
	if path.as_os_str().is_empty()
		|| path
			.components()
			.any(|component| !matches!(component, Component::CurDir | Component::Normal(_)))
	{
		return Err(ProjectError::InvalidConfigPath {
			field,
			path: path.to_path_buf(),
			reason: "paths must be non-empty, relative, and cannot contain `..`",
		});
	}

	Ok(root.join(path))
}

fn resolve_output_config_path(
	root: &Path,
	field: &'static str,
	path: &Path,
) -> Result<PathBuf, ProjectError> {
	let resolved = resolve_config_path(root, field, path)?;
	let mut current = root.to_path_buf();

	for component in path.components() {
		if component == Component::CurDir {
			continue;
		}
		current.push(component);
		match std::fs::symlink_metadata(&current) {
			Ok(metadata) if is_link_like(&metadata) => {
				return Err(ProjectError::InvalidConfigPath {
					field,
					path: path.to_path_buf(),
					reason: "configured output paths cannot traverse symbolic links",
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

	Ok(resolved)
}

fn is_link_like(metadata: &std::fs::Metadata) -> bool {
	if metadata.file_type().is_symlink() {
		return true;
	}

	#[cfg(windows)]
	{
		use std::os::windows::fs::MetadataExt;

		const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
		return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
	}

	#[cfg(not(windows))]
	false
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

fn library_details(package: &Package) -> Result<(String, PathBuf), ProjectError> {
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

	Ok((
		target.name.clone(),
		target.src_path.as_std_path().to_path_buf(),
	))
}

fn cargo_metadata(start: &Path, manifest_path: Option<&Path>) -> Result<Metadata, ProjectError> {
	let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
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

fn find_ancestor_file(start: &Path, name: &str) -> Option<PathBuf> {
	start
		.ancestors()
		.map(|ancestor| ancestor.join(name))
		.find(|candidate| candidate.is_file())
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
	fn config_rejects_absolute_and_parent_paths_for_every_path_field() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		write_program(temp.path(), "counter");
		let cases = [
			("project.program", "[project]\nprogram = \"/outside\"\n"),
			("project.program", "[project]\nprogram = \"../outside\"\n"),
			(
				"project.idl_dir",
				"[project]\nprogram = \".\"\nidl_dir = \"/outside\"\n",
			),
			(
				"project.idl_dir",
				"[project]\nprogram = \".\"\nidl_dir = \"../outside\"\n",
			),
			("clients.output", "[clients]\noutput = \"/outside\"\n"),
			("clients.output", "[clients]\noutput = \"../outside\"\n"),
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
		assert_eq!(
			resolve_output_config_path(&root, "clients.output", Path::new("."))
				.unwrap_or_else(|error| panic!("dot output should resolve: {error}")),
			root
		);
		let too_long = PathBuf::from("x".repeat(32 * 1024));
		assert!(matches!(
			resolve_output_config_path(&root, "clients.output", &too_long),
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
}
