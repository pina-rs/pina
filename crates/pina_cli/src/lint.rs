//! Project-aware execution of Pina's official security lints.

use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use crate::lint_bundle::BundleError;
use crate::lint_bundle::prepare_bundle;
use crate::lint_bundle::prepare_dylint_tools;
use crate::project::Project;
use crate::project::ProjectError;

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

	#[error(transparent)]
	Bundle(#[from] BundleError),

	#[error("Could not resolve Cargo home for Pina's managed Dylint tools")]
	MissingCargoHome,

	#[error("Could not resolve relative CARGO_HOME from the current directory: {source}")]
	CurrentDirectory { source: std::io::Error },

	#[error("Could not construct PATH for the pinned Dylint tools: {source}")]
	ToolPath { source: std::env::JoinPathsError },

	#[error("Could not run Pina's security lints: {source}")]
	RunDylint { source: std::io::Error },

	#[error("Pina's security lints failed with status {status}")]
	LintFailed { status: String },
}

/// Discover a Pina project and run this CLI release's official lint set.
///
/// The first invocation downloads pinned `cargo-dylint` and the precompiled
/// libraries below Cargo home. Later invocations and projects reuse the
/// verified caches. Exact library paths are passed to Dylint, so
/// project-provided Dylint metadata is ignored.
///
/// # Errors
///
/// Returns an error when project discovery fails, the pinned tools cannot be
/// prepared, or Dylint reports a lint or compilation failure.
pub fn lint_project(options: &LintOptions) -> Result<LintOutput, LintError> {
	let project = Project::discover(&options.project)?;
	let cargo_home = cargo_home()?;
	let tools = prepare_dylint_tools(&cargo_home)?;
	let bundle = prepare_bundle(&cargo_home)?;
	let manifest = project.program_dir.join("Cargo.toml");
	let mut args = vec![
		OsString::from("dylint"),
		OsString::from("--no-deps"),
		OsString::from("--no-metadata"),
		OsString::from("--manifest-path"),
		manifest.into_os_string(),
		OsString::from("--package"),
		OsString::from(&project.package_name),
	];
	for library_path in bundle.library_paths {
		args.push(OsString::from("--lib-path"));
		args.push(library_path.into_os_string());
	}

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

/// Prepend Pina's managed tool directory without discarding the caller's PATH.
fn tool_path(bin_dir: &Path) -> Result<OsString, LintError> {
	let current_path = std::env::var_os("PATH").unwrap_or_default();
	let paths = std::iter::once(bin_dir.to_path_buf()).chain(std::env::split_paths(&current_path));
	std::env::join_paths(paths).map_err(|source| LintError::ToolPath { source })
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
