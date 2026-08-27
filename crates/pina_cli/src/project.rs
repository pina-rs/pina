//! Project-local configuration and Cargo workspace discovery.

use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;

use cargo_metadata::MetadataCommand;
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
	pub root: PathBuf,
	pub program_dir: PathBuf,
	pub package_name: String,
	pub library_name: String,
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

#[derive(Debug, Deserialize)]
struct CargoManifest {
	package: CargoPackage,
	lib: Option<CargoLibrary>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
	name: String,
}

#[derive(Debug, Deserialize)]
struct CargoLibrary {
	name: Option<String>,
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

	#[error("Cargo metadata discovery failed from {path}: {message}")]
	CargoMetadata { path: PathBuf, message: String },

	#[error(
		"Could not choose a program from the Cargo workspace at {path}. Run from a package \
		 directory or add Pina.toml. Candidates: {candidates}"
	)]
	AmbiguousWorkspace { path: PathBuf, candidates: String },
}

impl Project {
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
		let configured_program = root.join(&config.project.program);
		let program_dir = std::fs::canonicalize(&configured_program).map_err(|source| {
			ProjectError::InspectPath {
				path: configured_program,
				source,
			}
		})?;
		let manifest_path = program_dir.join("Cargo.toml");
		let manifest = read_manifest(&manifest_path)?;
		let metadata = cargo_metadata(&program_dir, Some(&manifest_path))?;
		let target_dir = metadata.target_directory.as_std_path().to_path_buf();
		let library_name = manifest
			.lib
			.and_then(|lib| lib.name)
			.unwrap_or_else(|| manifest.package.name.replace('-', "_"));

		Ok(Self {
			program_dir,
			package_name: manifest.package.name,
			library_name,
			idl_dir: config
				.project
				.idl_dir
				.map_or_else(|| target_dir.join("idl"), |path| root.join(path)),
			target_dir,
			clients_dir: root.join(config.clients.output),
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

		let library_name = package
			.targets
			.iter()
			.find(|target| {
				target
					.kind
					.iter()
					.any(|kind| matches!(kind, TargetKind::Lib | TargetKind::CDyLib))
			})
			.map_or_else(
				|| package.name.replace('-', "_"),
				|target| target.name.clone(),
			);
		let root = program_dir.to_path_buf();
		let target_dir = metadata.target_directory.as_std_path().to_path_buf();

		Ok(Self {
			program_dir: root.clone(),
			package_name: package.name.to_string(),
			library_name,
			idl_dir: target_dir.join("idl"),
			target_dir,
			clients_dir: root.join("clients"),
			clients: ClientsConfig::default().languages,
			root,
		})
	}
}

fn cargo_metadata(
	start: &Path,
	manifest_path: Option<&Path>,
) -> Result<cargo_metadata::Metadata, ProjectError> {
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
	let metadata = std::fs::metadata(&absolute).map_err(|source| {
		ProjectError::InspectPath {
			path: absolute.clone(),
			source,
		}
	})?;

	if metadata.is_dir() {
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

fn read_manifest(path: &Path) -> Result<CargoManifest, ProjectError> {
	if !path.is_file() {
		return Err(ProjectError::MissingManifest {
			path: path.to_path_buf(),
		});
	}

	let source = std::fs::read_to_string(path).map_err(|source| {
		ProjectError::ReadFile {
			path: path.to_path_buf(),
			source,
		}
	})?;

	toml::from_str(&source).map_err(|source| {
		ProjectError::ParseToml {
			path: path.to_path_buf(),
			source,
		}
	})
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
	fn cargo_metadata_discovers_single_package_without_config() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		write_program(temp.path(), "existing-program");

		let project = Project::discover(temp.path())
			.unwrap_or_else(|error| panic!("discovery failed: {error}"));
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("failed to canonicalize root: {error}"));

		assert_eq!(project.program_dir, root);
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
}
