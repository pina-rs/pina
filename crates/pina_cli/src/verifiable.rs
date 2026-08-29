//! Deterministic SBF build boundary backed by `solana-verify`.

use std::ffi::OsStr;
use std::ffi::OsString;
use std::io::Read;
use std::net::IpAddr;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use atomic_write_file::AtomicWriteFile;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;
use tempfile::TempDir;

use crate::project::Project;

pub(super) const SUPPORTED_SOLANA_VERIFY_VERSION: &str = "0.5.1";

const MAX_FEATURE_COUNT: usize = 128;
const MAX_FEATURE_LENGTH: usize = 128;
const MAX_FEATURE_BYTES: usize = 4 * 1024;
const MAX_DIAGNOSTIC_COUNT: usize = 8;
const MAX_DIAGNOSTIC_LENGTH: usize = 128;
const MAX_DIAGNOSTIC_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyBuildOptions {
	/// `solana-verify` executable to invoke. Pina never installs this tool.
	pub executable: OsString,
}

impl Default for VerifyBuildOptions {
	fn default() -> Self {
		Self {
			executable: OsString::from("solana-verify"),
		}
	}
}

#[derive(Debug, thiserror::Error)]
pub enum VerifyBuildError {
	#[error("Failed to run `{command}`: {source}")]
	Run {
		command: &'static str,
		source: std::io::Error,
	},

	#[error(
		"Unsupported solana-verify version; Pina requires exactly \
		 {SUPPORTED_SOLANA_VERIFY_VERSION}"
	)]
	UnsupportedVersion,

	#[error("`solana-verify build` failed ({status})")]
	BuildFailed { status: String },

	#[error("A verifiable build requires Cargo.lock at the source root: {path}")]
	MissingRootLockfile { path: PathBuf },

	#[error("The Cargo workspace is outside the mounted source root: {workspace}")]
	WorkspaceOutsideSource { workspace: PathBuf },

	#[error("Could not resolve the Cargo workspace for the verifiable build: {message}")]
	WorkspaceDiscovery { message: String },

	#[error("Could not create a private verifiable-build snapshot: {source}")]
	CreateSnapshot { source: std::io::Error },

	#[error("Verifiable builds require a Git worktree with tracked build inputs")]
	SourceControlRequired,

	#[error("Verifiable builds require a completely clean Git worktree and index")]
	DirtySource,

	#[error("Could not resolve the exact Git revision for the verifiable build")]
	SourceRevisionUnavailable,

	#[error("Invalid Cargo feature selection for a verifiable build")]
	InvalidFeatureSelection,

	#[error("Could not stage tracked source files with Git ({status})")]
	StageTrackedSource { status: String },

	#[error("This host is not supported by solana-verify 0.5.1 ({host})")]
	UnsupportedHost { host: &'static str },

	#[error("Verifiable builds reject symbolic links and reparse-point aliases: {path}")]
	SourceAlias { path: PathBuf },

	#[error("Unsupported non-file entry in the verifiable-build source snapshot: {path}")]
	UnsupportedEntry { path: PathBuf },

	#[error("Failed to copy verifiable-build input {path}: {source}")]
	CopySource {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("solana-verify completed without creating the expected SBF artifact: {path}")]
	MissingArtifact { path: PathBuf },

	#[error("Failed to hash {path}: {source}")]
	Hash {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Failed to serialize the Pina build record: {source}")]
	SerializeRecord { source: serde_json::Error },

	#[error("Failed to publish Pina build record output {path}: {source}")]
	PublishRecord {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Pina build record outputs must not be filesystem aliases: {path}")]
	RecordAlias { path: PathBuf },

	#[error("Failed to read Pina build record {path}: {source}")]
	ReadRecord {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Failed to parse Pina build record {path}: {source}")]
	ParseRecord {
		path: PathBuf,
		source: serde_json::Error,
	},

	#[error("Pina build record hash does not match its artifact: {path}")]
	RecordHashMismatch { path: PathBuf },

	#[error("Pina build record is too large: {path}")]
	RecordTooLarge { path: PathBuf },

	#[error("Unsupported Pina build record schema version: {found}")]
	UnsupportedRecordSchema { found: u32 },

	#[error("Unsupported solana-verify build record version: {found}")]
	UnsupportedRecordToolVersion { found: String },

	#[error("Invalid Pina build record: {reason}")]
	InvalidRecord { reason: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct VerificationManifest {
	pub schema_version: u32,
	pub package_name: String,
	pub library_name: String,
	pub executable_hash: String,
	pub solana_verify_version: String,
	pub build: VerificationBuildInputs,
	pub source: VerificationSource,
	pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct VerificationBuildInputs {
	pub mount_path: String,
	pub workspace_path: String,
	pub program_path: String,
	pub library_name: String,
	pub features: Vec<String>,
	pub default_features: bool,
	pub cargo_lock_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct VerificationSource {
	pub repository: Option<String>,
	pub revision: Option<String>,
	pub dirty: Option<bool>,
}

pub(super) struct VerifiedBuild {
	_snapshot: TempDir,
	pub artifact: PathBuf,
	pub program_dir: PathBuf,
	pub manifest: VerificationManifest,
}

/// A Pina-local deterministic build record whose adjacent artifact hash has
/// been recomputed successfully.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedBuildRecord {
	manifest: VerificationManifest,
	artifact: PathBuf,
}

impl VerifiedBuildRecord {
	#[must_use]
	pub fn artifact(&self) -> &Path {
		&self.artifact
	}

	#[must_use]
	pub fn library_name(&self) -> &str {
		&self.manifest.library_name
	}

	#[must_use]
	pub fn executable_hash(&self) -> &str {
		&self.manifest.executable_hash
	}

	#[must_use]
	pub fn repository(&self) -> Option<&str> {
		self.manifest.source.repository.as_deref()
	}

	#[must_use]
	pub fn revision(&self) -> Option<&str> {
		self.manifest.source.revision.as_deref()
	}

	#[must_use]
	pub fn mount_path(&self) -> &str {
		&self.manifest.build.mount_path
	}

	#[must_use]
	pub fn workspace_path(&self) -> &str {
		&self.manifest.build.workspace_path
	}

	#[must_use]
	pub fn program_path(&self) -> &str {
		&self.manifest.build.program_path
	}

	#[must_use]
	pub fn features(&self) -> &[String] {
		&self.manifest.build.features
	}

	#[must_use]
	pub fn default_features(&self) -> bool {
		self.manifest.build.default_features
	}
}

struct CommandOutput {
	success: bool,
	status: String,
	stdout: Vec<u8>,
}

trait CommandRunner {
	fn capture(
		&self,
		program: &OsStr,
		args: &[OsString],
		cwd: &Path,
	) -> Result<CommandOutput, std::io::Error>;

	fn inherit(
		&self,
		program: &OsStr,
		args: &[OsString],
		cwd: &Path,
	) -> Result<CommandOutput, std::io::Error>;
}

struct ProcessRunner;

impl CommandRunner for ProcessRunner {
	fn capture(
		&self,
		program: &OsStr,
		args: &[OsString],
		cwd: &Path,
	) -> Result<CommandOutput, std::io::Error> {
		let output = Command::new(program).args(args).current_dir(cwd).output()?;

		Ok(CommandOutput {
			success: output.status.success(),
			status: output.status.to_string(),
			stdout: output.stdout,
		})
	}

	fn inherit(
		&self,
		program: &OsStr,
		args: &[OsString],
		cwd: &Path,
	) -> Result<CommandOutput, std::io::Error> {
		let status = Command::new(program)
			.args(args)
			.current_dir(cwd)
			.stdin(Stdio::null())
			.stdout(Stdio::inherit())
			.stderr(Stdio::inherit())
			.status()?;

		Ok(CommandOutput {
			success: status.success(),
			status: status.to_string(),
			stdout: Vec::new(),
		})
	}
}

pub(super) fn build(
	project: &Project,
	features: &[String],
	no_default_features: bool,
	options: &VerifyBuildOptions,
) -> Result<VerifiedBuild, VerifyBuildError> {
	build_with_runner(
		project,
		features,
		no_default_features,
		options,
		&ProcessRunner,
	)
}

pub(super) fn publish_record(
	verified: &VerifiedBuild,
	target_dir: &Path,
) -> Result<PathBuf, VerifyBuildError> {
	let output_dir = target_dir.join("pina/verifiable");
	let stem = format!(
		"{}-{}",
		verified.manifest.library_name, verified.manifest.executable_hash
	);
	let artifact_path = output_dir.join(format!("{stem}.so"));
	let manifest_path = output_dir.join(format!("{stem}.json"));
	std::fs::create_dir_all(&output_dir).map_err(|source| {
		VerifyBuildError::PublishRecord {
			path: output_dir.clone(),
			source,
		}
	})?;
	ensure_record_path(&output_dir, true)?;
	ensure_record_path(&artifact_path, false)?;
	ensure_record_path(&manifest_path, false)?;
	copy_atomic(&verified.artifact, &artifact_path)?;
	let manifest = serde_json::to_vec_pretty(&verified.manifest)
		.map_err(|source| VerifyBuildError::SerializeRecord { source })?;
	write_atomic(&manifest_path, &manifest)?;

	Ok(manifest_path)
}

pub fn read_record(path: &Path) -> Result<VerifiedBuildRecord, VerifyBuildError> {
	ensure_record_path(path, false)?;
	let file = std::fs::File::open(path).map_err(|source| {
		VerifyBuildError::ReadRecord {
			path: path.to_path_buf(),
			source,
		}
	})?;
	let bytes = read_bounded_record(file, path)?;
	let manifest: VerificationManifest = serde_json::from_slice(&bytes).map_err(|source| {
		VerifyBuildError::ParseRecord {
			path: path.to_path_buf(),
			source,
		}
	})?;
	validate_manifest(path, &manifest)?;
	let artifact = path.with_extension("so");
	ensure_record_path(&artifact, false)?;
	if executable_hash(&artifact)? != manifest.executable_hash {
		return Err(VerifyBuildError::RecordHashMismatch { path: artifact });
	}

	Ok(VerifiedBuildRecord { manifest, artifact })
}

fn read_bounded_record(reader: impl Read, path: &Path) -> Result<Vec<u8>, VerifyBuildError> {
	const MAX_RECORD_SIZE: u64 = 1024 * 1024;

	let mut bytes = Vec::new();
	reader
		.take(MAX_RECORD_SIZE + 1)
		.read_to_end(&mut bytes)
		.map_err(|source| {
			VerifyBuildError::ReadRecord {
				path: path.to_path_buf(),
				source,
			}
		})?;
	if bytes.len() as u64 > MAX_RECORD_SIZE {
		return Err(VerifyBuildError::RecordTooLarge {
			path: path.to_path_buf(),
		});
	}

	Ok(bytes)
}

fn validate_manifest(path: &Path, manifest: &VerificationManifest) -> Result<(), VerifyBuildError> {
	if manifest.schema_version != 1 {
		return Err(VerifyBuildError::UnsupportedRecordSchema {
			found: manifest.schema_version,
		});
	}
	if manifest.solana_verify_version != SUPPORTED_SOLANA_VERIFY_VERSION {
		return Err(VerifyBuildError::UnsupportedRecordToolVersion {
			found: manifest.solana_verify_version.clone(),
		});
	}
	if manifest.executable_hash.len() != 64
		|| !manifest
			.executable_hash
			.bytes()
			.all(|byte| byte.is_ascii_hexdigit())
	{
		return Err(VerifyBuildError::InvalidRecord {
			reason: "executableHash must be 64 hexadecimal characters",
		});
	}
	if manifest.library_name != manifest.build.library_name
		|| manifest.library_name.is_empty()
		|| !manifest
			.library_name
			.bytes()
			.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
	{
		return Err(VerifyBuildError::InvalidRecord {
			reason: "library names are inconsistent or invalid",
		});
	}
	let expected = format!(
		"{}-{}.json",
		manifest.library_name, manifest.executable_hash
	);
	if path.file_name().and_then(OsStr::to_str) != Some(expected.as_str()) {
		return Err(VerifyBuildError::InvalidRecord {
			reason: "record filename does not match its library and hash",
		});
	}
	for value in [
		&manifest.build.mount_path,
		&manifest.build.workspace_path,
		&manifest.build.program_path,
	] {
		if !is_safe_relative_path(value) {
			return Err(VerifyBuildError::InvalidRecord {
				reason: "build paths must be relative and must not traverse parents",
			});
		}
	}
	if !valid_features(&manifest.build.features) {
		return Err(VerifyBuildError::InvalidRecord {
			reason: "build features must be bounded, sorted, unique Cargo feature selectors",
		});
	}
	if !valid_diagnostics(&manifest.diagnostics) {
		return Err(VerifyBuildError::InvalidRecord {
			reason: "diagnostics must be bounded, unique diagnostic identifiers",
		});
	}
	if manifest.source.dirty != Some(false) {
		return Err(VerifyBuildError::InvalidRecord {
			reason: "source dirty state must be false",
		});
	}
	if !manifest
		.source
		.revision
		.as_deref()
		.is_some_and(is_git_revision)
	{
		return Err(VerifyBuildError::InvalidRecord {
			reason: "source revision must be a 40-character hexadecimal Git revision",
		});
	}
	if manifest
		.source
		.repository
		.as_deref()
		.is_some_and(|repository| !is_public_https_url(repository))
	{
		return Err(VerifyBuildError::InvalidRecord {
			reason: "source repository must be a public HTTPS URL",
		});
	}
	if !is_sha256(&manifest.build.cargo_lock_sha256) {
		return Err(VerifyBuildError::InvalidRecord {
			reason: "cargoLockSha256 must be 64 hexadecimal characters",
		});
	}

	Ok(())
}

fn valid_features(features: &[String]) -> bool {
	features.len() <= MAX_FEATURE_COUNT
		&& features
			.iter()
			.map(String::len)
			.try_fold(0_usize, usize::checked_add)
			.is_some_and(|total| total <= MAX_FEATURE_BYTES)
		&& features.iter().all(|feature| {
			feature.len() <= MAX_FEATURE_LENGTH && is_cargo_feature_selector(feature)
		}) && features.windows(2).all(|pair| pair[0] < pair[1])
		&& features
			.binary_search_by(|feature| feature.as_str().cmp("bpf-entrypoint"))
			.is_ok()
}

fn is_cargo_feature_selector(feature: &str) -> bool {
	!feature.is_empty()
		&& !feature.contains(',')
		&& !feature.chars().any(char::is_whitespace)
		&& !feature.chars().any(char::is_control)
}

fn valid_diagnostics(diagnostics: &[String]) -> bool {
	diagnostics.len() <= MAX_DIAGNOSTIC_COUNT
		&& diagnostics
			.iter()
			.map(String::len)
			.try_fold(0_usize, usize::checked_add)
			.is_some_and(|total| total <= MAX_DIAGNOSTIC_BYTES)
		&& diagnostics.iter().all(|diagnostic| {
			!diagnostic.is_empty()
				&& diagnostic.len() <= MAX_DIAGNOSTIC_LENGTH
				&& diagnostic
					.bytes()
					.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
		}) && diagnostics
		.iter()
		.enumerate()
		.all(|(index, diagnostic)| !diagnostics[..index].contains(diagnostic))
}

fn is_safe_relative_path(value: &str) -> bool {
	!value.is_empty()
		&& !value.contains(['\\', ':'])
		&& Path::new(value).components().all(|component| {
			matches!(
				component,
				std::path::Component::CurDir | std::path::Component::Normal(_)
			)
		})
}

fn is_sha256(value: &str) -> bool {
	value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_git_revision(value: &str) -> bool {
	value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn ensure_record_path(path: &Path, directory: bool) -> Result<(), VerifyBuildError> {
	match std::fs::symlink_metadata(path) {
		Ok(metadata)
			if metadata_is_alias(&metadata)
				|| (directory && !metadata.is_dir())
				|| (!directory && !metadata.is_file()) =>
		{
			Err(VerifyBuildError::RecordAlias {
				path: path.to_path_buf(),
			})
		}
		Ok(_) => Ok(()),
		Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
		Err(source) => {
			Err(VerifyBuildError::PublishRecord {
				path: path.to_path_buf(),
				source,
			})
		}
	}
}

fn metadata_is_alias(metadata: &std::fs::Metadata) -> bool {
	if metadata.file_type().is_symlink() {
		return true;
	}

	#[cfg(windows)]
	{
		use std::os::windows::fs::MetadataExt;

		const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
		return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
	}

	#[cfg(not(windows))]
	false
}

fn copy_atomic(source: &Path, destination: &Path) -> Result<(), VerifyBuildError> {
	let bytes = std::fs::read(source).map_err(|source_error| {
		VerifyBuildError::PublishRecord {
			path: source.to_path_buf(),
			source: source_error,
		}
	})?;
	write_atomic(destination, &bytes)
}

fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), VerifyBuildError> {
	let file = AtomicWriteFile::open(path).map_err(|source| {
		VerifyBuildError::PublishRecord {
			path: path.to_path_buf(),
			source,
		}
	})?;
	complete_atomic_write(file, path, contents)
}

trait RecordFile: std::io::Write {
	fn commit(self) -> Result<(), std::io::Error>;
}

impl RecordFile for AtomicWriteFile {
	fn commit(self) -> Result<(), std::io::Error> {
		AtomicWriteFile::commit(self)
	}
}

fn complete_atomic_write(
	mut file: impl RecordFile,
	path: &Path,
	contents: &[u8],
) -> Result<(), VerifyBuildError> {
	file.write_all(contents).map_err(|source| {
		VerifyBuildError::PublishRecord {
			path: path.to_path_buf(),
			source,
		}
	})?;
	RecordFile::commit(file).map_err(|source| {
		VerifyBuildError::PublishRecord {
			path: path.to_path_buf(),
			source,
		}
	})
}

fn build_with_runner(
	project: &Project,
	features: &[String],
	no_default_features: bool,
	options: &VerifyBuildOptions,
	runner: &impl CommandRunner,
) -> Result<VerifiedBuild, VerifyBuildError> {
	ensure_supported_host()?;
	build_on_supported_host(project, features, no_default_features, options, runner)
}

fn build_on_supported_host(
	project: &Project,
	features: &[String],
	no_default_features: bool,
	options: &VerifyBuildOptions,
	runner: &impl CommandRunner,
) -> Result<VerifiedBuild, VerifyBuildError> {
	if !valid_features(features) {
		return Err(VerifyBuildError::InvalidFeatureSelection);
	}
	check_version(options.executable.as_os_str(), &project.root, runner)?;
	let source = inspect_source(project, runner);
	let source_root = source
		.root
		.as_deref()
		.ok_or(VerifyBuildError::SourceControlRequired)?;
	if source.dirty != Some(false) {
		return Err(VerifyBuildError::DirtySource);
	}
	if source.revision.is_none() {
		return Err(VerifyBuildError::SourceRevisionUnavailable);
	}

	let workspace_root = project.workspace_root().map_err(|source| {
		VerifyBuildError::WorkspaceDiscovery {
			message: source.to_string(),
		}
	})?;
	let (workspace_relative, program_relative) =
		resolve_build_paths(project, source_root, &workspace_root)?;
	let snapshot = tempfile::Builder::new()
		.prefix("pina-verifiable-")
		.tempdir()
		.map_err(|source| VerifyBuildError::CreateSnapshot { source })?;
	let snapshot_root = snapshot.path().join("source");
	std::fs::create_dir(&snapshot_root)
		.map_err(|source| VerifyBuildError::CreateSnapshot { source })?;
	stage_tracked_source(source_root, &snapshot_root, runner)?;
	audit_snapshot(&snapshot_root)?;

	let snapshot_workspace = snapshot_root.join(workspace_relative);
	let args = build_arguments(
		&snapshot_root,
		&snapshot_workspace,
		&project.library_name,
		features,
		no_default_features,
	);
	let output = runner
		.inherit(options.executable.as_os_str(), &args, source_root)
		.map_err(|source| {
			VerifyBuildError::Run {
				command: "solana-verify build",
				source,
			}
		})?;

	if !output.success {
		return Err(VerifyBuildError::BuildFailed {
			status: output.status,
		});
	}

	let artifact = snapshot_workspace
		.join("target/deploy")
		.join(format!("{}.so", project.library_name));
	ensure_regular_file(&artifact)?;
	let executable_hash = executable_hash(&artifact)?;
	let cargo_lock_sha256 = sha256_file(&snapshot_workspace.join("Cargo.lock"))?;
	let diagnostics = source.diagnostics;

	Ok(VerifiedBuild {
		artifact,
		program_dir: snapshot_root.join(program_relative),
		manifest: VerificationManifest {
			schema_version: 1,
			package_name: project.package_name.clone(),
			library_name: project.library_name.clone(),
			executable_hash,
			solana_verify_version: SUPPORTED_SOLANA_VERIFY_VERSION.to_owned(),
			build: VerificationBuildInputs {
				mount_path: ".".to_owned(),
				workspace_path: portable_path(workspace_relative),
				program_path: portable_path(program_relative),
				library_name: project.library_name.clone(),
				features: features.to_vec(),
				default_features: !no_default_features,
				cargo_lock_sha256,
			},
			source: VerificationSource {
				repository: source.repository,
				revision: source.revision,
				dirty: source.dirty,
			},
			diagnostics: diagnostics.into_iter().map(str::to_owned).collect(),
		},
		_snapshot: snapshot,
	})
}

fn resolve_build_paths<'a>(
	project: &'a Project,
	source_root: &'a Path,
	workspace_root: &'a Path,
) -> Result<(&'a Path, &'a Path), VerifyBuildError> {
	let lockfile = workspace_root.join("Cargo.lock");
	if !lockfile.is_file() {
		return Err(VerifyBuildError::MissingRootLockfile { path: lockfile });
	}
	let workspace_relative = workspace_root.strip_prefix(source_root).map_err(|_| {
		VerifyBuildError::WorkspaceOutsideSource {
			workspace: workspace_root.to_path_buf(),
		}
	})?;
	let program_relative = project.program_dir.strip_prefix(source_root).map_err(|_| {
		VerifyBuildError::WorkspaceOutsideSource {
			workspace: project.program_dir.clone(),
		}
	})?;

	Ok((workspace_relative, program_relative))
}

fn check_version(
	executable: &OsStr,
	cwd: &Path,
	runner: &impl CommandRunner,
) -> Result<(), VerifyBuildError> {
	let output = runner
		.capture(executable, &[OsString::from("--version")], cwd)
		.map_err(|source| {
			VerifyBuildError::Run {
				command: "solana-verify --version",
				source,
			}
		})?;
	let expected = format!("solana-verify {SUPPORTED_SOLANA_VERIFY_VERSION}");

	if !output.success || output.stdout.trim_ascii_end() != expected.as_bytes() {
		return Err(VerifyBuildError::UnsupportedVersion);
	}

	Ok(())
}

fn build_arguments(
	mount: &Path,
	workspace: &Path,
	library_name: &str,
	features: &[String],
	no_default_features: bool,
) -> Vec<OsString> {
	let mut args = vec![
		OsString::from("build"),
		mount.as_os_str().to_owned(),
		OsString::from("--workspace-path"),
		workspace.as_os_str().to_owned(),
		OsString::from("--library-name"),
		OsString::from(library_name),
		OsString::from("--"),
	];
	if !features.is_empty() {
		args.push(OsString::from("--features"));
		args.push(OsString::from(features.join(",")));
	}

	if no_default_features {
		args.push(OsString::from("--no-default-features"));
	}

	args
}

fn ensure_supported_host() -> Result<(), VerifyBuildError> {
	ensure_supported_host_values(
		std::env::consts::OS,
		std::env::consts::ARCH,
		option_env!("CARGO_CFG_TARGET_ENV").unwrap_or(""),
	)
}

fn ensure_supported_host_values(
	os: &'static str,
	arch: &'static str,
	_environment: &'static str,
) -> Result<(), VerifyBuildError> {
	if os == "linux" || (os == "macos" && matches!(arch, "x86_64" | "aarch64")) {
		return Ok(());
	}

	Err(VerifyBuildError::UnsupportedHost { host: os })
}

struct SourceInspection {
	root: Option<PathBuf>,
	repository: Option<String>,
	revision: Option<String>,
	dirty: Option<bool>,
	diagnostics: Vec<&'static str>,
}

fn inspect_source(project: &Project, runner: &impl CommandRunner) -> SourceInspection {
	let git = OsStr::new("git");
	let root_output = runner.capture(
		git,
		&[
			OsString::from("-C"),
			project.program_dir.as_os_str().to_owned(),
			OsString::from("rev-parse"),
			OsString::from("--show-toplevel"),
		],
		&project.root,
	);
	let Ok(root_output) = root_output else {
		return unavailable_source();
	};
	let Some(root) = successful_path(&root_output) else {
		return unavailable_source();
	};
	let Ok(root) = std::fs::canonicalize(root) else {
		return unavailable_source();
	};
	let revision =
		git_text(runner, &root, &["rev-parse", "HEAD"]).filter(|value| is_git_revision(value));
	let repository = git_text(runner, &root, &["config", "--get", "remote.origin.url"])
		.filter(|value| is_public_https_url(value));
	let dirty = git_capture(
		runner,
		&root,
		&["status", "--porcelain", "--untracked-files=normal"],
	)
	.map(|output| !output.is_empty());
	let mut diagnostics = Vec::new();

	if repository.is_none() {
		diagnostics.push("source-repository-is-not-public-https");
	}
	if revision.is_none() {
		diagnostics.push("source-revision-is-unavailable");
	}
	match dirty {
		Some(true) => diagnostics.push("source-tree-is-dirty"),
		None => diagnostics.push("source-state-is-unavailable"),
		Some(false) => {}
	}
	diagnostics.push("remote-revision-reachability-not-checked");

	SourceInspection {
		root: Some(root),
		repository,
		revision,
		dirty,
		diagnostics,
	}
}

fn unavailable_source() -> SourceInspection {
	SourceInspection {
		root: None,
		repository: None,
		revision: None,
		dirty: None,
		diagnostics: vec!["source-control-is-unavailable"],
	}
}

fn stage_tracked_source(
	root: &Path,
	destination: &Path,
	runner: &impl CommandRunner,
) -> Result<(), VerifyBuildError> {
	let mut prefix = destination.as_os_str().to_owned();
	prefix.push(std::path::MAIN_SEPARATOR_STR);
	let args = [
		OsString::from("checkout-index"),
		OsString::from("--all"),
		OsString::from("--force"),
		{
			let mut argument = OsString::from("--prefix=");
			argument.push(prefix);
			argument
		},
	];
	let output = runner
		.capture(OsStr::new("git"), &args, root)
		.map_err(|source| {
			VerifyBuildError::Run {
				command: "git checkout-index",
				source,
			}
		})?;

	if !output.success {
		return Err(VerifyBuildError::StageTrackedSource {
			status: output.status,
		});
	}

	Ok(())
}

fn git_text(runner: &impl CommandRunner, root: &Path, args: &[&str]) -> Option<String> {
	let output = git_capture(runner, root, args)?;
	String::from_utf8(output)
		.ok()
		.map(|value| value.trim().to_owned())
}

fn git_capture(runner: &impl CommandRunner, root: &Path, args: &[&str]) -> Option<Vec<u8>> {
	let args = args.iter().map(OsString::from).collect::<Vec<_>>();
	let output = runner.capture(OsStr::new("git"), &args, root).ok()?;
	output.success.then_some(output.stdout)
}

fn successful_path(output: &CommandOutput) -> Option<PathBuf> {
	if !output.success {
		return None;
	}

	let value = String::from_utf8(output.stdout.clone()).ok()?;
	let value = value.trim();
	(!value.is_empty()).then(|| PathBuf::from(value))
}

fn is_public_https_url(value: &str) -> bool {
	if value.contains(['?', '#']) || value.chars().any(char::is_control) {
		return false;
	}
	let Some(authority_and_path) = value.strip_prefix("https://") else {
		return false;
	};
	let authority = authority_and_path.split('/').next().unwrap_or_default();
	if authority.is_empty() || authority.contains('@') {
		return false;
	}
	let host = if let Some(bracketed) = authority.strip_prefix('[') {
		let Some((host, suffix)) = bracketed.split_once(']') else {
			return false;
		};
		if !valid_port_suffix(suffix) {
			return false;
		}
		host
	} else if let Some((host, port)) = authority.split_once(':') {
		if port.is_empty() || port.parse::<u16>().is_err() {
			return false;
		}
		host
	} else {
		authority
	};
	let host = host.to_ascii_lowercase();
	if host == "localhost" || host.ends_with(".localhost") || host.strip_suffix(".local").is_some()
	{
		return false;
	}
	match host.parse::<IpAddr>() {
		Ok(IpAddr::V4(ip)) => is_public_ipv4(ip),
		Ok(IpAddr::V6(ip)) => {
			if let Some(mapped) = ip.to_ipv4_mapped() {
				return is_public_ipv4(mapped);
			}
			let first = ip.segments()[0];
			!(ip.is_loopback()
				|| ip.is_unspecified()
				|| ip.is_multicast()
				|| (first & 0xfe00) == 0xfc00
				|| (first & 0xffc0) == 0xfe80
				|| (ip.segments()[0] == 0x2001 && ip.segments()[1] == 0x0db8))
		}
		Err(_) => is_public_dns_name(&host),
	}
}

fn is_public_ipv4(ip: std::net::Ipv4Addr) -> bool {
	!(ip.is_private()
		|| ip.is_loopback()
		|| ip.is_link_local()
		|| ip.is_unspecified()
		|| ip.is_multicast()
		|| ip.is_broadcast()
		|| ip.is_documentation())
}

fn valid_port_suffix(suffix: &str) -> bool {
	suffix.is_empty()
		|| suffix
			.strip_prefix(':')
			.is_some_and(|port| !port.is_empty() && port.parse::<u16>().is_ok())
}

fn is_public_dns_name(host: &str) -> bool {
	if host.len() > 253
		|| !host.contains('.')
		|| host
			.bytes()
			.all(|byte| byte.is_ascii_digit() || byte == b'.')
		|| host
			.bytes()
			.any(|byte| !byte.is_ascii_alphanumeric() && byte != b'-' && byte != b'.')
	{
		return false;
	}
	if host.split('.').any(|label| {
		label.is_empty() || label.len() > 63 || label.starts_with('-') || label.ends_with('-')
	}) {
		return false;
	}
	let tld = host.rsplit('.').next().unwrap_or_default();
	!matches!(
		tld,
		"example" | "internal" | "invalid" | "local" | "localhost" | "test"
	)
}

fn audit_snapshot(root: &Path) -> Result<(), VerifyBuildError> {
	for entry in walkdir::WalkDir::new(root).follow_links(false) {
		let entry = entry.map_err(|source_error| {
			VerifyBuildError::CopySource {
				path: source_error
					.path()
					.map_or_else(|| root.to_path_buf(), Path::to_path_buf),
				source: source_error
					.into_io_error()
					.unwrap_or_else(|| std::io::Error::other("directory traversal failed")),
			}
		})?;
		let kind = entry.file_type();
		if kind.is_symlink() {
			return Err(VerifyBuildError::SourceAlias {
				path: entry.path().to_path_buf(),
			});
		}
		if !kind.is_dir() && !kind.is_file() {
			return Err(VerifyBuildError::UnsupportedEntry {
				path: entry.path().to_path_buf(),
			});
		}
	}

	Ok(())
}

fn ensure_regular_file(path: &Path) -> Result<(), VerifyBuildError> {
	let metadata = std::fs::symlink_metadata(path).map_err(|_| {
		VerifyBuildError::MissingArtifact {
			path: path.to_path_buf(),
		}
	})?;

	if metadata_is_alias(&metadata) {
		return Err(VerifyBuildError::SourceAlias {
			path: path.to_path_buf(),
		});
	}
	if !metadata.is_file() {
		return Err(VerifyBuildError::MissingArtifact {
			path: path.to_path_buf(),
		});
	}

	Ok(())
}

fn sha256_file(path: &Path) -> Result<String, VerifyBuildError> {
	let mut file = std::fs::File::open(path).map_err(|source| {
		VerifyBuildError::Hash {
			path: path.to_path_buf(),
			source,
		}
	})?;
	let mut hasher = Sha256::new();
	let mut buffer = [0_u8; 16 * 1024];

	loop {
		let read = file.read(&mut buffer).map_err(|source| {
			VerifyBuildError::Hash {
				path: path.to_path_buf(),
				source,
			}
		})?;
		if read == 0 {
			break;
		}
		hasher.update(&buffer[..read]);
	}

	Ok(format!("{:x}", hasher.finalize()))
}

fn executable_hash(path: &Path) -> Result<String, VerifyBuildError> {
	let mut file = std::fs::File::open(path).map_err(|source| {
		VerifyBuildError::Hash {
			path: path.to_path_buf(),
			source,
		}
	})?;
	let mut hasher = Sha256::new();
	let mut buffer = [0_u8; 16 * 1024];
	let zeroes = [0_u8; 16 * 1024];
	let mut pending_zeroes = 0_usize;

	loop {
		let read = file.read(&mut buffer).map_err(|source| {
			VerifyBuildError::Hash {
				path: path.to_path_buf(),
				source,
			}
		})?;
		if read == 0 {
			break;
		}
		if let Some(last_nonzero) = buffer[..read].iter().rposition(|byte| *byte != 0) {
			while pending_zeroes > 0 {
				let count = pending_zeroes.min(zeroes.len());
				hasher.update(&zeroes[..count]);
				pending_zeroes -= count;
			}
			hasher.update(&buffer[..=last_nonzero]);
			pending_zeroes = read - last_nonzero - 1;
		} else {
			pending_zeroes += read;
		}
	}

	Ok(format!("{:x}", hasher.finalize()))
}

fn portable_path(path: &Path) -> String {
	if path.as_os_str().is_empty() {
		return ".".to_owned();
	}

	path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
	use std::cell::RefCell;
	use std::collections::VecDeque;
	use std::fs;

	use tempfile::TempDir;

	use super::*;

	#[derive(Debug, PartialEq, Eq)]
	struct Invocation {
		program: OsString,
		args: Vec<OsString>,
		cwd: PathBuf,
		inherit: bool,
	}

	#[derive(Default)]
	struct FakeRunner {
		outputs: RefCell<VecDeque<Result<CommandOutput, std::io::Error>>>,
		calls: RefCell<Vec<Invocation>>,
	}

	impl FakeRunner {
		fn with_outputs(outputs: Vec<Result<CommandOutput, std::io::Error>>) -> Self {
			Self {
				outputs: RefCell::new(outputs.into()),
				calls: RefCell::new(Vec::new()),
			}
		}

		fn next(
			&self,
			program: &OsStr,
			args: &[OsString],
			cwd: &Path,
			inherit: bool,
		) -> Result<CommandOutput, std::io::Error> {
			self.calls.borrow_mut().push(Invocation {
				program: program.to_owned(),
				args: args.to_vec(),
				cwd: cwd.to_path_buf(),
				inherit,
			});
			self.outputs
				.borrow_mut()
				.pop_front()
				.unwrap_or_else(|| panic!("missing fake command output"))
		}
	}

	impl CommandRunner for FakeRunner {
		fn capture(
			&self,
			program: &OsStr,
			args: &[OsString],
			cwd: &Path,
		) -> Result<CommandOutput, std::io::Error> {
			self.next(program, args, cwd, false)
		}

		fn inherit(
			&self,
			program: &OsStr,
			args: &[OsString],
			cwd: &Path,
		) -> Result<CommandOutput, std::io::Error> {
			self.next(program, args, cwd, true)
		}
	}

	#[allow(clippy::unnecessary_wraps)]
	fn output(success: bool, stdout: &[u8]) -> Result<CommandOutput, std::io::Error> {
		Ok(CommandOutput {
			success,
			status: if success { "ok" } else { "failed" }.to_owned(),
			stdout: stdout.to_vec(),
		})
	}

	fn valid_manifest(hash: &str) -> VerificationManifest {
		VerificationManifest {
			schema_version: 1,
			package_name: "program".to_owned(),
			library_name: "program".to_owned(),
			executable_hash: hash.to_owned(),
			solana_verify_version: SUPPORTED_SOLANA_VERIFY_VERSION.to_owned(),
			build: VerificationBuildInputs {
				mount_path: ".".to_owned(),
				workspace_path: "workspace".to_owned(),
				program_path: "workspace/programs/program".to_owned(),
				library_name: "program".to_owned(),
				features: vec!["bpf-entrypoint".to_owned()],
				default_features: false,
				cargo_lock_sha256: "1".repeat(64),
			},
			source: VerificationSource {
				repository: Some("https://github.com/pina-rs/program".to_owned()),
				revision: Some("2".repeat(40)),
				dirty: Some(false),
			},
			diagnostics: Vec::new(),
		}
	}

	fn test_project(root: &Path) -> Project {
		Project {
			root: root.to_path_buf(),
			program_dir: root.to_path_buf(),
			package_name: "program".to_owned(),
			library_name: "program".to_owned(),
			library_source: root.join("src/lib.rs"),
			target_dir: root.join("target"),
			idl_dir: root.join("target/idl"),
			clients_dir: root.join("clients"),
			clients: Vec::new(),
		}
	}

	fn git_inspection_outputs(
		root: &Path,
		revision: &[u8],
		repository: &[u8],
		status: Result<CommandOutput, std::io::Error>,
	) -> Vec<Result<CommandOutput, std::io::Error>> {
		vec![
			output(true, root.to_string_lossy().as_bytes()),
			output(true, revision),
			output(true, repository),
			status,
		]
	}

	#[test]
	fn version_check_requires_the_exact_supported_release() {
		let runner = FakeRunner::with_outputs(vec![output(true, b"solana-verify 0.5.1\r\n")]);
		check_version(OsStr::new("verify tool"), Path::new("project"), &runner)
			.unwrap_or_else(|error| panic!("version should be accepted: {error}"));
		assert_eq!(
			runner.calls.into_inner(),
			vec![Invocation {
				program: OsString::from("verify tool"),
				args: vec![OsString::from("--version")],
				cwd: PathBuf::from("project"),
				inherit: false,
			}]
		);

		for response in [
			output(true, b"solana-verify 0.5.0\n"),
			output(false, b"solana-verify 0.5.1\n"),
		] {
			let runner = FakeRunner::with_outputs(vec![response]);
			assert!(matches!(
				check_version(OsStr::new("solana-verify"), Path::new("."), &runner),
				Err(VerifyBuildError::UnsupportedVersion)
			));
		}

		let runner = FakeRunner::with_outputs(vec![Err(std::io::Error::new(
			std::io::ErrorKind::NotFound,
			"missing",
		))]);
		assert!(matches!(
			check_version(OsStr::new("missing"), Path::new("."), &runner),
			Err(VerifyBuildError::Run { .. })
		));
	}

	#[test]
	fn build_arguments_keep_paths_and_features_in_separate_values() {
		let args = build_arguments(
			Path::new("source with spaces"),
			Path::new("source with spaces/workspace"),
			"counter_program",
			&["bpf-entrypoint".to_owned(), "logs".to_owned()],
			true,
		);
		assert_eq!(
			args,
			[
				"build",
				"source with spaces",
				"--workspace-path",
				"source with spaces/workspace",
				"--library-name",
				"counter_program",
				"--",
				"--features",
				"bpf-entrypoint,logs",
				"--no-default-features",
			]
			.map(OsString::from)
		);
		assert_eq!(
			build_arguments(
				Path::new("source"),
				Path::new("source"),
				"counter",
				&[],
				false,
			),
			[
				"build",
				"source",
				"--workspace-path",
				"source",
				"--library-name",
				"counter",
				"--",
			]
			.map(OsString::from)
		);
	}

	#[test]
	fn feature_and_diagnostic_metadata_is_strictly_bounded() {
		let valid = [
			"bpf-entrypoint".to_owned(),
			"dep:serde".to_owned(),
			"logging".to_owned(),
			"serde?/derive".to_owned(),
			"simd.v2".to_owned(),
			"télémetry".to_owned(),
		];
		assert!(valid_features(&valid));

		for invalid in [
			Vec::new(),
			vec!["bpf-entrypoint".to_owned(), String::new()],
			vec!["bpf-entrypoint".to_owned(), "bad,feature".to_owned()],
			vec!["bpf-entrypoint".to_owned(), "bad\nfeature".to_owned()],
			vec!["bpf-entrypoint".to_owned(), "bad feature".to_owned()],
			vec!["logging".to_owned(), "bpf-entrypoint".to_owned()],
			vec!["bpf-entrypoint".to_owned(), "bpf-entrypoint".to_owned()],
			vec![
				"bpf-entrypoint".to_owned(),
				"x".repeat(MAX_FEATURE_LENGTH + 1),
			],
			(0..=MAX_FEATURE_COUNT)
				.map(|index| format!("feature-{index:03}"))
				.chain(std::iter::once("bpf-entrypoint".to_owned()))
				.collect(),
			(0..40)
				.map(|index| format!("feature-{index:03}-{}", "x".repeat(110)))
				.chain(std::iter::once("bpf-entrypoint".to_owned()))
				.collect::<std::collections::BTreeSet<_>>()
				.into_iter()
				.collect(),
		] {
			assert!(
				!valid_features(&invalid),
				"unexpected valid features: {invalid:?}"
			);
		}

		assert!(valid_diagnostics(&[
			"remote-revision-reachability-not-checked".to_owned(),
		]));
		for invalid in [
			vec![String::new()],
			vec!["bad diagnostic".to_owned()],
			vec!["bad\ndiagnostic".to_owned()],
			vec!["duplicate".to_owned(), "duplicate".to_owned()],
			vec!["x".repeat(MAX_DIAGNOSTIC_LENGTH + 1)],
			(0..=MAX_DIAGNOSTIC_COUNT)
				.map(|index| format!("diagnostic-{index}"))
				.collect(),
			(0..5)
				.map(|index| format!("diagnostic-{index}-{}", "x".repeat(110)))
				.collect(),
		] {
			assert!(
				!valid_diagnostics(&invalid),
				"unexpected valid diagnostics: {invalid:?}"
			);
		}
	}

	#[test]
	fn verified_build_rejects_invalid_features_before_spawning() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let runner = FakeRunner::with_outputs(Vec::new());
		let result = build_on_supported_host(
			&test_project(temp.path()),
			&["bad,feature".to_owned()],
			false,
			&VerifyBuildOptions::default(),
			&runner,
		);
		assert!(matches!(
			result,
			Err(VerifyBuildError::InvalidFeatureSelection)
		));
		assert!(runner.calls.into_inner().is_empty());
	}

	#[test]
	fn executable_hash_matches_solana_verify_zero_padding_semantics() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let path = temp.path().join("program.so");
		fs::write(&path, b"program\0\0")
			.unwrap_or_else(|error| panic!("fixture write failed: {error}"));
		let expected = format!("{:x}", Sha256::digest(b"program"));
		assert_eq!(
			executable_hash(&path).unwrap_or_else(|error| panic!("hash failed: {error}")),
			expected
		);
		assert_eq!(
			sha256_file(&path).unwrap_or_else(|error| panic!("raw hash failed: {error}")),
			format!("{:x}", Sha256::digest(b"program\0\0"))
		);

		let mut large = vec![0_u8; 2 * 1024 * 1024];
		large[0] = 1;
		large[64 * 1024 + 7] = 2;
		fs::write(&path, &large)
			.unwrap_or_else(|error| panic!("large fixture write failed: {error}"));
		assert_eq!(
			executable_hash(&path).unwrap_or_else(|error| panic!("streaming hash failed: {error}")),
			format!("{:x}", Sha256::digest(&large[..64 * 1024 + 8]))
		);
	}

	#[test]
	fn host_policy_is_explicit() {
		assert!(ensure_supported_host_values("linux", "x86_64", "gnu").is_ok());
		assert!(ensure_supported_host_values("macos", "aarch64", "").is_ok());
		assert!(ensure_supported_host_values("macos", "x86_64", "").is_ok());
		assert!(ensure_supported_host_values("linux", "aarch64", "gnu").is_ok());
		assert!(ensure_supported_host_values("linux", "x86_64", "musl").is_ok());
		assert!(ensure_supported_host_values("windows", "x86_64", "msvc").is_err());
	}

	#[cfg(target_os = "windows")]
	#[test]
	fn verified_build_fails_closed_before_other_validation_on_windows() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let runner = FakeRunner::with_outputs(Vec::new());
		let result = build_with_runner(
			&test_project(temp.path()),
			&["bad,feature".to_owned()],
			false,
			&VerifyBuildOptions::default(),
			&runner,
		);
		assert!(matches!(
			result,
			Err(VerifyBuildError::UnsupportedHost { .. })
		));
		assert!(runner.calls.into_inner().is_empty());
	}

	#[test]
	fn source_urls_paths_and_regular_files_are_validated() {
		assert!(is_public_https_url("https://github.com/pina-rs/pina"));
		assert!(!is_public_https_url("git@github.com:pina-rs/pina"));
		assert!(!is_public_https_url(
			"https://token@github.com/pina-rs/pina"
		));
		assert!(!is_public_https_url("https:///missing-host"));
		assert!(!is_public_https_url(
			"https://github.com/pina-rs/pina?token=secret"
		));
		assert!(!is_public_https_url(
			"https://github.com/pina-rs/pina#token"
		));
		assert!(!is_public_https_url(
			"https://github.com/pina-rs/pina\nsecret"
		));
		for url in [
			"https://localhost/repo",
			"https://intranet/repo",
			"https://service.internal/repo",
			"https://service.test/repo",
			"https://service.invalid/repo",
			"https://service.example/repo",
			"https://github.com:notaport/repo",
			"https://github.com:/repo",
			"https://999.0.0.1/repo",
			"https://-bad.example.com/repo",
			"https://bad-.example.com/repo",
			"https://bad..example.com/repo",
			"https://bad_name.example.com/repo",
			"https://tést.example.com/repo",
			"https://host.local/repo",
			"https://127.0.0.1/repo",
			"https://10.0.0.1/repo",
			"https://169.254.1.1/repo",
			"https://0.0.0.0/repo",
			"https://255.255.255.255/repo",
			"https://192.0.2.1/repo",
			"https://224.0.0.1/repo",
			"https://[::1]/repo",
			"https://[::]/repo",
			"https://[fc00::1]/repo",
			"https://[fe80::1]/repo",
			"https://[2001:db8::1]/repo",
			"https://[ff02::1]/repo",
			"https://[::ffff:127.0.0.1]/repo",
			"https://[::ffff:10.0.0.1]/repo",
			"https://[::1/repo",
			"https://[2606:4700::6810:85e5]suffix/repo",
		] {
			assert!(!is_public_https_url(url), "unexpected public URL: {url}");
		}
		assert!(is_public_https_url("https://1.1.1.1/repo"));
		assert!(is_public_https_url("https://github.com:443/repo"));
		assert!(!is_public_https_url(&format!(
			"https://{}.com/repo",
			"a".repeat(64)
		)));
		assert!(!is_public_https_url(&format!(
			"https://{}.com/repo",
			"a".repeat(250)
		)));
		assert!(is_public_https_url(
			"https://[2606:4700::6810:85e5]:443/repo"
		));
		assert_eq!(portable_path(Path::new("")), ".");
		assert_eq!(
			portable_path(Path::new("programs/counter")),
			"programs/counter"
		);

		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let file = temp.path().join("file");
		fs::write(&file, b"file").unwrap_or_else(|error| panic!("write failed: {error}"));
		assert!(ensure_regular_file(&file).is_ok());
		assert!(matches!(
			ensure_regular_file(temp.path()),
			Err(VerifyBuildError::MissingArtifact { .. })
		));
		assert!(ensure_regular_file(&temp.path().join("missing")).is_err());
	}

	#[test]
	fn record_validation_rejects_untrusted_metadata() {
		let hash = "0".repeat(64);
		let path = PathBuf::from(format!("program-{hash}.json"));
		let manifest = valid_manifest(&hash);
		validate_manifest(&path, &manifest)
			.unwrap_or_else(|error| panic!("valid manifest rejected: {error}"));

		let mut invalid = manifest.clone();
		invalid.schema_version = 2;
		assert!(matches!(
			validate_manifest(&path, &invalid),
			Err(VerifyBuildError::UnsupportedRecordSchema { found: 2 })
		));
		let mut invalid = manifest.clone();
		invalid.solana_verify_version = "0.6.0".to_owned();
		assert!(matches!(
			validate_manifest(&path, &invalid),
			Err(VerifyBuildError::UnsupportedRecordToolVersion { .. })
		));

		let mut cases = Vec::new();
		for bad_hash in ["0".repeat(63), format!("{}z", "0".repeat(63))] {
			let mut invalid = manifest.clone();
			invalid.executable_hash = bad_hash;
			cases.push(invalid);
		}
		let mut invalid = manifest.clone();
		invalid.build.library_name = "different".to_owned();
		cases.push(invalid);
		for library in ["", "bad-name"] {
			let mut invalid = manifest.clone();
			invalid.library_name = library.to_owned();
			invalid.build.library_name = library.to_owned();
			cases.push(invalid);
		}
		for bad_path in ["", "../workspace", "/workspace", "C:\\workspace", "a\\b"] {
			let mut invalid = manifest.clone();
			invalid.build.workspace_path = bad_path.to_owned();
			cases.push(invalid);
		}
		let mut invalid = manifest.clone();
		invalid.source.dirty = Some(true);
		cases.push(invalid);
		for revision in [None, Some("bad".to_owned())] {
			let mut invalid = manifest.clone();
			invalid.source.revision = revision;
			cases.push(invalid);
		}
		let mut invalid = manifest.clone();
		invalid.source.repository = Some("https://127.0.0.1/repo".to_owned());
		cases.push(invalid);
		let mut invalid = manifest.clone();
		invalid.build.features.push("bad,feature".to_owned());
		cases.push(invalid);
		let mut invalid = manifest.clone();
		invalid.diagnostics.push("bad diagnostic".to_owned());
		cases.push(invalid);
		for lock_hash in ["0".repeat(63), format!("{}z", "0".repeat(63))] {
			let mut invalid = manifest.clone();
			invalid.build.cargo_lock_sha256 = lock_hash;
			cases.push(invalid);
		}

		for invalid in cases {
			assert!(matches!(
				validate_manifest(&path, &invalid),
				Err(VerifyBuildError::InvalidRecord { .. })
			));
		}
		assert!(matches!(
			validate_manifest(Path::new("wrong.json"), &manifest),
			Err(VerifyBuildError::InvalidRecord { .. })
		));
	}

	#[test]
	fn record_reader_validates_size_shape_hash_and_accessors() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let artifact_contents = b"program\0\0";
		let hash = format!("{:x}", Sha256::digest(b"program"));
		let record_path = temp.path().join(format!("program-{hash}.json"));
		let artifact_path = record_path.with_extension("so");
		let manifest = valid_manifest(&hash);
		fs::write(&artifact_path, artifact_contents)
			.unwrap_or_else(|error| panic!("artifact write failed: {error}"));
		fs::write(
			&record_path,
			serde_json::to_vec(&manifest)
				.unwrap_or_else(|error| panic!("serialization failed: {error}")),
		)
		.unwrap_or_else(|error| panic!("record write failed: {error}"));

		let record =
			read_record(&record_path).unwrap_or_else(|error| panic!("record read failed: {error}"));
		assert_eq!(record.artifact(), artifact_path);
		assert_eq!(record.library_name(), "program");
		assert_eq!(record.executable_hash(), hash);
		assert_eq!(record.repository(), manifest.source.repository.as_deref());
		assert_eq!(record.revision(), manifest.source.revision.as_deref());
		assert_eq!(record.mount_path(), ".");
		assert_eq!(record.workspace_path(), "workspace");
		assert_eq!(record.program_path(), "workspace/programs/program");
		assert_eq!(record.features(), &["bpf-entrypoint"]);
		assert!(!record.default_features());

		fs::write(&record_path, vec![b' '; 1024 * 1024 + 1])
			.unwrap_or_else(|error| panic!("oversized record write failed: {error}"));
		assert!(matches!(
			read_record(&record_path),
			Err(VerifyBuildError::RecordTooLarge { .. })
		));
		assert!(matches!(
			read_record(&temp.path().join("missing.json")),
			Err(VerifyBuildError::ReadRecord { .. })
		));
	}

	#[test]
	fn bounded_record_reader_reports_io_failures() {
		struct FailingReader;

		impl Read for FailingReader {
			fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
				Err(std::io::Error::other("fixture failure"))
			}
		}

		assert!(matches!(
			read_bounded_record(FailingReader, Path::new("record.json")),
			Err(VerifyBuildError::ReadRecord { .. })
		));
	}

	#[test]
	fn source_inspection_fails_closed_and_reports_diagnostics() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let project = test_project(temp.path());
		let unavailable = [
			vec![Err(std::io::Error::other("git unavailable"))],
			vec![output(false, b"")],
			vec![output(true, b"\xff")],
			vec![output(true, b"/path/that/does/not/exist")],
		];
		for outputs in unavailable {
			let source = inspect_source(&project, &FakeRunner::with_outputs(outputs));
			assert!(source.root.is_none());
			assert_eq!(source.diagnostics, ["source-control-is-unavailable"]);
		}

		let runner = FakeRunner::with_outputs(git_inspection_outputs(
			temp.path(),
			b"bad-revision",
			b"https://intranet/repo",
			Err(std::io::Error::other("status failed")),
		));
		let source = inspect_source(&project, &runner);
		assert!(source.revision.is_none());
		assert!(source.repository.is_none());
		assert!(source.dirty.is_none());
		assert!(
			source
				.diagnostics
				.contains(&"source-repository-is-not-public-https")
		);
		assert!(
			source
				.diagnostics
				.contains(&"source-revision-is-unavailable")
		);
		assert!(source.diagnostics.contains(&"source-state-is-unavailable"));

		let runner = FakeRunner::with_outputs(git_inspection_outputs(
			temp.path(),
			b"0000000000000000000000000000000000000000",
			b"https://github.com/pina-rs/program",
			output(true, b" M src/lib.rs"),
		));
		let source = inspect_source(&project, &runner);
		assert_eq!(source.dirty, Some(true));
		assert!(source.diagnostics.contains(&"source-tree-is-dirty"));
	}

	#[test]
	fn verified_build_preconditions_fail_with_specific_errors() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let canonical_temp = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("temp path canonicalization failed: {error}"));
		let project = test_project(&canonical_temp);
		let options = VerifyBuildOptions::default();
		let version = output(true, b"solana-verify 0.5.1");
		let features = ["bpf-entrypoint".to_owned()];

		let runner =
			FakeRunner::with_outputs(vec![version, Err(std::io::Error::other("git missing"))]);
		assert!(matches!(
			build_on_supported_host(&project, &features, false, &options, &runner),
			Err(VerifyBuildError::SourceControlRequired)
		));

		let mut outputs = vec![output(true, b"solana-verify 0.5.1")];
		outputs.extend(git_inspection_outputs(
			&canonical_temp,
			b"bad",
			b"https://github.com/pina-rs/program",
			output(true, b""),
		));
		let runner = FakeRunner::with_outputs(outputs);
		assert!(matches!(
			build_on_supported_host(&project, &features, false, &options, &runner),
			Err(VerifyBuildError::SourceRevisionUnavailable)
		));

		let mut outputs = vec![output(true, b"solana-verify 0.5.1")];
		outputs.extend(git_inspection_outputs(
			&canonical_temp,
			b"0000000000000000000000000000000000000000",
			b"https://github.com/pina-rs/program",
			output(true, b""),
		));
		let runner = FakeRunner::with_outputs(outputs);
		assert!(matches!(
			build_on_supported_host(&project, &features, false, &options, &runner),
			Err(VerifyBuildError::WorkspaceDiscovery { .. })
		));

		fs::create_dir_all(canonical_temp.join("src"))
			.unwrap_or_else(|error| panic!("source directory failed: {error}"));
		fs::write(
			canonical_temp.join("Cargo.toml"),
			"[package]\nname = \"program\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
		)
		.unwrap_or_else(|error| panic!("manifest write failed: {error}"));
		fs::write(canonical_temp.join("src/lib.rs"), "pub fn program() {}\n")
			.unwrap_or_else(|error| panic!("source write failed: {error}"));
		fs::write(canonical_temp.join("Cargo.lock"), "version = 4\n")
			.unwrap_or_else(|error| panic!("lockfile write failed: {error}"));
		let outside = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let mut outputs = vec![output(true, b"solana-verify 0.5.1")];
		outputs.extend(git_inspection_outputs(
			outside.path(),
			b"0000000000000000000000000000000000000000",
			b"https://github.com/pina-rs/program",
			output(true, b""),
		));
		let runner = FakeRunner::with_outputs(outputs);
		assert!(matches!(
			build_on_supported_host(&project, &features, false, &options, &runner),
			Err(VerifyBuildError::WorkspaceOutsideSource { .. })
		));

		let mut outputs = vec![output(true, b"solana-verify 0.5.1")];
		outputs.extend(git_inspection_outputs(
			&canonical_temp,
			b"0000000000000000000000000000000000000000",
			b"https://github.com/pina-rs/program",
			output(true, b""),
		));
		outputs.push(output(true, b""));
		outputs.push(Err(std::io::Error::other("build spawn failed")));
		let runner = FakeRunner::with_outputs(outputs);
		let error = build_on_supported_host(&project, &features, false, &options, &runner).err();
		assert!(
			matches!(
				error,
				Some(VerifyBuildError::Run {
					command: "solana-verify build",
					..
				})
			),
			"unexpected error: {error:?}"
		);

		let mut outputs = vec![output(true, b"solana-verify 0.5.1")];
		outputs.extend(git_inspection_outputs(
			&canonical_temp,
			b"0000000000000000000000000000000000000000",
			b"https://github.com/pina-rs/program",
			output(true, b""),
		));
		outputs.push(output(true, b""));
		outputs.push(output(true, b""));
		let runner = FakeRunner::with_outputs(outputs);
		assert!(matches!(
			build_on_supported_host(&project, &features, false, &options, &runner),
			Err(VerifyBuildError::MissingArtifact { .. })
		));

		let without_lock =
			TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		assert!(matches!(
			resolve_build_paths(&project, without_lock.path(), without_lock.path()),
			Err(VerifyBuildError::MissingRootLockfile { .. })
		));
		let mut outside_project = project.clone();
		outside_project.program_dir = outside.path().to_path_buf();
		assert!(matches!(
			resolve_build_paths(&outside_project, temp.path(), temp.path()),
			Err(VerifyBuildError::WorkspaceOutsideSource { .. })
		));
	}

	#[test]
	fn record_publication_reports_filesystem_failures() {
		struct FailingRecordFile {
			write_fails: bool,
			commit_fails: bool,
		}

		impl std::io::Write for FailingRecordFile {
			fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
				if self.write_fails {
					return Err(std::io::Error::other("write failed"));
				}
				Ok(buffer.len())
			}

			fn flush(&mut self) -> std::io::Result<()> {
				Ok(())
			}
		}

		impl RecordFile for FailingRecordFile {
			fn commit(self) -> Result<(), std::io::Error> {
				if self.commit_fails {
					return Err(std::io::Error::other("commit failed"));
				}
				Ok(())
			}
		}

		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let snapshot = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let artifact = snapshot.path().join("program.so");
		fs::write(&artifact, b"program")
			.unwrap_or_else(|error| panic!("artifact write failed: {error}"));
		let hash = format!("{:x}", Sha256::digest(b"program"));
		let verified = VerifiedBuild {
			_snapshot: snapshot,
			artifact,
			program_dir: temp.path().to_path_buf(),
			manifest: valid_manifest(&hash),
		};

		fs::create_dir(temp.path().join("blocked"))
			.unwrap_or_else(|error| panic!("fixture directory failed: {error}"));
		fs::write(temp.path().join("blocked/pina"), b"file")
			.unwrap_or_else(|error| panic!("fixture file failed: {error}"));
		assert!(matches!(
			publish_record(&verified, &temp.path().join("blocked")),
			Err(VerifyBuildError::PublishRecord { .. })
		));
		assert!(matches!(
			copy_atomic(&temp.path().join("missing"), &temp.path().join("output")),
			Err(VerifyBuildError::PublishRecord { .. })
		));
		assert!(matches!(
			write_atomic(&temp.path().join("missing/output"), b"data"),
			Err(VerifyBuildError::PublishRecord { .. })
		));
		assert!(matches!(
			write_atomic(temp.path(), b"data"),
			Err(VerifyBuildError::PublishRecord { .. })
		));
		for file in [
			FailingRecordFile {
				write_fails: true,
				commit_fails: false,
			},
			FailingRecordFile {
				write_fails: false,
				commit_fails: true,
			},
		] {
			assert!(matches!(
				complete_atomic_write(file, Path::new("record"), b"data"),
				Err(VerifyBuildError::PublishRecord { .. })
			));
		}
		let mut successful_file = FailingRecordFile {
			write_fails: false,
			commit_fails: false,
		};
		std::io::Write::flush(&mut successful_file)
			.unwrap_or_else(|error| panic!("flush failed: {error}"));
		RecordFile::commit(successful_file)
			.unwrap_or_else(|error| panic!("commit failed: {error}"));
	}

	#[cfg(unix)]
	#[test]
	fn snapshot_and_hash_helpers_reject_special_or_unreadable_inputs() {
		use std::os::unix::fs::PermissionsExt;
		use std::os::unix::net::UnixListener;

		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let socket = temp.path().join("socket");
		let _listener = UnixListener::bind(&socket)
			.unwrap_or_else(|error| panic!("socket fixture failed: {error}"));
		assert!(matches!(
			audit_snapshot(temp.path()),
			Err(VerifyBuildError::UnsupportedEntry { .. })
		));
		assert!(matches!(
			sha256_file(&temp.path().join("missing")),
			Err(VerifyBuildError::Hash { .. })
		));
		assert!(matches!(
			executable_hash(&temp.path().join("missing")),
			Err(VerifyBuildError::Hash { .. })
		));
		assert!(matches!(
			sha256_file(temp.path()),
			Err(VerifyBuildError::Hash { .. })
		));
		assert!(matches!(
			executable_hash(temp.path()),
			Err(VerifyBuildError::Hash { .. })
		));

		let long_name = "x".repeat(5000);
		assert!(matches!(
			ensure_record_path(&temp.path().join(long_name), false),
			Err(VerifyBuildError::PublishRecord { .. })
		));

		let unreadable = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let locked = unreadable.path().join("locked");
		fs::create_dir(&locked).unwrap_or_else(|error| panic!("locked fixture failed: {error}"));
		fs::write(locked.join("file"), b"file")
			.unwrap_or_else(|error| panic!("locked file failed: {error}"));
		fs::set_permissions(&locked, fs::Permissions::from_mode(0o0))
			.unwrap_or_else(|error| panic!("locking fixture failed: {error}"));
		let result = audit_snapshot(unreadable.path());
		fs::set_permissions(&locked, fs::Permissions::from_mode(0o700))
			.unwrap_or_else(|error| panic!("unlocking fixture failed: {error}"));
		assert!(matches!(result, Err(VerifyBuildError::CopySource { .. })));
	}

	#[test]
	fn default_options_select_the_supported_executable_name() {
		assert_eq!(
			VerifyBuildOptions::default().executable,
			OsString::from("solana-verify")
		);
	}

	#[cfg(unix)]
	#[test]
	fn snapshot_audit_rejects_symbolic_links() {
		use std::os::unix::fs::symlink;

		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		fs::write(temp.path().join("file"), b"file")
			.unwrap_or_else(|error| panic!("write failed: {error}"));
		assert!(audit_snapshot(temp.path()).is_ok());
		symlink("file", temp.path().join("alias"))
			.unwrap_or_else(|error| panic!("symlink failed: {error}"));
		assert!(matches!(
			audit_snapshot(temp.path()),
			Err(VerifyBuildError::SourceAlias { .. })
		));
		assert!(matches!(
			ensure_regular_file(&temp.path().join("alias")),
			Err(VerifyBuildError::SourceAlias { .. })
		));
		assert!(matches!(
			ensure_record_path(&temp.path().join("alias"), false),
			Err(VerifyBuildError::RecordAlias { .. })
		));
		let directory_alias = temp.path().join("directory-alias");
		symlink(temp.path(), &directory_alias)
			.unwrap_or_else(|error| panic!("directory symlink failed: {error}"));
		assert!(matches!(
			ensure_record_path(&directory_alias, true),
			Err(VerifyBuildError::RecordAlias { .. })
		));
	}

	#[test]
	fn tracked_source_staging_uses_a_single_structured_git_invocation() {
		let runner = FakeRunner::with_outputs(vec![output(true, b"")]);
		stage_tracked_source(Path::new("repo"), Path::new("stage with spaces"), &runner)
			.unwrap_or_else(|error| panic!("staging failed: {error}"));
		let calls = runner.calls.into_inner();
		assert_eq!(calls.len(), 1);
		assert_eq!(calls[0].program, OsString::from("git"));
		assert_eq!(calls[0].args[0], OsString::from("checkout-index"));
		assert_eq!(calls[0].args[1], OsString::from("--all"));
		assert_eq!(calls[0].args[2], OsString::from("--force"));
		assert!(
			calls[0].args[3]
				.to_string_lossy()
				.starts_with("--prefix=stage with spaces")
		);

		let runner = FakeRunner::with_outputs(vec![output(false, b"")]);
		assert!(matches!(
			stage_tracked_source(Path::new("repo"), Path::new("stage"), &runner),
			Err(VerifyBuildError::StageTrackedSource { .. })
		));
		let runner = FakeRunner::with_outputs(vec![Err(std::io::Error::other("failed"))]);
		assert!(matches!(
			stage_tracked_source(Path::new("repo"), Path::new("stage"), &runner),
			Err(VerifyBuildError::Run { .. })
		));
	}

	#[test]
	fn command_runner_inherit_records_the_execution_mode() {
		let runner = FakeRunner::with_outputs(vec![output(true, b"")]);
		let result = runner
			.inherit(
				OsStr::new("tool"),
				&[OsString::from("build")],
				Path::new("."),
			)
			.unwrap_or_else(|error| panic!("inherit failed: {error}"));
		assert!(result.success);
		assert!(runner.calls.borrow()[0].inherit);
	}

	#[test]
	fn process_runner_closes_stdin_for_noninteractive_builds() {
		let current_exe = std::env::current_exe()
			.unwrap_or_else(|error| panic!("test executable lookup failed: {error}"));
		let mut child = Command::new(&current_exe)
			.args([
				"--exact",
				"verifiable::tests::process_runner_stdin_parent_helper",
				"--nocapture",
			])
			.env("PINA_PROCESS_RUNNER_STDIN_TEST", "1")
			.stdin(Stdio::piped())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()
			.unwrap_or_else(|error| panic!("stdin parent spawn failed: {error}"));
		let open_stdin = child.stdin.take();
		assert!(wait_for_stdin_probe(child, open_stdin, 100));

		let mut blocking_child = Command::new(current_exe)
			.args([
				"--exact",
				"verifiable::tests::process_runner_stdin_leaf_helper",
				"--nocapture",
			])
			.env("PINA_PROCESS_RUNNER_STDIN_TEST", "1")
			.stdin(Stdio::piped())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()
			.unwrap_or_else(|error| panic!("blocking stdin probe spawn failed: {error}"));
		let open_stdin = blocking_child.stdin.take();
		assert!(!wait_for_stdin_probe(blocking_child, open_stdin, 0));
	}

	fn wait_for_stdin_probe(
		mut child: std::process::Child,
		open_stdin: Option<std::process::ChildStdin>,
		attempts: usize,
	) -> bool {
		for _ in 0..attempts {
			if let Some(status) = child
				.try_wait()
				.unwrap_or_else(|error| panic!("stdin parent wait failed: {error}"))
			{
				return status.success();
			}
			std::thread::sleep(std::time::Duration::from_millis(10));
		}

		drop(open_stdin);
		let _ = child.wait();
		false
	}

	#[test]
	fn process_runner_stdin_parent_helper() {
		if std::env::var_os("PINA_PROCESS_RUNNER_STDIN_TEST").is_none() {
			return;
		}

		let current_exe = std::env::current_exe()
			.unwrap_or_else(|error| panic!("test executable lookup failed: {error}"));
		let output = ProcessRunner
			.inherit(
				current_exe.as_os_str(),
				&[
					OsString::from("--exact"),
					OsString::from("verifiable::tests::process_runner_stdin_leaf_helper"),
					OsString::from("--nocapture"),
				],
				Path::new("."),
			)
			.unwrap_or_else(|error| panic!("stdin leaf spawn failed: {error}"));
		assert!(output.success, "stdin leaf failed: {}", output.status);
	}

	#[test]
	fn process_runner_stdin_leaf_helper() {
		if std::env::var_os("PINA_PROCESS_RUNNER_STDIN_TEST").is_none() {
			return;
		}

		let mut input = Vec::new();
		std::io::stdin()
			.read_to_end(&mut input)
			.unwrap_or_else(|error| panic!("stdin read failed: {error}"));
		assert!(input.is_empty(), "stdin must be closed");
	}
}
