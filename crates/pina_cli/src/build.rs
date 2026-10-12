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

use crate::compute_units::MeasurementUse;
use crate::error::IdlError;
use crate::generate_idl;
use crate::generate_idl_with;
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

	#[error(
		"`sbpf-linker` is required for the upstream build but could not be run: {source}. \
	         Install it with `cargo binstall sbpf-linker` (or run `install:all` in this \
	         repository), keep it on PATH, and read docs/sbpf-linker-evaluation.md for the \
	         measured caveats of the experimental upstream toolchain"
	)]
	MissingSbpfLinker { source: std::io::Error },

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
	build_project_with_options(&default_build_options(start))
}

/// The options `pina build` uses without flags.
fn default_build_options(start: &Path) -> BuildOptions {
	BuildOptions {
		project_dir: start.to_path_buf(),
		features: Vec::new(),
		no_default_features: false,
		size_profile: SizeProfile::default(),
	}
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
	build_with_measurements(options, MeasurementUse::Lenient)
}

/// Build a project for `pina test --record-compute-units`.
///
/// The IDL this build writes carries no compute unit budgets: the recording
/// replaces `compute-units.json`, so whatever the old file says, including
/// measurements of instructions the program no longer declares, must not stop
/// the run that fixes it.
pub(crate) fn build_project_for_recording(start: &Path) -> Result<BuildOutput, BuildError> {
	build_with_measurements(&default_build_options(start), MeasurementUse::Skip)
}

fn build_with_measurements(
	options: &BuildOptions,
	measurements: MeasurementUse,
) -> Result<BuildOutput, BuildError> {
	let project = Project::discover(&options.project_dir)?;
	check_migrations_for_build(&project)?;
	// `sbpf-linker = true` under `[project]` in pina.toml (written by `pina
	// init --sbpf-linker`) routes every build to the upstream toolchain;
	// removing the key reverts to the cargo-build-sbf driver.
	if project.sbpf_linker {
		return build_project_upstream(&project, options);
	}
	if options.size_profile.requests_lto() {
		warn_lto_unavailable(&project);
	}
	let compiler_artifact = compile_sbf(
		&project,
		options,
		&SbfCompilation {
			out_dir: &project.target_dir.join("sbf-build"),
			line_tables: false,
			quiet_stdout: false,
		},
	)?;

	let idl = generate_idl_with(
		&project.program_dir,
		Some(&project.library_name),
		measurements,
	)
	.map_err(|source| {
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
		package_name: project.package_name.clone(),
		sbf_artifact,
		idl: idl_path,
	})
}

/// Where and how one `cargo build-sbf` run writes its artifact.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SbfCompilation<'a> {
	/// The `--sbf-out-dir` that receives the stripped program.
	pub(crate) out_dir: &'a Path,
	/// Emit DWARF line tables and keep the linker output's symbols.
	pub(crate) line_tables: bool,
	/// Send the compiler's stdout to stderr so stdout stays machine-readable.
	pub(crate) quiet_stdout: bool,
}

/// Run `cargo build-sbf` for `project` and return the stripped program it
/// published in `compilation.out_dir`.
///
/// The unstripped linker output stays under the Cargo target directory; see
/// [`unstripped_artifact`].
pub(crate) fn compile_sbf(
	project: &Project,
	options: &BuildOptions,
	compilation: &SbfCompilation<'_>,
) -> Result<PathBuf, BuildError> {
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
	let args = build_sbf_args(
		project,
		&manifest_path,
		&features,
		options,
		compilation.out_dir,
	);
	let command_label = command_label(&cargo, &args);
	let mut command = Command::new(&cargo);
	command
		.current_dir(&project.root)
		.env("CARGO_TARGET_DIR", &project.target_dir)
		.args(&args);
	apply_size_profile(&mut command, options, declared_release_profile(project));

	if compilation.line_tables {
		apply_line_tables(&mut command);
	}

	if compilation.quiet_stdout {
		command.stdout(std::io::stderr());
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

	let artifact = compilation
		.out_dir
		.join(format!("{}.so", project.library_name));

	if !artifact.is_file() {
		return Err(BuildError::MissingArtifact { path: artifact });
	}

	Ok(artifact)
}

/// Request DWARF line tables and an unstripped linker output.
///
/// Both are Cargo profile overrides, which `cargo build-sbf` passes through
/// to its Cargo invocation. They override the manifest, including the
/// `strip = true` that `pina init` scaffolds. Debug information can change
/// SBF code generation, so callers compare the result with a release build.
fn apply_line_tables(command: &mut Command) {
	command
		.env("CARGO_PROFILE_RELEASE_DEBUG", "line-tables-only")
		.env("CARGO_PROFILE_RELEASE_STRIP", "none");
}

/// The unstripped linker output of the project's latest SBF release build.
pub(crate) fn unstripped_artifact(project: &Project) -> Option<PathBuf> {
	pina_profile::elf::UNSTRIPPED_ARTIFACT_DIRS
		.iter()
		.map(|directory| {
			project
				.target_dir
				.join(directory)
				.join(format!("{}.so", project.library_name))
		})
		.find(|path| path.is_file())
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
pub(crate) struct DeclaredReleaseProfile {
	pub(crate) lto: Option<bool>,
	pub(crate) codegen_units: Option<u32>,
	pub(crate) opt_level: Option<u32>,
	pub(crate) overflow_checks: Option<bool>,
}

/// Read `[profile.release]` from the workspace manifest that owns the build.
///
/// Cargo applies `[profile]` tables from the workspace root manifest only, so
/// that is the file consulted. A missing file, table, or key all mean the
/// manifest expresses no opinion, which lets the size profile choose.
pub(crate) fn declared_release_profile(project: &Project) -> DeclaredReleaseProfile {
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
		opt_level: release
			.get("opt-level")
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

	// What each backend would build. The ordinary build applies the profile on
	// top of the manifest; the verified build gets the manifest as-is, with
	// Cargo's release defaults filling in whatever the manifest leaves unset.
	let verified_lto = declared.lto.unwrap_or(false);
	let verified_codegen_units = declared.codegen_units.unwrap_or(16);
	let verified_opt_level = declared.opt_level.unwrap_or(3);
	let verified_overflow_checks = declared.overflow_checks.unwrap_or(false);
	let ordinary_lto = if size_profile.requests_lto() {
		true
	} else if size_profile.forbids_lto() {
		false
	} else {
		declared.lto.unwrap_or(false)
	};
	let ordinary_overflow_checks =
		size_profile.keeps_overflow_checks() || declared.overflow_checks == Some(true);

	if verified_overflow_checks != ordinary_overflow_checks {
		return Some("its `overflow-checks` setting");
	}
	if verified_codegen_units != 1 {
		return Some("its `codegen-units` setting");
	}
	if verified_opt_level != 3 {
		return Some("its `opt-level` setting");
	}
	if verified_lto != ordinary_lto {
		return Some("its `lto` setting");
	}

	None
}

/// Warn when a verified build will not match the ordinary deploy artifact.
fn warn_verified_profile_divergence(project: &Project, size_profile: SizeProfile) {
	let declared = declared_release_profile(project);
	if let Some(reason) = verified_profile_divergence(declared, size_profile) {
		eprintln!(
			"warning: `pina build --verify` does not apply the size profile because a verified \
			 artifact must stay reproducible from its recorded Git revision, so the verified \
			 build keeps {reason} from the workspace manifest. The verified artifact will differ \
			 from the artifact an ordinary `pina build` deploys. Declare the settings under \
			 `[profile.release]` in the workspace manifest (as `pina init` does) so both backends \
			 shrink, or build without `--verify` and accept the difference."
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
	out_dir: &Path,
) -> Vec<OsString> {
	let mut args = vec![
		OsString::from("build-sbf"),
		OsString::from("--manifest-path"),
		manifest_path.as_os_str().to_owned(),
		OsString::from("--sbf-out-dir"),
		out_dir.as_os_str().to_owned(),
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
		 into a separate crate, or pass --no-lto to silence this warning. Check the entrypoint's \
		 stack frame after switching: LTO inlines every handler into it, and a program with many \
		 instructions can exceed the 4 KB per-frame limit in a build that still exits 0. Raise \
		 the limit with cargo build-sbf --sbf-stack-size <BYTES> when it does; see \
		 docs/src/program-size.md."
	);
}

/// Maximum number of libcall exports the upstream build adds across retries.
const UPSTREAM_EXPORT_RETRY_LIMIT: usize = 8;

/// The `sbpf-linker` binary name, resolved from PATH by rustc's `-C linker`.
const SBPF_LINKER: &str = "sbpf-linker";

/// Rustflags for the upstream `bpfel-unknown-none` build.
///
/// Mirrors the `[target.bpfel-unknown-none]` table in the repository's
/// `.cargo/config.toml` and the `--sbpf-linker` variant of `pina init`
/// scaffolding (the blueshift-gg solana-upstream-bpf-template flag set). The
/// two `--cfg` flags are load-bearing: they switch pinocchio's syscall
/// externs to static hash-encoded calls, which sbpf-linker links correctly;
/// without them the relocation-based fallback produces programs that load
/// but silently corrupt data. `extra_exports` appends the `--export=<symbol>`
/// requests collected from the linker's missing-symbol diagnostics.
fn upstream_rustflags(extra_exports: &[String]) -> String {
	let mut flags = String::from(
		"-C linker=sbpf-linker -C panic=abort -C relocation-model=static -C target-cpu=v4 \
		 -C target-feature=+allows-misaligned-mem-access -C \
		 link-arg=--llvm-args=--bpf-stack-size=4096 \
		 -C link-arg=--llvm-args=--bpf-max-stores-per-memfunc=5 -C \
		 link-arg=--llvm-args=--disable-gotox -C link-arg=--llvm-args=--disable-ldsx -C \
		 link-arg=--llvm-args=--disable-movsx --cfg=target_os=\"solana\" \
		 --cfg=target_feature=\"static-syscalls\" -A explicit_builtin_cfgs_in_flags",
	);
	for symbol in extra_exports {
		flags.push_str(" -C link-arg=--export=");
		flags.push_str(symbol);
	}
	flags
}

/// Arguments that drive an upstream sbpf-linker build with plain cargo.
///
/// Unlike the `cargo-build-sbf` driver, plain `cargo build` needs the
/// `bpfel-unknown-none` target and the build-std sysroot on the command line;
/// the linker and its tuning flags arrive through
/// `CARGO_TARGET_BPFEL_UNKNOWN_NONE_RUSTFLAGS` (see the `upstream_rustflags`
/// helper).
fn upstream_bpf_args(
	manifest_path: &Path,
	features: &str,
	options: &BuildOptions,
) -> Vec<OsString> {
	let mut args = vec![
		OsString::from("build"),
		OsString::from("--release"),
		OsString::from("--manifest-path"),
		manifest_path.as_os_str().to_owned(),
		OsString::from("--target"),
		OsString::from("bpfel-unknown-none"),
		OsString::from("--features"),
		OsString::from(features),
		OsString::from("-Z"),
		OsString::from("build-std=core,alloc"),
	];

	if options.no_default_features {
		args.push(OsString::from("--no-default-features"));
	}

	args
}

/// The libcall symbol named by sbpf-linker's missing-export diagnostic.
///
/// sbpf-linker reports one missing symbol at a time, for example
/// ``Symbol __udivti3 is missing. Export it with the `--export=__udivti3` flag.``
/// The upstream build retries with the suggested export appended, so u128
/// division programs link without manual flag bookkeeping.
fn missing_export_symbol(output: &str) -> Option<String> {
	let start = output.find("Symbol ")? + "Symbol ".len();
	let rest = &output[start..];
	let end = rest.find(" is missing")?;
	if rest[..end].is_empty() {
		return None;
	}
	Some(rest[..end].to_owned())
}

/// The `sbpf-linker` binary, overridable for pinned checkouts and tests.
///
/// `PINA_SBPF_LINKER` can name or point at the binary — for example the
/// repository pin at `.bin/rust-nightly/sbpf-linker/<version>/bin/sbpf-linker`.
/// The default is `sbpf-linker`, resolved from `PATH` by both this probe and
/// rustc's `-C linker` flag.
fn sbpf_linker_command() -> OsString {
	std::env::var_os("PINA_SBPF_LINKER").unwrap_or_else(|| OsString::from(SBPF_LINKER))
}

/// Fail early when the sbpf-linker binary cannot be spawned.
///
/// Returns the directory to prepend to the cargo child's `PATH` when the
/// override names a specific file, so rustc's `-C linker=sbpf-linker` and
/// the probe resolve to the same binary.
fn ensure_sbpf_linker_runnable(linker: &OsStr) -> Result<Option<PathBuf>, BuildError> {
	if let Err(source) = Command::new(linker).arg("--version").output() {
		return Err(BuildError::MissingSbpfLinker { source });
	}
	let path = PathBuf::from(linker);
	Ok(if path.components().count() > 1 {
		path.parent().map(Path::to_path_buf)
	} else {
		None
	})
}

/// Apply release-profile overrides for the upstream sbpf-linker build.
///
/// LTO is always disabled: fat LTO under sbpf-linker inflates runtime
/// compute units by 17-77% on pina examples and makes u128 division
/// unresolvable at link time, so the override beats a manifest that declares
/// `lto` (which `pina init` scaffolds). The remaining overrides mirror
/// [`apply_size_profile`].
fn apply_upstream_profile(
	command: &mut Command,
	options: &BuildOptions,
	declared: DeclaredReleaseProfile,
) {
	command.env("CARGO_PROFILE_RELEASE_LTO", "false");
	if !options.size_profile.applies_overrides() {
		return;
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

/// Build a project with upstream nightly Rust and the sbpf-linker binary.
///
/// Selected by `sbpf-linker = true` under `[project]` in pina.toml (`pina
/// init --sbpf-linker` records it): every `pina build` compiles for the
/// upstream `bpfel-unknown-none` target with `-Z build-std`, links with the
/// `sbpf-linker` binary from PATH, and encodes syscalls statically
/// (see the `upstream_rustflags` helper). Requires a nightly toolchain with `rust-src`,
/// which `pina init` scaffolds. The build runs without LTO and retries with
/// `--export=<symbol>` flags, up to `UPSTREAM_EXPORT_RETRY_LIMIT` times,
/// when the linker reports missing libcall symbols. The cdylib lands in
/// `target/bpfel-unknown-none/release` and is published like any build. An
/// explicit `--verify` build is unaffected and keeps the solana-verify
/// backend.
///
/// # Errors
///
/// Returns the same discovery, Cargo, extraction, and publication errors as
/// [`build_project_with_options`], plus [`BuildError::MissingSbpfLinker`]
/// when the linker binary cannot be spawned.
pub fn build_project_upstream(
	project: &Project,
	options: &BuildOptions,
) -> Result<BuildOutput, BuildError> {
	let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
	let linker = sbpf_linker_command();
	build_project_upstream_with(project, options, &cargo, &linker)
}

/// Drive one upstream build, from cargo invocation through IDL publication.
///
/// Split from [`build_project_upstream`] so unit tests can supply the cargo
/// and sbpf-linker paths directly — the wrapper reads them from environment
/// variables, which parallel tests cannot safely set.
fn build_project_upstream_with(
	project: &Project,
	options: &BuildOptions,
	cargo: &OsStr,
	linker: &OsStr,
) -> Result<BuildOutput, BuildError> {
	eprintln!("note: building with the upstream sbpf-linker (`sbpf-linker = true` in pina.toml)");
	let manifest_path = project.program_dir.join("Cargo.toml");
	let features = options
		.features
		.iter()
		.map(String::as_str)
		.chain(std::iter::once("bpf-entrypoint"))
		.collect::<BTreeSet<_>>()
		.into_iter()
		.collect::<Vec<_>>()
		.join(",");
	let linker_dir = ensure_sbpf_linker_runnable(linker)?;

	let mut exports: Vec<String> = Vec::new();
	let output = loop {
		let args = upstream_bpf_args(&manifest_path, &features, options);
		let command_label = command_label(cargo, &args);
		let mut command = Command::new(cargo);
		command
			.current_dir(&project.root)
			.env("CARGO_TARGET_DIR", &project.target_dir)
			.env(
				"CARGO_TARGET_BPFEL_UNKNOWN_NONE_RUSTFLAGS",
				upstream_rustflags(&exports),
			)
			.args(&args);
		if let Some(dir) = &linker_dir {
			// The `:` separator is unix-only, matching sbpf-linker itself,
			// which does not ship Windows binaries. `join_paths` was tried
			// but re-joining the devenv shell's very long PATH breaks the
			// child's ability to find bash through `#!/usr/bin/env`.
			let mut prefixed = dir.as_os_str().to_owned();
			prefixed.push(":");
			if let Some(path) = std::env::var_os("PATH") {
				prefixed.push(path);
			}
			command.env("PATH", prefixed);
		}
		apply_upstream_profile(&mut command, options, declared_release_profile(project));

		let output = command.output().map_err(|source| {
			BuildError::RunCargo {
				command: command_label.clone(),
				source,
			}
		})?;

		if output.status.success() {
			break output;
		}

		let diagnostics = format!(
			"{}{}",
			String::from_utf8_lossy(&output.stderr),
			String::from_utf8_lossy(&output.stdout),
		);
		if let Some(symbol) = missing_export_symbol(&diagnostics)
			&& !exports.contains(&symbol)
			&& exports.len() < UPSTREAM_EXPORT_RETRY_LIMIT
		{
			exports.push(symbol);
			continue;
		}

		// `command.output()` captures both streams; surface the compiler
		// diagnostics so upstream-build failures are diagnosable.
		eprint!("{diagnostics}");
		return Err(BuildError::CargoFailed {
			command: command_label,
			status: output.status.to_string(),
		});
	};
	drop(output);

	let compiler_artifact = project
		.target_dir
		.join("bpfel-unknown-none")
		.join("release")
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
		package_name: project.package_name.clone(),
		sbf_artifact,
		idl: idl_path,
	})
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
	#[cfg(unix)]
	use std::os::unix::fs::PermissionsExt;

	use tempfile::TempDir;

	use super::*;

	#[test]
	fn upstream_args_target_the_upstream_bpf_target() {
		let options = BuildOptions {
			project_dir: PathBuf::new(),
			features: Vec::new(),
			no_default_features: false,
			size_profile: SizeProfile::Production,
		};
		let args = upstream_bpf_args(
			Path::new("/tmp/prog/Cargo.toml"),
			"bpf-entrypoint,logs",
			&options,
		);
		let args: Vec<String> = args
			.iter()
			.map(|argument| argument.to_string_lossy().into_owned())
			.collect();
		assert_eq!(
			args,
			vec![
				"build",
				"--release",
				"--manifest-path",
				"/tmp/prog/Cargo.toml",
				"--target",
				"bpfel-unknown-none",
				"--features",
				"bpf-entrypoint,logs",
				"-Z",
				"build-std=core,alloc",
			]
		);
	}

	#[test]
	fn upstream_args_forward_no_default_features() {
		let options = BuildOptions {
			project_dir: PathBuf::new(),
			features: Vec::new(),
			no_default_features: true,
			size_profile: SizeProfile::Production,
		};
		let args = upstream_bpf_args(Path::new("Cargo.toml"), "bpf-entrypoint", &options);
		let args: Vec<String> = args
			.iter()
			.map(|argument| argument.to_string_lossy().into_owned())
			.collect();
		assert_eq!(
			args.last().map(String::as_str),
			Some("--no-default-features")
		);
	}

	#[test]
	fn upstream_rustflags_enable_static_syscalls() {
		let flags = upstream_rustflags(&[]);
		assert!(flags.contains("-C linker=sbpf-linker"));
		assert!(flags.contains("--cfg=target_os=\"solana\""));
		assert!(flags.contains("--cfg=target_feature=\"static-syscalls\""));
	}

	#[test]
	fn upstream_rustflags_append_missing_export_requests() {
		let flags = upstream_rustflags(&["__udivti3".to_owned()]);
		assert!(flags.ends_with(" -C link-arg=--export=__udivti3"));
	}

	#[test]
	fn missing_export_symbol_parses_the_linker_diagnostic() {
		let output = "error: linking with `sbpf-linker` failed: exit status: 1\n  = note: \
		 Error: Symbol __udivti3 is missing. Export it with the `--export=__udivti3` flag.";
		assert_eq!(missing_export_symbol(output).as_deref(), Some("__udivti3"));
	}

	#[test]
	fn missing_export_symbol_ignores_other_diagnostics() {
		assert_eq!(
			missing_export_symbol("ERROR llvm: stack limit exceeded"),
			None
		);
		assert_eq!(missing_export_symbol("Symbol  is missing"), None);
	}

	#[test]
	fn upstream_profile_forces_lto_off_even_for_none_profile() {
		let mut command = Command::new("cargo");
		apply_upstream_profile(
			&mut command,
			&BuildOptions {
				project_dir: PathBuf::new(),
				features: Vec::new(),
				no_default_features: false,
				size_profile: SizeProfile::None,
			},
			DeclaredReleaseProfile::default(),
		);
		assert_eq!(
			command
				.get_envs()
				.find(|(key, _)| *key == "CARGO_PROFILE_RELEASE_LTO")
				.and_then(|(_, value)| value),
			Some(std::ffi::OsStr::new("false"))
		);
		assert!(
			command
				.get_envs()
				.find(|(key, _)| *key == "CARGO_PROFILE_RELEASE_CODEGEN_UNITS")
				.is_none(),
			"SizeProfile::None should not set codegen-units"
		);
	}

	#[test]
	fn upstream_profile_sets_production_overrides() {
		let mut command = Command::new("cargo");
		apply_upstream_profile(
			&mut command,
			&BuildOptions {
				project_dir: PathBuf::new(),
				features: Vec::new(),
				no_default_features: false,
				size_profile: SizeProfile::Production,
			},
			DeclaredReleaseProfile {
				overflow_checks: Some(true),
				..DeclaredReleaseProfile::default()
			},
		);
		let envs: Vec<_> = command.get_envs().collect();
		let env = |name: &str| {
			envs.iter()
				.find(|(key, _)| *key == std::ffi::OsStr::new(name))
				.and_then(|(_, value)| *value)
		};
		assert_eq!(
			env("CARGO_PROFILE_RELEASE_LTO"),
			Some(std::ffi::OsStr::new("false"))
		);
		assert_eq!(
			env("CARGO_PROFILE_RELEASE_CODEGEN_UNITS"),
			Some(std::ffi::OsStr::new("1"))
		);
		assert_eq!(
			env("CARGO_PROFILE_RELEASE_OPT_LEVEL"),
			Some(std::ffi::OsStr::new("3"))
		);
		assert_eq!(
			env("CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS"),
			Some(std::ffi::OsStr::new("true"))
		);
	}

	#[test]
	fn upstream_profile_respects_explicit_overflow_checks_request() {
		let mut command = Command::new("cargo");
		apply_upstream_profile(
			&mut command,
			&BuildOptions {
				project_dir: PathBuf::new(),
				features: Vec::new(),
				no_default_features: false,
				size_profile: SizeProfile::ProductionWithOverflowChecks,
			},
			DeclaredReleaseProfile::default(),
		);
		assert_eq!(
			command
				.get_envs()
				.find(|(key, _)| *key == "CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS")
				.and_then(|(_, value)| value),
			Some(std::ffi::OsStr::new("true"))
		);
	}

	#[test]
	fn build_wrapper_forwards_discovery_errors() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let error = build_project(&temp.path().join("missing"))
			.expect_err("missing project should fail discovery");

		assert!(matches!(error, BuildError::Project(_)));
	}

	/// Write a dependency-free pina project fixture whose `pina.toml` carries
	/// the upstream key and whose `cargo metadata` resolves offline.
	#[cfg(unix)]
	fn upstream_fixture(root: &Path) -> Project {
		fs::create_dir_all(root.join("src"))
			.unwrap_or_else(|error| panic!("create source: {error}"));
		fs::write(
			root.join("Cargo.toml"),
			r#"[package]
name = "upstream_test"
version = "0.0.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[features]
bpf-entrypoint = []
"#,
		)
		.unwrap_or_else(|error| panic!("write manifest: {error}"));
		fs::write(
			root.join("pina.toml"),
			r#"[project]
program = "."
sbpf-linker = true
"#,
		)
		.unwrap_or_else(|error| panic!("write pina.toml: {error}"));
		fs::write(
			root.join("src/lib.rs"),
			r#"use pina::*;

declare_id!("11111111111111111111111111111111");

#[discriminator]
pub enum UpstreamInstruction {
	Hello = 0,
}

#[instruction(discriminator = UpstreamInstruction::Hello)]
pub struct HelloInstruction {}

#[derive(Accounts)]
pub struct HelloAccounts<'a> {
	pub user: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for HelloAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = HelloInstruction::try_from_bytes(data)?;
		self.user.assert_signer()?;
		Ok(())
	}
}

pub fn process_instruction(
	program_id: &Address,
	accounts: &mut [AccountView],
	data: &[u8],
) -> ProgramResult {
	let instruction: UpstreamInstruction = parse_instruction(program_id, &ID, data)?;
	match instruction {
		UpstreamInstruction::Hello => {
			HelloAccounts::try_from((program_id, accounts))?.process(data)
		}
	}
}
"#,
		)
		.unwrap_or_else(|error| panic!("write lib.rs: {error}"));
		Project::discover(root).unwrap_or_else(|error| panic!("discover fixture: {error}"))
	}

	/// Write a fake cargo that fabricates the upstream cdylib at the expected
	/// path, and a fake sbpf-linker that answers `--version`.
	#[cfg(unix)]
	fn fake_upstream_binaries(dir: &Path) -> (PathBuf, PathBuf) {
		let cargo = dir.join("fake-cargo.sh");
		fs::write(
			&cargo,
			format!(
				r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${{1:-}}" == "metadata" ]]; then
  exec "$REAL_CARGO" "$@"
fi
mkdir -p "$CARGO_TARGET_DIR/bpfel-unknown-none/release"
printf 'upstream-elf' > "$CARGO_TARGET_DIR/bpfel-unknown-none/release/libupstream_test.so"
"#
			),
		)
		.unwrap_or_else(|error| panic!("write fake cargo: {error}"));
		let mut perms = fs::metadata(&cargo)
			.unwrap_or_else(|error| panic!("stat fake cargo: {error}"))
			.permissions();
		perms.set_mode(0o755);
		fs::set_permissions(&cargo, perms)
			.unwrap_or_else(|error| panic!("chmod fake cargo: {error}"));

		let linker = dir.join("fake-sbpf-linker.sh");
		fs::write(&linker, "#!/usr/bin/env bash\nexit 0\n")
			.unwrap_or_else(|error| panic!("write fake linker: {error}"));
		let mut perms = fs::metadata(&linker)
			.unwrap_or_else(|error| panic!("stat fake linker: {error}"))
			.permissions();
		perms.set_mode(0o755);
		fs::set_permissions(&linker, perms)
			.unwrap_or_else(|error| panic!("chmod fake linker: {error}"));

		(cargo, linker)
	}

	/// Drive `build_project_upstream_with` in-process with fake binaries so
	/// the whole function body — preflight, cargo invocation, retry loop,
	/// artifact discovery, and publication — runs under coverage
	/// instrumentation.
	#[test]
	#[cfg(unix)]
	fn upstream_build_publishes_the_cdylib_and_idl() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let fake_dir = temp.path().join("fake");
		fs::create_dir_all(&fake_dir).unwrap_or_else(|error| panic!("fake dir: {error}"));
		let (cargo, linker) = fake_upstream_binaries(&fake_dir);
		let project_dir = temp.path().join("project");
		let project = upstream_fixture(&project_dir);

		// The fixture's Project::discover already ran with the real cargo; the
		// build itself uses the fake, which fabricates the expected artifact
		// at the discover-time target directory.
		let target_dir = project.target_dir.clone();

		let output = build_project_upstream_with(
			&project,
			&BuildOptions {
				project_dir: project_dir.clone(),
				features: Vec::new(),
				no_default_features: false,
				size_profile: SizeProfile::Production,
			},
			cargo.as_os_str(),
			linker.as_os_str(),
		)
		.unwrap_or_else(|error| panic!("upstream build should succeed: {error}"));

		assert!(
			target_dir.join("deploy/upstream_test.so").is_file(),
			"expected the upstream artifact to be published to target/deploy"
		);
		assert!(
			target_dir.join("idl/upstream_test.json").is_file(),
			"expected the IDL next to the deployed artifact"
		);
		assert!(output.sbf_artifact.ends_with("upstream_test.so"));
	}

	/// The upstream build reports a `MissingSbpfLinker` error when the
	/// configured binary cannot be spawned.
	#[test]
	#[cfg(unix)]
	fn upstream_build_fails_when_the_linker_is_missing() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let project_dir = temp.path().join("project");
		let project = upstream_fixture(&project_dir);

		let missing_linker = temp.path().join("nonexistent-sbpf-linker");
		let error = build_project_upstream_with(
			&project,
			&BuildOptions {
				project_dir,
				features: Vec::new(),
				no_default_features: false,
				size_profile: SizeProfile::Production,
			},
			OsString::from("cargo").as_os_str(),
			missing_linker.as_os_str(),
		)
		.expect_err("missing sbpf-linker should fail the preflight");

		assert!(matches!(error, BuildError::MissingSbpfLinker { .. }));
	}

	/// The upstream build retries with `--export=<symbol>` when cargo reports
	/// the linker's missing-libcall diagnostic, then succeeds.
	#[test]
	#[cfg(unix)]
	fn upstream_build_retries_with_the_reported_export_symbol() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let fake_dir = temp.path().join("fake");
		fs::create_dir_all(&fake_dir).unwrap_or_else(|error| panic!("fake dir: {error}"));
		let (cargo, linker) = fake_upstream_binaries(&fake_dir);
		// Wrap the fake cargo to fail on the first invocation with the
		// sbpf-linker missing-export diagnostic, then delegate.
		let retry_cargo = fake_dir.join("retry-cargo.sh");
		fs::write(
			&retry_cargo,
			format!(
				r#"#!/usr/bin/env bash
if [[ ! -f "{sentinel}" ]]; then
  touch "{sentinel}"
  printf 'error: linking with `sbpf-linker` failed: exit status: 1\n' >&2
  printf '  = note: Error: Symbol __udivti3 is missing. Export it with the `--export=__udivti3` flag.\n' >&2
  exit 1
fi
exec "{cargo}" "$@"
"#,
				sentinel = fake_dir.join("first-attempt.sentinel").display(),
				cargo = cargo.display(),
			),
		)
		.unwrap_or_else(|error| panic!("write retry cargo: {error}"));
		let mut perms = fs::metadata(&retry_cargo)
			.unwrap_or_else(|error| panic!("stat retry cargo: {error}"))
			.permissions();
		perms.set_mode(0o755);
		fs::set_permissions(&retry_cargo, perms)
			.unwrap_or_else(|error| panic!("chmod retry cargo: {error}"));

		let project_dir = temp.path().join("project");
		let project = upstream_fixture(&project_dir);
		let target_dir = project.target_dir.clone();

		let output = build_project_upstream_with(
			&project,
			&BuildOptions {
				project_dir,
				features: Vec::new(),
				no_default_features: false,
				size_profile: SizeProfile::Production,
			},
			retry_cargo.as_os_str(),
			linker.as_os_str(),
		)
		.unwrap_or_else(|error| panic!("upstream retry should succeed: {error}"));

		assert!(
			target_dir.join("deploy/upstream_test.so").is_file(),
			"expected the artifact after the retry"
		);
	}

	/// The upstream build surfaces a `MissingArtifact` error when cargo
	/// succeeds but the cdylib is not at the expected path.
	#[test]
	#[cfg(unix)]
	fn upstream_build_reports_a_missing_artifact() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let fake_dir = temp.path().join("fake");
		fs::create_dir_all(&fake_dir).unwrap_or_else(|error| panic!("fake dir: {error}"));
		// A no-op cargo that exits 0 without producing anything.
		let no_op_cargo = fake_dir.join("no-op-cargo.sh");
		fs::write(&no_op_cargo, "#!/usr/bin/env bash\nexit 0\n")
			.unwrap_or_else(|error| panic!("write no-op cargo: {error}"));
		let mut perms = fs::metadata(&no_op_cargo)
			.unwrap_or_else(|error| panic!("stat no-op cargo: {error}"))
			.permissions();
		perms.set_mode(0o755);
		fs::set_permissions(&no_op_cargo, perms)
			.unwrap_or_else(|error| panic!("chmod no-op cargo: {error}"));

		let linker = fake_dir.join("fake-sbpf-linker.sh");
		fs::write(&linker, "#!/usr/bin/env bash\nexit 0\n")
			.unwrap_or_else(|error| panic!("write fake linker: {error}"));
		let mut perms = fs::metadata(&linker)
			.unwrap_or_else(|error| panic!("stat fake linker: {error}"))
			.permissions();
		perms.set_mode(0o755);
		fs::set_permissions(&linker, perms)
			.unwrap_or_else(|error| panic!("chmod fake linker: {error}"));

		let project_dir = temp.path().join("project");
		let project = upstream_fixture(&project_dir);
		let target_dir = project.target_dir.clone();

		let error = build_project_upstream_with(
			&project,
			&BuildOptions {
				project_dir,
				features: Vec::new(),
				no_default_features: false,
				size_profile: SizeProfile::Production,
			},
			no_op_cargo.as_os_str(),
			linker.as_os_str(),
		)
		.expect_err("no-op cargo should leave the artifact missing");

		assert!(matches!(error, BuildError::MissingArtifact { .. }));
	}

	// The routing from `build_with_measurements` through the env-reading
	// wrapper (`build_project_upstream` → `sbpf_linker_command`) is exercised
	// end-to-end by `upstream_build_drives_the_upstream_recipe` in
	// `tests/upstream_build_command.rs`, which spawns the CLI binary with the
	// CARGO and PINA_SBPF_LINKER environment variables set on the child — the
	// only safe way to cover env-var readers, since Rust marks `set_var`
	// unsafe for multithreaded processes.

	/// The upstream build surfaces a `CargoFailed` error when cargo exits
	/// nonzero without a missing-export diagnostic to retry with.
	#[test]
	#[cfg(unix)]
	fn upstream_build_fails_on_a_non_retryable_cargo_error() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let fake_dir = temp.path().join("fake");
		fs::create_dir_all(&fake_dir).unwrap_or_else(|error| panic!("fake dir: {error}"));
		let failing_cargo = fake_dir.join("failing-cargo.sh");
		fs::write(
			&failing_cargo,
			"#!/usr/bin/env bash\nprintf 'error: unresolved dependency\\n' >&2\nexit 1\n",
		)
		.unwrap_or_else(|error| panic!("write failing cargo: {error}"));
		let mut perms = fs::metadata(&failing_cargo)
			.unwrap_or_else(|error| panic!("stat failing cargo: {error}"))
			.permissions();
		perms.set_mode(0o755);
		fs::set_permissions(&failing_cargo, perms)
			.unwrap_or_else(|error| panic!("chmod failing cargo: {error}"));

		let linker = fake_dir.join("fake-sbpf-linker.sh");
		fs::write(&linker, "#!/usr/bin/env bash\nexit 0\n")
			.unwrap_or_else(|error| panic!("write fake linker: {error}"));
		let mut perms = fs::metadata(&linker)
			.unwrap_or_else(|error| panic!("stat fake linker: {error}"))
			.permissions();
		perms.set_mode(0o755);
		fs::set_permissions(&linker, perms)
			.unwrap_or_else(|error| panic!("chmod fake linker: {error}"));

		let project_dir = temp.path().join("project");
		let project = upstream_fixture(&project_dir);
		let target_dir = project.target_dir.clone();

		let error = build_project_upstream_with(
			&project,
			&BuildOptions {
				project_dir,
				features: Vec::new(),
				no_default_features: false,
				size_profile: SizeProfile::Production,
			},
			failing_cargo.as_os_str(),
			linker.as_os_str(),
		)
		.expect_err("a non-retryable cargo error should surface");

		assert!(matches!(error, BuildError::CargoFailed { .. }));
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
		build_sbf_args(
			project,
			&manifest_path,
			"bpf-entrypoint",
			&options,
			&project.target_dir.join("sbf-build"),
		)
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

	#[test]
	fn line_tables_request_dwarf_and_an_unstripped_linker_output() {
		let mut command = Command::new("cargo");
		apply_line_tables(&mut command);
		let env: Vec<(String, Option<String>)> = command
			.get_envs()
			.map(|(key, value)| {
				(
					key.to_string_lossy().into_owned(),
					value.map(|value| value.to_string_lossy().into_owned()),
				)
			})
			.collect();

		assert_eq!(
			env,
			[
				(
					"CARGO_PROFILE_RELEASE_DEBUG".to_owned(),
					Some("line-tables-only".to_owned())
				),
				(
					"CARGO_PROFILE_RELEASE_STRIP".to_owned(),
					Some("none".to_owned())
				),
			]
		);
	}

	#[test]
	fn unstripped_artifact_finds_the_linker_output() {
		let (temp, mut project) =
			discover_fixture_with_crate_types("unstripped-fixture", "\"cdylib\"");
		// An ambient CARGO_TARGET_DIR (such as llvm-cov's) must not be touched.
		project.target_dir = temp.path().join("target");
		assert_eq!(unstripped_artifact(&project), None);

		let release = project.target_dir.join("sbpf-solana-solana/release");
		fs::create_dir_all(&release).unwrap_or_else(|error| panic!("create release: {error}"));
		let artifact = release.join(format!("{}.so", project.library_name));
		fs::write(&artifact, b"elf").unwrap_or_else(|error| panic!("write artifact: {error}"));

		assert_eq!(unstripped_artifact(&project), Some(artifact));
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
				opt_level: None,
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
					opt_level: Some(3),
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

		// A manifest that keeps overflow checks on agrees with the ordinary
		// build, which honours the manifest over the profile's override.
		assert_eq!(
			verified_profile_divergence(
				DeclaredReleaseProfile {
					lto: Some(true),
					codegen_units: Some(1),
					opt_level: Some(3),
					overflow_checks: Some(true),
				},
				SizeProfile::Production,
			),
			None
		);

		// The `--overflow-checks` flag turns checks on for the ordinary build
		// even under a silent manifest, so the verified build diverges.
		assert!(
			verified_profile_divergence(
				DeclaredReleaseProfile::default(),
				SizeProfile::ProductionWithOverflowChecks
			)
			.is_some()
		);

		// `--no-lto` turns LTO off for the ordinary build even when the
		// manifest enables it, so the verified build diverges.
		assert!(
			verified_profile_divergence(
				DeclaredReleaseProfile {
					lto: Some(true),
					codegen_units: Some(1),
					opt_level: Some(3),
					overflow_checks: Some(false),
				},
				SizeProfile::ProductionWithoutLto,
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
	fn a_non_default_opt_level_diverges_from_the_profile() {
		// The profile compiles at opt-level 3; a manifest pinned to `2` builds
		// the verified artifact with different codegen.
		let (_temp, project) = discover_workspace_fixture(
			"opt-level-fixture",
			"[profile.release]\nlto = \"fat\"\ncodegen-units = 1\nopt-level = 2\noverflow-checks \
			 = false\n",
		);

		assert_eq!(
			verified_profile_divergence(
				declared_release_profile(&project),
				SizeProfile::Production
			)
			.is_some(),
			true
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
