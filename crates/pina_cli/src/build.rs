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
pub use crate::verifiable::VerifiedBuildRecord;
use crate::verifiable::VerifyBuildError;
pub use crate::verifiable::VerifyBuildOptions;

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
	/// Release profile overrides applied to the SBF build.
	pub size_profile: SizeProfile,
}

/// Release profile applied to a deployed program build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SizeProfile {
	/// Fat LTO, `codegen-units = 1`, `opt-level = 3`, and overflow checks off.
	///
	/// Overflow checks are disabled because the flag lets arithmetic overflow
	/// wrap instead of panicking; use [`Self::ProductionWithOverflowChecks`]
	/// when a program must fail loudly.
	#[default]
	Production,
	/// The production profile, keeping arithmetic overflow checks enabled.
	ProductionWithOverflowChecks,
	/// The production profile with LTO explicitly disabled.
	///
	/// LTO is turned off as a positive override rather than by leaving the
	/// setting alone, so a manifest that declares `lto = "fat"` (which
	/// `pina init` generates) is still overridden.
	ProductionWithoutLto,
	/// Leave the program's own release profile untouched.
	None,
}

impl SizeProfile {
	/// Whether fat LTO should be requested for this profile.
	fn requests_lto(self) -> bool {
		matches!(self, Self::Production | Self::ProductionWithOverflowChecks)
	}

	/// Whether the profile disables LTO over a manifest that enables it.
	fn forbids_lto(self) -> bool {
		matches!(self, Self::ProductionWithoutLto)
	}

	/// Whether the profile wants overflow checks left enabled.
	fn keeps_overflow_checks(self) -> bool {
		matches!(self, Self::ProductionWithOverflowChecks)
	}

	/// Whether this profile writes any release overrides at all.
	fn applies_overrides(self) -> bool {
		!matches!(self, Self::None)
	}
}

/// Outputs produced by a deterministic Solana Verify build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedBuildOutput {
	pub build: BuildOutput,
	pub verifiable_artifact: PathBuf,
	pub verification_manifest: PathBuf,
}

/// Errors produced by the project build workflow.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
	#[error(transparent)]
	Project(#[from] ProjectError),

	#[error(transparent)]
	Verify(#[from] VerifyBuildError),

	#[error(transparent)]
	Migration(#[from] crate::migrations::MigrationError),

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
		size_profile: SizeProfile::default(),
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
	check_migrations_for_build(&project)?;
	if options.size_profile.requests_lto() {
		warn_lto_unavailable(&project);
	}
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
	let args = build_sbf_args(&project, &manifest_path, &features, options);
	let command_label = command_label(&cargo, &args);
	let mut command = Command::new(&cargo);
	command
		.current_dir(&project.root)
		.env("CARGO_TARGET_DIR", &project.target_dir)
		.args(&args);
	apply_size_profile(&mut command, options, declared_release_profile(&project));

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
		.join("sbf-build")
		.join(format!("{}.so", project.library_name));

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

/// Build a project deterministically through Solana Verify 0.5.1.
///
/// The ordinary build API remains unchanged. This function switches only the
/// SBF compiler backend, publishes a content-addressed Pina build record, and
/// then publishes the canonical deploy artifact and IDL.
///
/// # Errors
///
/// Returns an error when project discovery, source staging, Solana Verify,
/// IDL generation, hashing, or atomic file publication fails.
pub fn build_project_verified_with_options(
	options: &BuildOptions,
	verify: &VerifyBuildOptions,
) -> Result<VerifiedBuildOutput, BuildError> {
	let project = Project::discover(&options.project_dir)?;
	check_migrations_for_build(&project)?;
	warn_verified_profile_divergence(&project, options.size_profile);
	let features = options
		.features
		.iter()
		.map(String::as_str)
		.chain(std::iter::once("bpf-entrypoint"))
		.collect::<BTreeSet<_>>()
		.into_iter()
		.map(str::to_owned)
		.collect::<Vec<_>>();
	let verified =
		crate::verifiable::build(&project, &features, options.no_default_features, verify)?;
	publish_verified_build(&project, &verified)
}

fn check_migrations_for_build(project: &Project) -> Result<(), BuildError> {
	match crate::migrations::check_project_migrations(project) {
		Ok(_) => Ok(()),
		Err(crate::migrations::MigrationError::Parse(source)) => {
			Err(BuildError::GenerateIdl {
				package: project.package_name.clone(),
				source,
			})
		}
		Err(error) => Err(BuildError::Migration(error)),
	}
}

/// Read a Pina-local deterministic build record and verify its adjacent
/// content-addressed artifact before returning any provenance.
///
/// # Errors
///
/// Returns an error for aliases, malformed records, missing artifacts, or hash
/// mismatches.
pub fn read_verified_build_record(path: &Path) -> Result<VerifiedBuildRecord, VerifyBuildError> {
	crate::verifiable::read_record(path)
}

fn publish_verified_build(
	project: &Project,
	verified: &crate::verifiable::VerifiedBuild,
) -> Result<VerifiedBuildOutput, BuildError> {
	let manifest_path = crate::verifiable::publish_record(verified, &project.target_dir)?;
	let deploy_dir = project.target_dir.join("deploy");
	std::fs::create_dir_all(&project.idl_dir).map_err(|source| {
		BuildError::CreateIdlDir {
			path: project.idl_dir.clone(),
			source,
		}
	})?;
	std::fs::create_dir_all(&deploy_dir).map_err(|source| {
		BuildError::CreateArtifactDir {
			path: deploy_dir.clone(),
			source,
		}
	})?;
	let idl =
		generate_idl(&verified.program_dir, Some(&project.library_name)).map_err(|source| {
			BuildError::GenerateIdl {
				package: project.package_name.clone(),
				source,
			}
		})?;
	let json = serde_json::to_string_pretty(&idl)
		.map_err(|source| serialize_idl_error(&project.package_name, source))?;
	let idl_path = project
		.idl_dir
		.join(format!("{}.json", project.library_name));
	let canonical_artifact = deploy_dir.join(format!("{}.so", project.library_name));
	publish_outputs(
		&project.target_dir.join(".pina-build.lock"),
		&idl_path,
		json.as_bytes(),
		&verified.artifact,
		&canonical_artifact,
	)?;

	Ok(VerifiedBuildOutput {
		build: BuildOutput {
			package_name: project.package_name.clone(),
			sbf_artifact: canonical_artifact,
			idl: idl_path,
		},
		verifiable_artifact: manifest_path.with_extension("so"),
		verification_manifest: manifest_path,
	})
}

/// Applies the production release profile to an SBF build command.
///
/// The SBF toolchain honors Cargo profile environment overrides, so setting
/// them here gives every `pina build` the production profile without editing
/// the program's manifest. `lto` and `codegen-units` are pure size wins;
/// `overflow-checks` is a semantic switch because disabling it turns
/// arithmetic overflow from a panic into a wrap.
///
/// `declared` is what the workspace release profile already says. Environment
/// overrides beat `[profile.release]`, so a program that explicitly opts into
/// overflow checks keeps them: the profile may only disable the check when the
/// manifest expresses no opinion about it, or agrees with disabling it.
fn apply_size_profile(
	command: &mut Command,
	options: &BuildOptions,
	declared: DeclaredReleaseProfile,
) {
	if !options.size_profile.applies_overrides() {
		return;
	}

	if options.size_profile.requests_lto() {
		command.env("CARGO_PROFILE_RELEASE_LTO", "fat");
	}
	if options.size_profile.forbids_lto() {
		command.env("CARGO_PROFILE_RELEASE_LTO", "false");
	}
	let overflow_checks =
		options.size_profile.keeps_overflow_checks() || declared.overflow_checks == Some(true);
	if overflow_checks && !options.size_profile.keeps_overflow_checks() {
		warn_manifest_overflow_checks();
	}
	command
		.env("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "1")
		.env("CARGO_PROFILE_RELEASE_OPT_LEVEL", "3")
		.env(
			"CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS",
			if overflow_checks { "true" } else { "false" },
		);
}

/// Report that the manifest's overflow-check opt-in outranks the size profile.
fn warn_manifest_overflow_checks() {
	eprintln!(
		"warning: the release profile sets `overflow-checks = true`, so `pina build` keeps \
		 arithmetic overflow checks enabled and gives up the size profile's `overflow-checks = \
		 false` override. Pass `--no-size-profile` to leave the profile untouched, or drop the \
		 manifest setting to accept the size win."
	);
}

/// Release-profile settings declared by the manifest that owns the build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct DeclaredReleaseProfile {
	lto: Option<bool>,
	codegen_units: Option<u32>,
	overflow_checks: Option<bool>,
}

/// Read `[profile.release]` from the workspace manifest that owns the build.
///
/// Cargo applies `[profile]` tables from the workspace root manifest only, so
/// that is the file consulted. A missing file, table, or key all mean the
/// manifest expresses no opinion, which lets the size profile choose.
fn declared_release_profile(project: &Project) -> DeclaredReleaseProfile {
	let manifest_path = project.workspace_root().ok().map_or_else(
		|| project.program_dir.join("Cargo.toml"),
		|root| root.join("Cargo.toml"),
	);
	let Some(release) = std::fs::read_to_string(manifest_path)
		.ok()
		.and_then(|text| toml::from_str::<toml::Value>(&text).ok())
		.and_then(|manifest| manifest.get("profile")?.get("release").cloned())
	else {
		return DeclaredReleaseProfile::default();
	};

	DeclaredReleaseProfile {
		lto: release.get("lto").and_then(|value| {
			match value {
				toml::Value::Boolean(enabled) => Some(*enabled),
				toml::Value::String(name) => Some(name != "off" && name != "false"),
				_ => None,
			}
		}),
		codegen_units: release
			.get("codegen-units")
			.and_then(toml::Value::as_integer)
			.and_then(|value| u32::try_from(value).ok()),
		overflow_checks: release
			.get("overflow-checks")
			.and_then(toml::Value::as_bool),
	}
}

/// Describe why a verified build cannot carry the requested size profile.
///
/// Solana Verify rebuilds from the recorded Git revision, so a verified
/// artifact is only reproducible when the profile it was built with is declared
/// in the committed manifest. Pina therefore never injects profile overrides
/// into a verified build: an override absent from the manifest would make the
/// verified artifact differ from the one `pina build` deploys, and on-chain
/// verification of that deployment would fail.
fn verified_profile_divergence(
	declared: DeclaredReleaseProfile,
	size_profile: SizeProfile,
) -> Option<&'static str> {
	if !size_profile.applies_overrides() {
		return None;
	}
	if declared.overflow_checks == Some(true) {
		return Some(
			"the manifest sets `overflow-checks = true`, which the size profile disables for an \
			 ordinary build",
		);
	}
	if declared.codegen_units != Some(1) {
		return Some("the manifest does not set `codegen-units = 1`");
	}
	if size_profile.requests_lto() && declared.lto != Some(true) {
		return Some("the manifest does not enable fat LTO");
	}

	None
}

/// Warn when a verified build will not match the ordinary deploy artifact.
fn warn_verified_profile_divergence(project: &Project, size_profile: SizeProfile) {
	let declared = declared_release_profile(project);
	if let Some(reason) = verified_profile_divergence(declared, size_profile) {
		eprintln!(
			"warning: `pina build --verify` does not apply the size profile because a verified \
			 artifact must stay reproducible from its recorded Git revision, and {reason}. The \
			 verified artifact will differ from the artifact an ordinary `pina build` deploys. \
			 Declare the settings under `[profile.release]` in the workspace manifest (as `pina \
			 init` does) so both backends shrink, or build without `--verify` and accept the \
			 difference."
		);
	}
}

/// Arguments that delegate SBF compilation to the Agave `cargo-build-sbf`
/// driver.
///
/// The driver owns the SBF toolchain (platform-tools rustc and sbf-linker),
/// produces artifacts whose relocations are applied correctly by the real
/// runtimes, and honors `CARGO_TARGET_DIR` for its intermediate output.
///
/// Fat LTO is requested through the driver's `--lto` flag rather than
/// `CARGO_PROFILE_RELEASE_LTO`: the SBF toolchain ignores profile overrides,
/// and `--lto` only compiles when the program crate produces a cdylib without
/// a second rlib/lib output, so [`library_supports_lto`] gates it.
fn build_sbf_args(
	project: &Project,
	manifest_path: &Path,
	features: &str,
	options: &BuildOptions,
) -> Vec<OsString> {
	let mut args = vec![
		OsString::from("build-sbf"),
		OsString::from("--manifest-path"),
		manifest_path.as_os_str().to_owned(),
		OsString::from("--sbf-out-dir"),
		project.target_dir.join("sbf-build").into_os_string(),
		OsString::from("--features"),
		OsString::from(features),
	];

	if options.no_default_features {
		args.push(OsString::from("--no-default-features"));
	}

	if options.size_profile.requests_lto() && library_supports_lto(project) {
		args.push(OsString::from("--lto"));
	}

	args
}

/// Whether the program crate can be linked with fat LTO.
///
/// rustc rejects `-C lto` when one invocation emits a cdylib together with a
/// second lib/rlib output, so programs that also build as a Rust library for
/// host-side tests must ship `crate-type = ["cdylib"]` (moving shared logic
/// into a separate crate) before LTO applies.
fn library_supports_lto(project: &Project) -> bool {
	project
		.library_crate_types
		.iter()
		.all(|crate_type| crate_type == "cdylib")
}

/// Report when a program crate gives up the default fat-LTO build.
fn warn_lto_unavailable(project: &Project) {
	if !project
		.library_crate_types
		.iter()
		.any(|crate_type| crate_type != "cdylib")
	{
		return;
	}
	let crate_types = project.library_crate_types.join(", ");
	eprintln!(
		"warning: library crate-type [{crate_types}] precludes link-time optimization; deployed \
		 programs built from `[\"cdylib\"]` only are typically 20-30% smaller. Move shared logic \
		 into a separate crate, or pass --no-lto to silence this warning."
	);
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
	fn build_requires_checked_migration_history() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		fs::create_dir_all(temp.path().join("src"))
			.unwrap_or_else(|error| panic!("create source: {error}"));
		fs::write(
			temp.path().join("Cargo.toml"),
			"[package]\nname = \"migration-build\"\nversion = \"0.0.0\"\nedition = \
			 \"2024\"\n[lib]\npath = \"src/lib.rs\"\n",
		)
		.unwrap_or_else(|error| panic!("write manifest: {error}"));
		fs::write(
			temp.path().join("src/lib.rs"),
			"use pina::*;\ndeclare_id!(\"GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS\");\n#\
			 [discriminator]\nenum Kind { State = 1 }\n#[account(discriminator = Kind::State, \
			 migrations)]\nstruct State { value: u64 }\n",
		)
		.unwrap_or_else(|error| panic!("write source: {error}"));
		let project = Project::discover(temp.path())
			.unwrap_or_else(|error| panic!("discover fixture: {error}"));

		assert!(matches!(
			check_migrations_for_build(&project),
			Err(BuildError::Migration(
				crate::migrations::MigrationError::MissingSnapshot { .. }
			))
		));
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

	fn discover_fixture_with_crate_types(name: &str, crate_types: &str) -> (TempDir, Project) {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		fs::create_dir_all(temp.path().join("src"))
			.unwrap_or_else(|error| panic!("create source: {error}"));
		fs::write(
			temp.path().join("Cargo.toml"),
			format!(
				"[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \
				 \"2024\"\n[lib]\ncrate-type = [{crate_types}]\npath = \"src/lib.rs\"\n"
			),
		)
		.unwrap_or_else(|error| panic!("write manifest: {error}"));
		fs::write(temp.path().join("src/lib.rs"), "")
			.unwrap_or_else(|error| panic!("write source: {error}"));
		let project = Project::discover(temp.path())
			.unwrap_or_else(|error| panic!("discover fixture: {error}"));
		(temp, project)
	}

	fn build_args_for(project: &Project, size_profile: SizeProfile) -> Vec<String> {
		let manifest_path = project.program_dir.join("Cargo.toml");
		let options = BuildOptions {
			project_dir: project.root.clone(),
			features: Vec::new(),
			no_default_features: false,
			size_profile,
		};
		build_sbf_args(project, &manifest_path, "bpf-entrypoint", &options)
			.into_iter()
			.map(|argument| argument.to_string_lossy().into_owned())
			.collect()
	}

	#[test]
	fn build_args_request_lto_for_cdylib_only_programs() {
		let (_temp, project) =
			discover_fixture_with_crate_types("lto-cdylib-fixture", "\"cdylib\"");

		assert!(library_supports_lto(&project));
		let args = build_args_for(&project, SizeProfile::Production);
		assert!(args.contains(&"--lto".to_owned()));

		let args = build_args_for(&project, SizeProfile::None);
		assert!(!args.contains(&"--lto".to_owned()));
	}

	#[test]
	fn build_args_omit_lto_for_dual_crate_type_programs() {
		let (_temp, project) =
			discover_fixture_with_crate_types("lto-dual-fixture", "\"cdylib\", \"lib\"");

		assert!(!library_supports_lto(&project));
		let args = build_args_for(&project, SizeProfile::Production);
		assert!(!args.contains(&"--lto".to_owned()));
	}

	fn profile_env_for(size_profile: SizeProfile) -> Vec<(String, String)> {
		profile_env_with_manifest(size_profile, None)
	}

	fn profile_env_with_manifest(
		size_profile: SizeProfile,
		manifest_overflow_checks: Option<bool>,
	) -> Vec<(String, String)> {
		let options = BuildOptions {
			project_dir: PathBuf::new(),
			features: Vec::new(),
			no_default_features: false,
			size_profile,
		};
		let declared = DeclaredReleaseProfile {
			overflow_checks: manifest_overflow_checks,
			..DeclaredReleaseProfile::default()
		};
		let mut command = Command::new("cargo");
		apply_size_profile(&mut command, &options, declared);
		command
			.get_envs()
			.map(|(key, value)| {
				(
					key.to_string_lossy().into_owned(),
					value
						.map(|value| value.to_string_lossy().into_owned())
						.unwrap_or_default(),
				)
			})
			.collect()
	}

	#[test]
	fn size_profile_sets_lto_codegen_units_and_opt_level() {
		let env = profile_env_for(SizeProfile::Production);
		let lookup = |key: &str| {
			env.iter()
				.find(|(name, _)| name == key)
				.map(|(_, value)| value.clone())
		};

		assert_eq!(lookup("CARGO_PROFILE_RELEASE_LTO").as_deref(), Some("fat"));
		assert_eq!(
			lookup("CARGO_PROFILE_RELEASE_CODEGEN_UNITS").as_deref(),
			Some("1")
		);
		assert_eq!(
			lookup("CARGO_PROFILE_RELEASE_OPT_LEVEL").as_deref(),
			Some("3")
		);
		// Overflow checks stay off unless the caller opts in.
		assert_eq!(
			lookup("CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS").as_deref(),
			Some("false")
		);
	}

	#[test]
	fn overflow_checks_flag_restores_the_safety_check() {
		let env = profile_env_for(SizeProfile::ProductionWithOverflowChecks);
		let overflow = env
			.iter()
			.find(|(name, _)| name == "CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS")
			.map(|(_, value)| value.clone());

		assert_eq!(overflow.as_deref(), Some("true"));
	}

	#[test]
	fn size_profile_none_leaves_every_override_unset() {
		let env = profile_env_for(SizeProfile::None);

		assert!(env.is_empty());
	}

	#[test]
	fn production_without_lto_disables_lto_and_keeps_the_rest() {
		let env = profile_env_for(SizeProfile::ProductionWithoutLto);
		let lookup = |key: &str| {
			env.iter()
				.find(|(name, _)| name == key)
				.map(|(_, value)| value.clone())
		};

		// A manifest that declares `lto = "fat"` must still be overridden, so
		// the profile sets it explicitly rather than omitting the variable.
		assert_eq!(
			lookup("CARGO_PROFILE_RELEASE_LTO").as_deref(),
			Some("false")
		);
		assert_eq!(
			lookup("CARGO_PROFILE_RELEASE_CODEGEN_UNITS").as_deref(),
			Some("1")
		);
		assert_eq!(
			lookup("CARGO_PROFILE_RELEASE_OPT_LEVEL").as_deref(),
			Some("3")
		);
		assert_eq!(
			lookup("CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS").as_deref(),
			Some("false")
		);
	}

	#[test]
	fn manifest_overflow_checks_opt_in_outranks_the_size_profile() {
		let env = profile_env_with_manifest(SizeProfile::Production, Some(true));
		let lookup = |key: &str| {
			env.iter()
				.find(|(name, _)| name == key)
				.map(|(_, value)| value.clone())
		};

		// The manifest explicitly asks for overflow checks, so the profile keeps
		// them on: env overrides beat `[profile.release]`, and silently wrapping
		// arithmetic the manifest opted into would be a semantic change.
		assert_eq!(
			lookup("CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS").as_deref(),
			Some("true")
		);
		// The pure size knobs still apply.
		assert_eq!(lookup("CARGO_PROFILE_RELEASE_LTO").as_deref(), Some("fat"));
	}

	#[test]
	fn manifest_overflow_checks_opt_out_matches_the_size_profile() {
		let env = profile_env_with_manifest(SizeProfile::Production, Some(false));
		let lookup = |key: &str| {
			env.iter()
				.find(|(name, _)| name == key)
				.map(|(_, value)| value.clone())
		};

		assert_eq!(
			lookup("CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS").as_deref(),
			Some("false")
		);
	}

	#[test]
	fn silent_manifest_leaves_the_size_profile_in_charge() {
		let env = profile_env_with_manifest(SizeProfile::Production, None);
		let lookup = |key: &str| {
			env.iter()
				.find(|(name, _)| name == key)
				.map(|(_, value)| value.clone())
		};

		assert_eq!(
			lookup("CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS").as_deref(),
			Some("false")
		);
	}

	#[test]
	fn explicit_overflow_checks_flag_still_wins_over_a_silent_manifest() {
		let env = profile_env_with_manifest(SizeProfile::ProductionWithOverflowChecks, Some(false));
		let lookup = |key: &str| {
			env.iter()
				.find(|(name, _)| name == key)
				.map(|(_, value)| value.clone())
		};

		assert_eq!(
			lookup("CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS").as_deref(),
			Some("true")
		);
	}

	#[test]
	fn manifest_overflow_checks_reads_the_workspace_release_profile() {
		let (_temp, project) = discover_workspace_fixture(
			"overflow-manifest-fixture",
			"[profile.release]\noverflow-checks = true\n",
		);

		assert_eq!(
			declared_release_profile(&project).overflow_checks,
			Some(true)
		);
	}

	#[test]
	fn manifest_overflow_checks_ignores_a_program_local_release_profile() {
		let (_temp, project) = discover_workspace_fixture("overflow-program-fixture", "");
		// Cargo applies `[profile]` tables from the workspace root only, so a
		// program-local table must not be mistaken for a manifest opt-in.
		fs::write(
			project.program_dir.join("Cargo.toml"),
			"[package]\nname = \"overflow-program-fixture\"\nversion = \"0.0.0\"\nedition = \
			 \"2024\"\n[lib]\ncrate-type = [\"cdylib\"]\npath = \
			 \"src/lib.rs\"\n\n[profile.release]\noverflow-checks = true\n",
		)
		.unwrap_or_else(|error| panic!("write program manifest: {error}"));

		assert_eq!(declared_release_profile(&project).overflow_checks, None);
	}

	#[test]
	fn manifest_overflow_checks_is_none_without_a_release_profile() {
		let (_temp, project) = discover_workspace_fixture("silent-manifest-fixture", "");

		assert_eq!(declared_release_profile(&project).overflow_checks, None);
	}

	#[test]
	fn declared_profile_reads_every_size_setting() {
		let (_temp, project) = discover_workspace_fixture(
			"declared-profile-fixture",
			"[profile.release]\nlto = \"fat\"\ncodegen-units = 1\noverflow-checks = false\n",
		);

		assert_eq!(
			declared_release_profile(&project),
			DeclaredReleaseProfile {
				lto: Some(true),
				codegen_units: Some(1),
				overflow_checks: Some(false),
			}
		);
	}

	#[test]
	fn verified_build_diverges_when_the_manifest_lacks_the_size_profile() {
		// The manifest `pina init` writes declares the whole profile, so a
		// verified build reproduces the ordinary artifact.
		assert_eq!(
			verified_profile_divergence(
				DeclaredReleaseProfile {
					lto: Some(true),
					codegen_units: Some(1),
					overflow_checks: Some(false),
				},
				SizeProfile::Production,
			),
			None
		);

		// A silent manifest cannot reproduce the production profile.
		assert!(
			verified_profile_divergence(DeclaredReleaseProfile::default(), SizeProfile::Production)
				.is_some()
		);

		// `--no-size-profile` never diverges: it asks for the manifest as-is.
		assert_eq!(
			verified_profile_divergence(DeclaredReleaseProfile::default(), SizeProfile::None),
			None
		);

		// A manifest that keeps overflow checks on diverges, because the
		// ordinary build honours the manifest while a size profile would not.
		assert!(
			verified_profile_divergence(
				DeclaredReleaseProfile {
					lto: Some(true),
					codegen_units: Some(1),
					overflow_checks: Some(true),
				},
				SizeProfile::Production,
			)
			.is_some()
		);
	}

	#[test]
	fn lto_string_off_is_not_treated_as_enabled() {
		let (_temp, project) = discover_workspace_fixture(
			"lto-off-fixture",
			"[profile.release]\nlto = \"off\"\ncodegen-units = 1\noverflow-checks = false\n",
		);

		let declared = declared_release_profile(&project);
		assert_eq!(declared.lto, Some(false));
		assert!(
			verified_profile_divergence(declared, SizeProfile::Production).is_some(),
			"fat LTO requested but the manifest disables it"
		);
	}

	#[test]
	fn lto_named_modes_count_as_enabled() {
		// A named LTO mode other than the disabling spellings means LTO is on,
		// so the manifest already reproduces that part of the profile. A
		// malformed value never reaches here: `cargo metadata` rejects it while
		// the project is discovered.
		let (_named, named) = discover_workspace_fixture(
			"lto-named-fixture",
			"[profile.release]\nlto = \"thin\"\ncodegen-units = 1\noverflow-checks = false\n",
		);
		assert_eq!(declared_release_profile(&named).lto, Some(true));
		assert_eq!(
			verified_profile_divergence(declared_release_profile(&named), SizeProfile::Production),
			None,
			"a named LTO mode and matching knobs reproduce the production profile"
		);
	}

	#[test]
	fn warning_paths_run_for_both_opt_in_and_divergence() {
		// The printers are reached only when a warning fires, so exercise both
		// so the guidance text is covered and cannot rot.
		warn_manifest_overflow_checks();

		let (_temp, project) = discover_workspace_fixture("warn-fixture", "");
		warn_verified_profile_divergence(&project, SizeProfile::Production);

		// A conforming manifest stays quiet rather than warning.
		let (_conforming, conforming) = discover_workspace_fixture(
			"quiet-fixture",
			"[profile.release]\nlto = \"fat\"\ncodegen-units = 1\noverflow-checks = false\n",
		);
		assert_eq!(
			verified_profile_divergence(
				declared_release_profile(&conforming),
				SizeProfile::Production
			),
			None
		);
		warn_verified_profile_divergence(&conforming, SizeProfile::Production);
		warn_verified_profile_divergence(&conforming, SizeProfile::None);
	}

	/// Build a workspace whose root manifest carries `extra_manifest` and whose
	/// single member is a cdylib program.
	fn discover_workspace_fixture(name: &str, extra_manifest: &str) -> (TempDir, Project) {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let program_dir = temp.path().join("program");
		fs::create_dir_all(program_dir.join("src"))
			.unwrap_or_else(|error| panic!("create source: {error}"));
		fs::write(
			temp.path().join("Cargo.toml"),
			format!("[workspace]\nmembers = [\"program\"]\nresolver = \"2\"\n\n{extra_manifest}"),
		)
		.unwrap_or_else(|error| panic!("write workspace manifest: {error}"));
		fs::write(
			program_dir.join("Cargo.toml"),
			format!(
				"[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \
				 \"2024\"\n[lib]\ncrate-type = [\"cdylib\"]\npath = \"src/lib.rs\"\n"
			),
		)
		.unwrap_or_else(|error| panic!("write program manifest: {error}"));
		fs::write(program_dir.join("src/lib.rs"), "")
			.unwrap_or_else(|error| panic!("write source: {error}"));
		let project = Project::discover(&program_dir)
			.unwrap_or_else(|error| panic!("discover fixture: {error}"));
		(temp, project)
	}
}
