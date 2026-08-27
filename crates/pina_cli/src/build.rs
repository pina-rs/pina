//! Project-aware SBF and IDL build workflow.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs::File;
use std::fs::OpenOptions;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use atomic_write_file::AtomicWriteFile;

use crate::error::IdlError;
use crate::generate_idl;
use crate::project::Project;
use crate::project::ProjectError;

/// Outputs produced by [`build_project`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOutput {
	pub package_name: String,
	pub sbf_artifact: PathBuf,
	pub idl: PathBuf,
}

/// Structured inputs for a project SBF build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOptions {
	pub project_dir: PathBuf,
	pub features: Vec<String>,
	pub no_default_features: bool,
}

/// Errors produced by the project build workflow.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
	#[error(transparent)]
	Project(#[from] ProjectError),

	#[error("Failed to run `{command}`: {source}")]
	RunCargo {
		command: String,
		source: std::io::Error,
	},

	#[error("`{command}` failed ({status})")]
	CargoFailed { command: String, status: String },

	#[error("IDL generation failed for `{package}`: {source}")]
	GenerateIdl { package: String, source: IdlError },

	#[error("Failed to serialize the `{package}` IDL: {source}")]
	SerializeIdl {
		package: String,
		source: serde_json::Error,
	},

	#[error("Failed to create IDL directory {path}: {source}")]
	CreateIdlDir {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Failed to write IDL to {path}: {source}")]
	WriteIdl {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Cargo completed successfully but the compiler SBF artifact was not created: {path}")]
	MissingArtifact { path: PathBuf },

	#[error("Failed to create artifact directory {path}: {source}")]
	CreateArtifactDir {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Failed to stage artifact from {source_path}: {source}")]
	StageArtifact {
		source_path: PathBuf,
		source: std::io::Error,
	},

	#[error("Failed to publish artifact to {path}: {source}")]
	PublishArtifact {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Failed to restore {path} after artifact publication failed ({publish}): {rollback}")]
	RollbackIdl {
		path: PathBuf,
		publish: std::io::Error,
		rollback: std::io::Error,
	},

	#[error("Failed to read existing IDL at {path} before publication: {source}")]
	ReadPreviousIdl {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Failed to lock build outputs using {path}: {source}")]
	LockPublication {
		path: PathBuf,
		source: std::io::Error,
	},
}

/// Build the discovered program for SBF and write its Codama IDL.
///
/// The standard `CARGO` environment variable can select a Cargo-compatible
/// executable. This keeps the workflow compatible with Rust toolchain wrappers
/// and deterministic test harnesses.
///
/// # Errors
///
/// Returns an error when discovery, the SBF build, IDL extraction, or writing
/// the generated IDL fails.
pub fn build_project(start: &Path) -> Result<BuildOutput, BuildError> {
	build_project_with_options(&BuildOptions {
		project_dir: start.to_path_buf(),
		features: Vec::new(),
		no_default_features: false,
	})
}

/// Build a project with explicit Cargo feature selection.
///
/// `bpf-entrypoint` is always enabled and deduplicated from caller-provided
/// features.
///
/// # Errors
///
/// Returns the same discovery, Cargo, extraction, and publication errors as
/// [`build_project`].
pub fn build_project_with_options(options: &BuildOptions) -> Result<BuildOutput, BuildError> {
	let project = Project::discover(&options.project_dir)?;
	let manifest_path = project.program_dir.join("Cargo.toml");
	let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
	let features = options
		.features
		.iter()
		.map(String::as_str)
		.chain(std::iter::once("bpf-entrypoint"))
		.collect::<BTreeSet<_>>()
		.into_iter()
		.collect::<Vec<_>>()
		.join(",");
	let args = cargo_build_args(&project, &manifest_path, &features, options);
	let command_label = command_label(&cargo, &args);
	let mut command = Command::new(&cargo);
	command.current_dir(&project.root).args(&args);

	let status = command.status().map_err(|source| {
		BuildError::RunCargo {
			command: command_label.clone(),
			source,
		}
	})?;

	if !status.success() {
		return Err(BuildError::CargoFailed {
			command: command_label,
			status: status.to_string(),
		});
	}

	let compiler_artifact = project
		.target_dir
		.join("bpfel-unknown-none/release")
		.join(format!("lib{}.so", project.library_name));

	if !compiler_artifact.is_file() {
		return Err(BuildError::MissingArtifact {
			path: compiler_artifact,
		});
	}

	let idl =
		generate_idl(&project.program_dir, Some(&project.library_name)).map_err(|source| {
			BuildError::GenerateIdl {
				package: project.package_name.clone(),
				source,
			}
		})?;
	let json = serde_json::to_string_pretty(&idl)
		.map_err(|source| serialize_idl_error(&project.package_name, source))?;

	std::fs::create_dir_all(&project.idl_dir).map_err(|source| {
		BuildError::CreateIdlDir {
			path: project.idl_dir.clone(),
			source,
		}
	})?;

	let deploy_dir = project.target_dir.join("deploy");
	std::fs::create_dir_all(&deploy_dir).map_err(|source| {
		BuildError::CreateArtifactDir {
			path: deploy_dir.clone(),
			source,
		}
	})?;
	let idl_path = project
		.idl_dir
		.join(format!("{}.json", project.library_name));
	let sbf_artifact = deploy_dir.join(format!("{}.so", project.library_name));
	publish_outputs(
		&project.target_dir.join(".pina-build.lock"),
		&idl_path,
		json.as_bytes(),
		&compiler_artifact,
		&sbf_artifact,
	)?;

	Ok(BuildOutput {
		package_name: project.package_name,
		sbf_artifact,
		idl: idl_path,
	})
}

fn cargo_build_args(
	project: &Project,
	manifest_path: &Path,
	features: &str,
	options: &BuildOptions,
) -> Vec<OsString> {
	let mut args = vec![
		OsString::from("build"),
		OsString::from("--release"),
		OsString::from("--target"),
		OsString::from("bpfel-unknown-none"),
		OsString::from("--target-dir"),
		project.target_dir.as_os_str().to_owned(),
		OsString::from("--manifest-path"),
		manifest_path.as_os_str().to_owned(),
		OsString::from("-p"),
		OsString::from(&project.package_name),
		OsString::from("-Z"),
		OsString::from("build-std=core,alloc"),
		OsString::from("--features"),
		OsString::from(features),
	];

	if options.no_default_features {
		args.push(OsString::from("--no-default-features"));
	}

	args
}

fn command_label(cargo: &OsStr, args: &[OsString]) -> String {
	std::iter::once(cargo)
		.chain(args.iter().map(OsString::as_os_str))
		.map(debug_argument)
		.collect::<Vec<_>>()
		.join(" ")
}

#[allow(clippy::unnecessary_debug_formatting)]
fn debug_argument(argument: &OsStr) -> String {
	// Debug formatting preserves argument boundaries and escapes non-Unicode bytes.
	format!("{argument:?}")
}

fn serialize_idl_error(package: &str, source: serde_json::Error) -> BuildError {
	BuildError::SerializeIdl {
		package: package.to_owned(),
		source,
	}
}

fn publish_outputs(
	lock_path: &Path,
	idl_path: &Path,
	idl: &[u8],
	compiler_artifact: &Path,
	sbf_artifact: &Path,
) -> Result<(), BuildError> {
	use std::io::Write;

	let _lock = acquire_publication_lock(lock_path)?;
	let previous_idl = match std::fs::read(idl_path) {
		Ok(contents) => Some(contents),
		Err(source) if source.kind() == std::io::ErrorKind::NotFound => None,
		Err(source) => {
			return Err(BuildError::ReadPreviousIdl {
				path: idl_path.to_path_buf(),
				source,
			});
		}
	};
	let mut idl_file = AtomicWriteFile::open(idl_path).map_err(|source| {
		BuildError::WriteIdl {
			path: idl_path.to_path_buf(),
			source,
		}
	})?;
	idl_file
		.write_all(idl)
		.map_err(|source| write_idl_error(idl_path, source))?;
	let mut source_file = File::open(compiler_artifact).map_err(|source| {
		BuildError::StageArtifact {
			source_path: compiler_artifact.to_path_buf(),
			source,
		}
	})?;
	let mut artifact_file = AtomicWriteFile::open(sbf_artifact).map_err(|source| {
		BuildError::StageArtifact {
			source_path: compiler_artifact.to_path_buf(),
			source,
		}
	})?;
	std::io::copy(&mut source_file, &mut artifact_file).map_err(|source| {
		BuildError::StageArtifact {
			source_path: compiler_artifact.to_path_buf(),
			source,
		}
	})?;

	idl_file
		.commit()
		.map_err(|source| write_idl_error(idl_path, source))?;

	if let Err(publish) = artifact_file.commit() {
		return Err(handle_publish_failure(
			idl_path,
			sbf_artifact,
			previous_idl.as_deref(),
			publish,
		));
	}

	Ok(())
}

#[derive(Debug)]
struct PublicationLock(File);

impl Drop for PublicationLock {
	fn drop(&mut self) {
		let _ = fs2::FileExt::unlock(&self.0);
	}
}

fn acquire_publication_lock(path: &Path) -> Result<PublicationLock, BuildError> {
	let file = OpenOptions::new()
		.read(true)
		.write(true)
		.create(true)
		.truncate(false)
		.open(path)
		.map_err(|source| publication_lock_error(path, source))?;
	fs2::FileExt::lock_exclusive(&file).map_err(|source| publication_lock_error(path, source))?;

	Ok(PublicationLock(file))
}

fn publication_lock_error(path: &Path, source: std::io::Error) -> BuildError {
	BuildError::LockPublication {
		path: path.to_path_buf(),
		source,
	}
}

fn handle_publish_failure(
	idl_path: &Path,
	sbf_artifact: &Path,
	previous_idl: Option<&[u8]>,
	publish: std::io::Error,
) -> BuildError {
	match restore_idl(idl_path, previous_idl) {
		Ok(()) => {
			BuildError::PublishArtifact {
				path: sbf_artifact.to_path_buf(),
				source: publish,
			}
		}
		Err(rollback) => rollback_idl_error(idl_path, publish, rollback),
	}
}

fn write_idl_error(path: &Path, source: std::io::Error) -> BuildError {
	BuildError::WriteIdl {
		path: path.to_path_buf(),
		source,
	}
}

fn rollback_idl_error(
	path: &Path,
	publish: std::io::Error,
	rollback: std::io::Error,
) -> BuildError {
	BuildError::RollbackIdl {
		path: path.to_path_buf(),
		publish,
		rollback,
	}
}

fn restore_idl(path: &Path, previous: Option<&[u8]>) -> std::io::Result<()> {
	use std::io::Write;

	let Some(previous) = previous else {
		return match std::fs::remove_file(path) {
			Ok(()) => Ok(()),
			Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
			Err(source) => Err(source),
		};
	};
	let mut file = AtomicWriteFile::open(path)?;
	file.write_all(previous)?;
	file.commit()
}

#[cfg(test)]
mod tests {
	use std::fs;

	use tempfile::TempDir;

	use super::*;

	#[test]
	fn build_wrapper_forwards_discovery_errors() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let error = build_project(&temp.path().join("missing"))
			.expect_err("missing project should fail discovery");

		assert!(matches!(error, BuildError::Project(_)));
	}

	#[test]
	fn publication_replaces_both_outputs_repeatedly() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let idl = temp.path().join("program.json");
		let compiler = temp.path().join("compiler.so");
		let artifact = temp.path().join("program.so");
		let lock = temp.path().join("build.lock");
		fs::write(&compiler, b"first artifact")
			.unwrap_or_else(|error| panic!("failed to write compiler artifact: {error}"));

		publish_outputs(&lock, &idl, b"first idl", &compiler, &artifact)
			.unwrap_or_else(|error| panic!("initial publish failed: {error}"));
		fs::write(&compiler, b"second artifact")
			.unwrap_or_else(|error| panic!("failed to replace compiler artifact: {error}"));
		publish_outputs(&lock, &idl, b"second idl", &compiler, &artifact)
			.unwrap_or_else(|error| panic!("repeat publish failed: {error}"));

		assert_eq!(fs::read(&idl).unwrap_or_default(), b"second idl");
		assert_eq!(fs::read(&artifact).unwrap_or_default(), b"second artifact");
	}

	#[test]
	fn publication_lock_excludes_another_publisher() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let lock_path = temp.path().join("build.lock");
		let first = acquire_publication_lock(&lock_path)
			.unwrap_or_else(|error| panic!("failed to acquire first lock: {error}"));
		let second = OpenOptions::new()
			.read(true)
			.write(true)
			.open(&lock_path)
			.unwrap_or_else(|error| panic!("failed to open second lock handle: {error}"));

		assert!(fs2::FileExt::try_lock_exclusive(&second).is_err());
		drop(first);
		fs2::FileExt::try_lock_exclusive(&second)
			.unwrap_or_else(|error| panic!("lock should be released: {error}"));
	}

	#[test]
	fn publication_lock_errors_preserve_the_lock_path() {
		let error = acquire_publication_lock(Path::new("missing-parent/build.lock"))
			.expect_err("missing lock parent should fail");
		assert!(matches!(error, BuildError::LockPublication { .. }));

		let error = publication_lock_error(
			Path::new("build.lock"),
			std::io::Error::other("lock failure"),
		);
		assert!(error.to_string().contains("build.lock"));
	}

	#[test]
	fn publication_reports_staging_and_destination_errors() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let idl = temp.path().join("program.json");
		let artifact = temp.path().join("program.so");
		let missing = temp.path().join("missing.so");
		let lock = temp.path().join("build.lock");
		assert!(matches!(
			publish_outputs(&lock, &idl, b"idl", &missing, &artifact),
			Err(BuildError::StageArtifact { .. })
		));

		let compiler = temp.path().join("compiler.so");
		fs::write(&compiler, b"artifact")
			.unwrap_or_else(|error| panic!("failed to write compiler artifact: {error}"));
		let missing_parent = temp.path().join("missing/program.so");
		assert!(matches!(
			publish_outputs(&lock, &idl, b"idl", &compiler, &missing_parent),
			Err(BuildError::StageArtifact { .. })
		));

		let idl_missing_parent = temp.path().join("idl/program.json");
		assert!(matches!(
			publish_outputs(&lock, &idl_missing_parent, b"idl", &compiler, &artifact),
			Err(BuildError::WriteIdl { .. })
		));

		let compiler_directory = temp.path().join("compiler-directory");
		fs::create_dir(&compiler_directory)
			.unwrap_or_else(|error| panic!("failed to create compiler directory: {error}"));
		assert!(matches!(
			publish_outputs(&lock, &idl, b"idl", &compiler_directory, &artifact),
			Err(BuildError::StageArtifact { .. })
		));
	}

	#[test]
	fn publication_rolls_back_the_idl_when_artifact_commit_fails() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let idl = temp.path().join("program.json");
		let compiler = temp.path().join("compiler.so");
		let artifact = temp.path().join("artifact-directory");
		let lock = temp.path().join("build.lock");
		fs::write(&idl, b"previous idl")
			.unwrap_or_else(|error| panic!("failed to write previous IDL: {error}"));
		fs::write(&compiler, b"artifact")
			.unwrap_or_else(|error| panic!("failed to write compiler artifact: {error}"));
		fs::create_dir(&artifact)
			.unwrap_or_else(|error| panic!("failed to create artifact directory: {error}"));

		let error = publish_outputs(&lock, &idl, b"new idl", &compiler, &artifact)
			.expect_err("publishing over a directory should fail");

		assert!(matches!(error, BuildError::PublishArtifact { .. }));
		assert_eq!(fs::read(&idl).unwrap_or_default(), b"previous idl");
	}

	#[test]
	fn restore_idl_handles_present_absent_and_missing_previous_outputs() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let idl = temp.path().join("program.json");
		fs::write(&idl, b"current")
			.unwrap_or_else(|error| panic!("failed to write current IDL: {error}"));

		restore_idl(&idl, Some(b"previous"))
			.unwrap_or_else(|error| panic!("failed to restore previous IDL: {error}"));
		assert_eq!(fs::read(&idl).unwrap_or_default(), b"previous");
		restore_idl(&idl, None).unwrap_or_else(|error| panic!("failed to remove new IDL: {error}"));
		assert!(!idl.exists());
		restore_idl(&idl, None)
			.unwrap_or_else(|error| panic!("missing IDL should already be restored: {error}"));

		let directory = temp.path().join("directory");
		fs::create_dir(&directory)
			.unwrap_or_else(|error| panic!("failed to create directory: {error}"));
		assert!(restore_idl(&directory, None).is_err());
	}

	#[test]
	fn publication_rejects_an_unreadable_existing_idl() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let idl = temp.path().join("idl-directory");
		let compiler = temp.path().join("compiler.so");
		let artifact = temp.path().join("program.so");
		let lock = temp.path().join("build.lock");
		fs::create_dir(&idl)
			.unwrap_or_else(|error| panic!("failed to create IDL directory: {error}"));
		fs::write(&compiler, b"artifact")
			.unwrap_or_else(|error| panic!("failed to write compiler artifact: {error}"));

		assert!(matches!(
			publish_outputs(&lock, &idl, b"idl", &compiler, &artifact),
			Err(BuildError::ReadPreviousIdl { .. })
		));
	}

	#[test]
	fn publication_error_helpers_preserve_context() {
		let write = write_idl_error(Path::new("program.json"), std::io::Error::other("write"));
		assert!(matches!(write, BuildError::WriteIdl { .. }));
		let rollback = rollback_idl_error(
			Path::new("program.json"),
			std::io::Error::other("publish"),
			std::io::Error::other("rollback"),
		);
		assert!(matches!(rollback, BuildError::RollbackIdl { .. }));

		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let directory = temp.path().join("idl-directory");
		fs::create_dir(&directory)
			.unwrap_or_else(|error| panic!("failed to create IDL directory: {error}"));
		let rollback = handle_publish_failure(
			&directory,
			Path::new("program.so"),
			None,
			std::io::Error::other("publish"),
		);
		assert!(matches!(rollback, BuildError::RollbackIdl { .. }));
	}

	#[test]
	fn idl_serialization_error_preserves_package_context() {
		let error = serialize_idl_error(
			"counter",
			serde_json::Error::io(std::io::Error::other("serializer failure")),
		);

		assert!(matches!(error, BuildError::SerializeIdl { .. }));
		assert!(error.to_string().contains("counter"));
	}
}
