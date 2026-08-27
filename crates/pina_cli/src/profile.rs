//! Project-aware SBF artifact discovery for profiling.

use std::path::Path;
use std::path::PathBuf;

use atomic_write_file::AtomicWriteFile;
use same_file::is_same_file;

use crate::project::Project;
use crate::project::ProjectError;

/// Profile input discovery failures.
#[derive(Debug, thiserror::Error)]
pub enum ProfileInputError {
	/// Project discovery failed.
	#[error(transparent)]
	Project(#[from] ProjectError),

	/// The conventional SBF artifact has not been built.
	#[error("compiled SBF artifact not found at {path:?}; build the program before profiling")]
	ArtifactNotFound { path: PathBuf },

	/// Profiling output must not alias or traverse links to the input program.
	#[error("refusing unsafe profile output {output:?}: {reason:?}")]
	UnsafeOutput { output: PathBuf, reason: String },

	/// Profile formatting failed before publication.
	#[error("failed to format profile output: {0}")]
	Format(#[from] pina_profile::output::OutputError),

	/// Atomic output publication failed.
	#[error("failed to publish profile output at {path:?}: {source}")]
	Publish {
		path: PathBuf,
		source: std::io::Error,
	},
}

/// Validate that an output path cannot truncate or replace the input program.
pub fn validate_profile_output(input: &Path, output: &Path) -> Result<(), ProfileInputError> {
	let link_result = crate::path_security::has_link_like_component(output);
	let has_link = inspect_output(output, link_result)?;

	if has_link {
		return Err(ProfileInputError::UnsafeOutput {
			output: output.to_path_buf(),
			reason: "path contains a symbolic link or reparse point".to_owned(),
		});
	}

	if output.exists()
		&& is_same_file(input, output).map_err(|error| {
			ProfileInputError::UnsafeOutput {
				output: output.to_path_buf(),
				reason: format!("failed to compare with input: {error}"),
			}
		})? {
		return Err(ProfileInputError::UnsafeOutput {
			output: output.to_path_buf(),
			reason: "path resolves to the input program".to_owned(),
		});
	}

	Ok(())
}

/// Atomically publish a profile report after validating the output path.
pub fn write_profile_output(
	profile: &pina_profile::ProgramProfile,
	format: pina_profile::OutputFormat,
	input: &Path,
	output: &Path,
) -> Result<(), ProfileInputError> {
	validate_profile_output(input, output)?;
	let mut file = publish_result(output, AtomicWriteFile::open(output))?;
	pina_profile::output::write_profile(profile, format, &mut file)?;
	publish_result(output, file.commit())
}

fn inspect_output<T>(output: &Path, result: std::io::Result<T>) -> Result<T, ProfileInputError> {
	result.map_err(|error| {
		ProfileInputError::UnsafeOutput {
			output: output.to_path_buf(),
			reason: error.to_string(),
		}
	})
}

fn publish_result<T>(output: &Path, result: std::io::Result<T>) -> Result<T, ProfileInputError> {
	result.map_err(|source| {
		ProfileInputError::Publish {
			path: output.to_path_buf(),
			source,
		}
	})
}

/// Resolve an explicit binary or discover the current project's SBF artifact.
pub fn resolve_profile_input(
	explicit: Option<&Path>,
	project_start: &Path,
) -> Result<PathBuf, ProfileInputError> {
	if let Some(path) = explicit {
		return Ok(path.to_path_buf());
	}

	let project = Project::discover(project_start)?;
	let artifact = project.sbf_artifact();

	if !artifact.is_file() {
		return Err(ProfileInputError::ArtifactNotFound { path: artifact });
	}

	Ok(artifact)
}

#[cfg(test)]
mod tests {
	use std::fs;

	use pina_profile::ProgramProfile;
	use tempfile::TempDir;

	use super::*;

	#[test]
	fn explicit_profile_input_preserves_legacy_behavior() {
		let path = Path::new("target/deploy/custom.so");

		assert_eq!(
			resolve_profile_input(Some(path), Path::new("/missing"))
				.unwrap_or_else(|error| panic!("explicit path failed: {error}")),
			path
		);
	}

	#[test]
	fn discovers_conventional_project_artifact() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let artifact = temp.path().join("target/deploy/demo_program.so");
		fs::create_dir_all(artifact.parent().expect("artifact has parent"))
			.unwrap_or_else(|error| panic!("create failed: {error}"));
		fs::create_dir_all(temp.path().join("src"))
			.unwrap_or_else(|error| panic!("source create failed: {error}"));
		fs::write(
			temp.path().join("Cargo.toml"),
			"[package]\nname = \"demo-program\"\n",
		)
		.unwrap_or_else(|error| panic!("manifest write failed: {error}"));
		fs::write(temp.path().join("src/lib.rs"), "")
			.unwrap_or_else(|error| panic!("source write failed: {error}"));
		fs::write(&artifact, []).unwrap_or_else(|error| panic!("artifact write failed: {error}"));

		let resolved = resolve_profile_input(None, temp.path())
			.unwrap_or_else(|error| panic!("discovery failed: {error}"));
		assert!(
			is_same_file(&resolved, &artifact)
				.unwrap_or_else(|error| panic!("artifact identity failed: {error}"))
		);
	}

	#[test]
	fn validates_safe_missing_output_and_reports_comparison_failures() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"));
		let input = root.join("input.so");
		let output = root.join("output.json");
		fs::write(&input, []).unwrap_or_else(|error| panic!("input write failed: {error}"));

		validate_profile_output(&input, &output)
			.unwrap_or_else(|error| panic!("safe output rejected: {error}"));

		let missing_input = root.join("missing.so");
		fs::write(&output, []).unwrap_or_else(|error| panic!("output write failed: {error}"));
		let error = validate_profile_output(&missing_input, &output)
			.expect_err("missing input comparison must fail closed");
		assert!(matches!(error, ProfileInputError::UnsafeOutput { .. }));

		let inspection_error = inspect_output::<bool>(
			&output,
			Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
		)
		.expect_err("path inspection failures must fail closed");
		assert!(matches!(
			inspection_error,
			ProfileInputError::UnsafeOutput { .. }
		));
	}

	#[test]
	fn profile_output_reports_publication_failure_without_touching_input() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"));
		let input = root.join("input.so");
		let output = root.join("output-directory");
		fs::write(&input, b"program").unwrap_or_else(|error| panic!("input write failed: {error}"));
		fs::create_dir(&output).unwrap_or_else(|error| panic!("output create failed: {error}"));
		let profile = ProgramProfile {
			program_name: "demo".to_owned(),
			binary_size: 7,
			text_size: 0,
			total_instructions: 0,
			total_syscalls: 0,
			total_cu: 0,
			functions: Vec::new(),
		};

		let error =
			write_profile_output(&profile, pina_profile::OutputFormat::Json, &input, &output)
				.expect_err("directory output must fail");

		assert!(matches!(error, ProfileInputError::Publish { .. }));
		assert_eq!(
			fs::read(&input).unwrap_or_else(|read_error| panic!("input read failed: {read_error}")),
			b"program"
		);
	}

	#[test]
	fn profile_errors_escape_control_characters_in_paths_and_reasons() {
		let error = ProfileInputError::UnsafeOutput {
			output: PathBuf::from("profile\n\u{1b}[31m.json"),
			reason: "reason\n\u{1b}[31m".to_owned(),
		}
		.to_string();

		assert!(!error.contains('\u{1b}'));
		assert!(!error.contains('\n'));
		assert!(error.contains("\\n"));
		assert!(error.contains("\\u{1b}"));
	}
}
