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

/// Immutable Git revision containing Pina's reviewed official lint set.
///
/// Update this only after reviewing changes below `lints/`. Unlike a release
/// tag, a full commit ID cannot be redirected to different native lint code.
pub const PINA_LINT_REVISION: &str = "7aff2afb89cd34a13e1e9d6ff854bc16d12263cc";

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

	#[error("Could not resolve Cargo home for Pina's managed Dylint tools")]
	MissingCargoHome,

	#[error("Could not resolve relative CARGO_HOME from the current directory: {source}")]
	CurrentDirectory { source: std::io::Error },

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

/// Discover a Pina project and run Pina's revision-pinned official lint set.
///
/// The first invocation installs pinned `cargo-dylint` and `dylint-link`
/// binaries below Cargo home. Later invocations and projects reuse that managed
/// installation. Lint libraries are loaded from the immutable revision reviewed
/// for this CLI, not from a mutable branch, tag, or project-provided Dylint
/// metadata.
///
/// # Errors
///
/// Returns an error when project discovery fails, the pinned tools cannot be
/// prepared, or Dylint reports a lint or compilation failure.
pub fn lint_project(options: &LintOptions) -> Result<LintOutput, LintError> {
	let project = Project::discover(&options.project)?;
	let tools = prepare_tools(&project)?;
	let manifest = project.program_dir.join("Cargo.toml");
	let mut args = vec![
		OsString::from("dylint"),
		OsString::from("--no-deps"),
		OsString::from("--git"),
		OsString::from(PINA_REPOSITORY),
		OsString::from("--rev"),
		OsString::from(PINA_LINT_REVISION),
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

/// Executables installed and selected by Pina rather than project metadata.
#[derive(Debug)]
struct Tools {
	bin_dir: PathBuf,
	cargo_dylint: PathBuf,
}

/// Prepare the versioned Dylint tools in a user-owned Cargo-home cache.
fn prepare_tools(project: &Project) -> Result<Tools, LintError> {
	let root = cargo_home()?.join("pina/tools").join(format!(
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

/// Resolve Cargo home using Cargo's environment and platform conventions.
fn cargo_home() -> Result<PathBuf, LintError> {
	resolve_cargo_home(
		std::env::var_os("CARGO_HOME"),
		std::env::current_dir(),
		platform_home(),
	)
}

/// Resolve an explicit or platform Cargo home without reading process state.
fn resolve_cargo_home(
	cargo_home: Option<OsString>,
	current_dir: Result<PathBuf, std::io::Error>,
	platform_home: Option<PathBuf>,
) -> Result<PathBuf, LintError> {
	if let Some(home) = cargo_home {
		let home = PathBuf::from(home);
		if home.is_absolute() {
			return Ok(home);
		}

		return current_dir
			.map(|current_dir| current_dir.join(home))
			.map_err(|source| LintError::CurrentDirectory { source });
	}

	platform_home
		.map(|home| home.join(".cargo"))
		.ok_or(LintError::MissingCargoHome)
}

#[cfg(not(windows))]
/// Return the platform home used by Cargo on Unix-like systems.
fn platform_home() -> Option<PathBuf> {
	std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(windows)]
/// Return the platform home used by Cargo on Windows.
fn platform_home() -> Option<PathBuf> {
	std::env::var_os("USERPROFILE")
		.map(PathBuf::from)
		.or_else(|| {
			let drive = std::env::var_os("HOMEDRIVE")?;
			let path = std::env::var_os("HOMEPATH")?;
			Some(PathBuf::from(drive).join(path))
		})
}

/// Install one exact crates.io tool release unless its managed binary exists.
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

/// Prepend Pina's managed tool directory without discarding the caller's PATH.
fn tool_path(bin_dir: &Path) -> Result<OsString, LintError> {
	let current_path = std::env::var_os("PATH").unwrap_or_default();
	let paths = std::iter::once(bin_dir.to_path_buf()).chain(std::env::split_paths(&current_path));
	std::env::join_paths(paths).map_err(|source| LintError::ToolPath { source })
}

#[cfg(windows)]
/// Return a platform executable filename on Windows.
fn executable_name(name: &str) -> OsString {
	OsString::from(format!("{name}.exe"))
}

#[cfg(not(windows))]
/// Return a platform executable filename on Unix-like systems.
fn executable_name(name: &str) -> OsString {
	OsString::from(name)
}

#[cfg(test)]
mod tests {
	use std::io;

	use super::*;

	#[test]
	fn resolves_absolute_relative_and_default_cargo_homes() {
		let absolute = std::env::temp_dir().join("managed-cargo");
		assert_eq!(
			resolve_cargo_home(
				Some(absolute.clone().into_os_string()),
				Err(io::Error::other("unused")),
				None,
			)
			.expect("absolute Cargo home should resolve"),
			absolute
		);

		let working = std::env::temp_dir().join("working");
		assert_eq!(
			resolve_cargo_home(Some(OsString::from("relative")), Ok(working.clone()), None,)
				.expect("relative Cargo home should resolve"),
			working.join("relative")
		);

		let home = std::env::temp_dir().join("home-developer");
		assert_eq!(
			resolve_cargo_home(None, Err(io::Error::other("unused")), Some(home.clone()),)
				.expect("default Cargo home should resolve"),
			home.join(".cargo")
		);
	}

	#[test]
	fn reports_unavailable_working_and_home_directories() {
		let current_directory = resolve_cargo_home(
			Some(OsString::from("relative")),
			Err(io::Error::other("missing working directory")),
			None,
		)
		.expect_err("relative Cargo home should require a working directory");
		assert!(matches!(
			current_directory,
			LintError::CurrentDirectory { .. }
		));

		let home = resolve_cargo_home(None, Ok(PathBuf::from("/working")), None)
			.expect_err("a platform home should be required");
		assert!(matches!(home, LintError::MissingCargoHome));
	}
}
