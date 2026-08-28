//! Project-aware execution of Pina's official security lints.

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use fs2::FileExt;

use crate::project::Project;
use crate::project::ProjectError;

/// The `cargo-dylint` release used by `pina lint`.
pub const CARGO_DYLINT_VERSION: &str = "6.0.4";

/// The `dylint-link` release used to build Pina's lint libraries.
pub const DYLINT_LINK_VERSION: &str = "6.0.4";

const PINA_REPOSITORY: &str = "https://github.com/pina-rs/pina";
const PINA_LINT_PATTERN: &str = "lints/*";

/// Options for running Pina's official security lints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintOptions {
	/// Directory inside the project to discover.
	pub project: PathBuf,

	/// Apply machine-applicable suggestions emitted by the lint set.
	pub fix: bool,
}

/// The project linted by [`lint_project`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintOutput {
	/// Cargo package checked by Dylint.
	pub package_name: String,

	/// Whether automatic fixes were requested.
	pub fix: bool,
}

/// Errors produced while preparing or running Pina's security lints.
#[derive(Debug, thiserror::Error)]
pub enum LintError {
	#[error(transparent)]
	Project(#[from] ProjectError),

	#[error("Could not create the managed Dylint directory at {path}: {source}")]
	CreateToolDirectory {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Could not open the managed Dylint installation lock at {path}: {source}")]
	OpenInstallLock {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Could not lock the managed Dylint installation at {path}: {source}")]
	LockInstall {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Could not install pinned tool `{package}` with Cargo: {source}")]
	RunInstall {
		package: &'static str,
		source: std::io::Error,
	},

	#[error("Installing pinned tool `{package}` failed with status {status}")]
	InstallFailed {
		package: &'static str,
		status: String,
	},

	#[error("Cargo reported success but did not install `{package}` at {path}")]
	MissingInstalledTool {
		package: &'static str,
		path: PathBuf,
	},

	#[error("Could not construct PATH for the pinned Dylint tools: {source}")]
	ToolPath { source: std::env::JoinPathsError },

	#[error("Could not run Pina's security lints: {source}")]
	RunDylint { source: std::io::Error },

	#[error("Pina's security lints failed with status {status}")]
	LintFailed { status: String },
}

/// Discover a Pina project and run the release-matched official lint set.
///
/// The first invocation installs pinned `cargo-dylint` and `dylint-link`
/// binaries below Cargo's target directory. Later invocations reuse that
/// project-local tool installation. Lint libraries are loaded from the Pina
/// release tag matching this CLI, not from a mutable branch or project-provided
/// Dylint metadata.
///
/// # Errors
///
/// Returns an error when project discovery fails, the pinned tools cannot be
/// prepared, or Dylint reports a lint or compilation failure.
pub fn lint_project(options: &LintOptions) -> Result<LintOutput, LintError> {
	let project = Project::discover(&options.project)?;
	let tools = prepare_tools(&project)?;
	let manifest = project.program_dir.join("Cargo.toml");
	let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
	let mut args = vec![
		OsString::from("dylint"),
		OsString::from("--no-deps"),
		OsString::from("--git"),
		OsString::from(PINA_REPOSITORY),
		OsString::from("--tag"),
		OsString::from(tag),
		OsString::from("--pattern"),
		OsString::from(PINA_LINT_PATTERN),
		OsString::from("--manifest-path"),
		manifest.into_os_string(),
		OsString::from("--package"),
		OsString::from(&project.package_name),
	];

	if options.fix {
		args.push(OsString::from("--fix"));
		args.push(OsString::from("--"));
		args.push(OsString::from("--allow-dirty"));
		args.push(OsString::from("--allow-staged"));
		args.push(OsString::from("--allow-no-vcs"));
	}

	let path = tool_path(&tools.bin_dir)?;
	let status = Command::new(&tools.cargo_dylint)
		.args(&args)
		.current_dir(&project.root)
		.env("CARGO_INCREMENTAL", "0")
		.env("PATH", path)
		.status()
		.map_err(|source| LintError::RunDylint { source })?;

	if !status.success() {
		return Err(LintError::LintFailed {
			status: status.to_string(),
		});
	}

	Ok(LintOutput {
		package_name: project.package_name,
		fix: options.fix,
	})
}

#[derive(Debug)]
struct Tools {
	bin_dir: PathBuf,
	cargo_dylint: PathBuf,
}

fn prepare_tools(project: &Project) -> Result<Tools, LintError> {
	let root = project.target_dir.join("pina/tools").join(format!(
		"dylint-{CARGO_DYLINT_VERSION}-{DYLINT_LINK_VERSION}"
	));
	std::fs::create_dir_all(&root).map_err(|source| {
		LintError::CreateToolDirectory {
			path: root.clone(),
			source,
		}
	})?;

	let lock_path = root.join("install.lock");
	let lock = OpenOptions::new()
		.read(true)
		.write(true)
		.create(true)
		.truncate(false)
		.open(&lock_path)
		.map_err(|source| {
			LintError::OpenInstallLock {
				path: lock_path.clone(),
				source,
			}
		})?;
	lock.try_lock_exclusive().map_err(|source| {
		LintError::LockInstall {
			path: lock_path,
			source,
		}
	})?;

	let bin_dir = root.join("bin");
	let cargo_dylint = bin_dir.join(executable_name("cargo-dylint"));
	let dylint_link = bin_dir.join(executable_name("dylint-link"));
	let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));

	install_tool(
		&cargo,
		&project.root,
		&root,
		"cargo-dylint",
		CARGO_DYLINT_VERSION,
		&cargo_dylint,
	)?;
	install_tool(
		&cargo,
		&project.root,
		&root,
		"dylint-link",
		DYLINT_LINK_VERSION,
		&dylint_link,
	)?;

	Ok(Tools {
		bin_dir,
		cargo_dylint,
	})
}

fn install_tool(
	cargo: &OsString,
	project_root: &Path,
	tool_root: &Path,
	package: &'static str,
	version: &'static str,
	executable: &Path,
) -> Result<(), LintError> {
	if executable.is_file() {
		return Ok(());
	}

	let build_target = tool_root.join("build").join(package);
	let status = Command::new(cargo)
		.args([
			OsString::from("install"),
			OsString::from("--locked"),
			OsString::from("--root"),
			tool_root.as_os_str().to_owned(),
			OsString::from("--version"),
			OsString::from(version),
			OsString::from(package),
		])
		.current_dir(project_root)
		.env("CARGO_TARGET_DIR", build_target)
		.status()
		.map_err(|source| LintError::RunInstall { package, source })?;

	if !status.success() {
		return Err(LintError::InstallFailed {
			package,
			status: status.to_string(),
		});
	}

	if !executable.is_file() {
		return Err(LintError::MissingInstalledTool {
			package,
			path: executable.to_path_buf(),
		});
	}

	Ok(())
}

fn tool_path(bin_dir: &Path) -> Result<OsString, LintError> {
	let current_path = std::env::var_os("PATH").unwrap_or_default();
	let paths = std::iter::once(bin_dir.to_path_buf()).chain(std::env::split_paths(&current_path));
	std::env::join_paths(paths).map_err(|source| LintError::ToolPath { source })
}

#[cfg(windows)]
fn executable_name(name: &str) -> OsString {
	OsString::from(format!("{name}.exe"))
}

#[cfg(not(windows))]
fn executable_name(name: &str) -> OsString {
	OsString::from(name)
}
