//! Project-aware SBF and IDL build workflow.

use std::collections::BTreeSet;
use std::ffi::OsString;
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
	let command_label = format!(
		"{} build --release --target bpfel-unknown-none -p {} -Z build-std --features {}",
		PathBuf::from(&cargo).display(),
		project.package_name,
		features
	);
	let mut command = Command::new(&cargo);
	command
		.current_dir(&project.root)
		.arg("build")
		.arg("--release")
		.arg("--target")
		.arg("bpfel-unknown-none")
		.arg("--manifest-path")
		.arg(&manifest_path)
		.arg("-p")
		.arg(&project.package_name)
		.arg("-Z")
		.arg("build-std")
		.arg("--features")
		.arg(&features);

	if options.no_default_features {
		command.arg("--no-default-features");
	}

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

	let idl = generate_idl(&project.program_dir, None).map_err(|source| {
		BuildError::GenerateIdl {
			package: project.package_name.clone(),
			source,
		}
	})?;
	let json = serde_json::to_string_pretty(&idl).map_err(|source| {
		BuildError::SerializeIdl {
			package: project.package_name.clone(),
			source,
		}
	})?;

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

fn publish_outputs(
	idl_path: &Path,
	idl: &[u8],
	compiler_artifact: &Path,
	sbf_artifact: &Path,
) -> Result<(), BuildError> {
	use std::io::Write;

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
	idl_file.write_all(idl).map_err(|source| {
		BuildError::WriteIdl {
			path: idl_path.to_path_buf(),
			source,
		}
	})?;
	let mut source_file = std::fs::File::open(compiler_artifact).map_err(|source| {
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

	idl_file.commit().map_err(|source| {
		BuildError::WriteIdl {
			path: idl_path.to_path_buf(),
			source,
		}
	})?;

	if let Err(publish) = artifact_file.commit() {
		if let Err(rollback) = restore_idl(idl_path, previous_idl.as_deref()) {
			return Err(BuildError::RollbackIdl {
				path: idl_path.to_path_buf(),
				publish,
				rollback,
			});
		}

		return Err(BuildError::PublishArtifact {
			path: sbf_artifact.to_path_buf(),
			source: publish,
		});
	}

	Ok(())
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
