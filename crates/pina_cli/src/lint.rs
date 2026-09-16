//! Project-aware execution of Pina's official security lints.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

use crate::lint_driver::DriverError;
use crate::lint_driver::DriverOptions;
use crate::lint_driver::DriverOrigin;
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

	/// Build the driver from source with the active toolchain instead of
	/// accepting a cached, bundled, or downloaded driver.
	pub build_driver: bool,
}

/// The project linted by [`lint_project`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintOutput {
	/// Cargo package checked by the lint driver.
	pub package_name: String,

	/// Whether automatic fixes were requested.
	pub fix: bool,

	/// How the lint driver was obtained.
	pub driver_origin: DriverOrigin,
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
/// into the `pina_lint_driver` binary. The CLI resolves a driver built for the
/// project's *active* toolchain (see [`crate::lint_driver`]), then runs `cargo
/// check` — or `cargo fix` with `--fix` — with the driver as
/// `RUSTC_WORKSPACE_WRAPPER`. Level overrides from the project's `pina.toml`
/// `[lints]` table are forwarded through `PINA_LINT_LEVELS`.
///
/// # Errors
///
/// Returns an error when project discovery fails, no lint driver can be
/// obtained for the active toolchain, or cargo reports a lint or compilation
/// failure.
pub fn lint_project(options: &LintOptions) -> Result<LintOutput, LintError> {
	let project = Project::discover(&options.project)?;
	let driver = prepare_driver(
		&project.root,
		DriverOptions {
			build_driver: options.build_driver,
			// A lint run may fetch the driver published for this CLI release;
			// only `pina doctor` reports state without changing it.
			allow_download: true,
		},
	)?;
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
		driver_origin: driver.origin,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn lint_options_record_the_requested_driver_build() {
		let options = LintOptions {
			project: PathBuf::from("."),
			fix: false,
			build_driver: true,
		};

		assert!(options.build_driver);
		assert!(!options.fix);
	}

	#[test]
	fn lint_output_carries_the_driver_origin() {
		let output = LintOutput {
			package_name: "counter".to_owned(),
			fix: true,
			driver_origin: DriverOrigin::Cache,
		};

		assert_eq!(output.package_name, "counter");
		assert!(output.fix);
		assert_eq!(output.driver_origin.as_str(), "cached");
	}
}
