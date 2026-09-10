//! Project-aware execution of Pina's official security lints.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

use crate::lint_driver::DriverError;
use crate::lint_driver::driver_build_identity;
use crate::lint_driver::driver_library_environment;
use crate::lint_driver::format_lint_levels;
use crate::lint_driver::prepare_driver;
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
	/// Cargo package checked by the lint driver.
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
	Driver(#[from] DriverError),

	#[error("Could not run Pina's security lints: {source}")]
	RunCargo { source: std::io::Error },

	#[error("Pina's security lints failed with status {status}")]
	LintFailed { status: String },
}

/// Discover a Pina project and run this CLI release's official lint set.
///
/// The lints are compiled into the `pina_lints` crate and statically linked
/// into the `pina_lint_driver` binary, which ships prebuilt next to this CLI
/// (see [`crate::lint_driver`]). This command resolves the driver, then runs
/// `cargo check` — or `cargo fix` with `--fix` — with the driver as
/// `RUSTC_WORKSPACE_WRAPPER`. Level overrides from the project's `pina.toml`
/// `[lints]` table are forwarded through `PINA_LINT_LEVELS`.
///
/// # Errors
///
/// Returns an error when project discovery fails, the prebuilt lint driver
/// cannot be resolved or loaded, or cargo reports a lint or compilation
/// failure.
pub fn lint_project(options: &LintOptions) -> Result<LintOutput, LintError> {
	let project = Project::discover(&options.project)?;
	let driver = prepare_driver(&project.root)?;
	let driver_build =
		driver_build_identity(&driver.path).map_err(|source| LintError::RunCargo { source })?;
	let manifest = project.program_dir.join("Cargo.toml");

	let levels = project
		.lint_levels
		.iter()
		.map(|(name, level)| (name.as_str(), level.as_str()))
		.collect::<Vec<_>>();

	// Some environments (devenv's rust integration) export `CARGO` as an
	// empty string; treat that like an unset variable rather than spawning a
	// nameless executable.
	let cargo = std::env::var_os("CARGO")
		.filter(|cargo| !cargo.is_empty())
		.unwrap_or_else(|| OsString::from("cargo"));
	let mut command = Command::new(&cargo);
	command
		.current_dir(&project.root)
		.env("RUSTC_WORKSPACE_WRAPPER", &driver.path)
		.env("PINA_LINT_DRIVER_BUILD", driver_build)
		.env("PINA_LINT_NO_DEPS", "1");
	// The driver loads `librustc_driver` through the rpath baked in at build
	// time; cargo and every wrapper it spawns also need the sysroot's library
	// directory on the search path for toolchains whose rpath went stale.
	for (name, value) in driver_library_environment(&driver.sysroot) {
		command.env(name, value);
	}
	if !levels.is_empty() {
		command.env("PINA_LINT_LEVELS", format_lint_levels(levels));
	}
	if options.fix {
		command
			.arg("fix")
			.arg("--allow-dirty")
			.arg("--allow-staged")
			.arg("--allow-no-vcs");
	} else {
		command.arg("check");
	}
	command
		.arg("--locked")
		.arg("--manifest-path")
		.arg(manifest)
		.arg("--package")
		.arg(&project.package_name);

	let status = command
		.status()
		.map_err(|source| LintError::RunCargo { source })?;

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
