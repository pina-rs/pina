//! Preparation of the `pina_lint_driver` binary used by `pina lint`.
//!
//! The driver links against the Rust compiler's unstable `rustc_private`
//! crates, so it must be built with the exact toolchain that compiles the
//! project. The CLI installs it from the crates.io release of `pina_lints`
//! matching its own version and caches it below Cargo home, keyed by the
//! toolchain fingerprint. The `PINA_LINT_DRIVER_PATH` environment variable
//! bypasses the cache and points at an already-built driver; the repository's
//! own tasks and tests use it to run the workspace driver.

use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

/// Environment variable pointing at an existing driver binary.
const PINA_LINT_DRIVER_PATH: &str = "PINA_LINT_DRIVER_PATH";

/// The prepared driver and how it was resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedDriver {
	/// Path of the driver executable.
	pub path: PathBuf,
}

/// Errors produced while preparing the lint driver.
#[derive(Debug, thiserror::Error)]
pub enum DriverError {
	#[error("Could not resolve Cargo home for Pina's managed lint driver")]
	MissingCargoHome,

	#[error("Could not resolve relative CARGO_HOME from the current directory: {source}")]
	CurrentDirectory { source: std::io::Error },

	#[error("Could not query the Rust compiler fingerprint: {source}")]
	QueryRustc { source: std::io::Error },

	#[error(
		"Could not query the Rust compiler fingerprint because rustc exited with status {status}"
	)]
	RustcFailed { status: String },

	#[error("Could not parse a release or host target from rustc -vV")]
	MissingRustcFingerprint,

	#[error("`PINA_LINT_DRIVER_PATH` does not point at an executable: {path}")]
	InvalidDriverOverride { path: PathBuf },

	#[error("Could not run cargo to install the lint driver: {source}")]
	RunCargo { source: std::io::Error },

	#[error("Could not install the lint driver; cargo exited with status {status}")]
	InstallFailed { status: String },

	#[error("The lint driver install finished without producing {path}")]
	MissingDriver { path: PathBuf },
}

/// Resolve the driver for the given project root.
///
/// The driver is installed below `cargo_home` at
/// `pina/lint-driver/<pina-version>/<toolchain>/bin/pina_lint_driver`, where
/// the toolchain component is the `release-host` fingerprint reported by the
/// Rust compiler that builds the project.
pub fn prepare_driver(
	cargo_home: &Path,
	project_root: &Path,
) -> Result<PreparedDriver, DriverError> {
	if let Some(path) = std::env::var_os(PINA_LINT_DRIVER_PATH) {
		let path = PathBuf::from(path);
		if is_executable(&path) {
			return Ok(PreparedDriver { path });
		}
		return Err(DriverError::InvalidDriverOverride { path });
	}

	let toolchain = rustc_fingerprint(project_root)?;
	let root = cargo_home
		.join("pina")
		.join("lint-driver")
		.join(env!("CARGO_PKG_VERSION"))
		.join(&toolchain);
	let bin = root.join("bin").join(driver_binary_name());

	if is_executable(&bin) {
		return Ok(PreparedDriver { path: bin });
	}

	install_driver(&root)?;

	if is_executable(&bin) {
		return Ok(PreparedDriver { path: bin });
	}
	Err(DriverError::MissingDriver { path: bin })
}

/// Install the driver from the crates.io release matching this CLI.
fn install_driver(root: &Path) -> Result<(), DriverError> {
	let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));

	let status = Command::new(&cargo)
		.arg("install")
		.arg("--locked")
		.arg("--root")
		.arg(root)
		.arg("--bin")
		.arg("pina_lint_driver")
		.arg("--version")
		.arg(concat!("=", env!("CARGO_PKG_VERSION")))
		.arg("pina_lints")
		.status()
		.map_err(|source| DriverError::RunCargo { source })?;

	if !status.success() {
		// Remove the partial installation so a later run retries cleanly.
		let _ = std::fs::remove_dir_all(root);
		return Err(DriverError::InstallFailed {
			status: status.to_string(),
		});
	}
	Ok(())
}

/// Return the platform file name of the driver binary.
#[cfg(windows)]
fn driver_binary_name() -> &'static str {
	"pina_lint_driver.exe"
}

/// Return the platform file name of the driver binary.
#[cfg(not(windows))]
fn driver_binary_name() -> &'static str {
	"pina_lint_driver"
}

/// Return whether `path` is an executable file.
#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
	use std::os::unix::fs::PermissionsExt;

	path.is_file()
		&& std::fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

/// Return whether `path` is an executable file.
#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
	path.is_file()
}

/// Return the `release-host` fingerprint of the Rust compiler used for
/// `project_root`.
fn rustc_fingerprint(project_root: &Path) -> Result<String, DriverError> {
	let output = Command::new("rustc")
		.arg("-vV")
		.current_dir(project_root)
		.output()
		.map_err(|source| DriverError::QueryRustc { source })?;

	if !output.status.success() {
		return Err(DriverError::RustcFailed {
			status: output.status.to_string(),
		});
	}

	parse_rustc_fingerprint(&output.stdout).ok_or(DriverError::MissingRustcFingerprint)
}

/// Parse the release and host from verbose rustc version output.
fn parse_rustc_fingerprint(output: &[u8]) -> Option<String> {
	let release = String::from_utf8_lossy(output)
		.lines()
		.find_map(|line| line.strip_prefix("release: "))?
		.split_whitespace()
		.next()?
		.to_owned();
	let host = String::from_utf8_lossy(output)
		.lines()
		.find_map(|line| line.strip_prefix("host: "))?
		.split_whitespace()
		.next()?
		.to_owned();

	let fingerprint = format!("{release}-{host}");
	// The release and host prefixes were parsed above, so the leftover
	// replacements cannot blank the name out.
	Some(
		fingerprint
			.chars()
			.map(|character| {
				if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
					character
				} else {
					'-'
				}
			})
			.collect::<String>(),
	)
}

/// Resolve the directory Cargo uses for its managed state.
pub fn resolve_cargo_home(
	cargo_home: Option<OsString>,
	current_dir: Result<PathBuf, std::io::Error>,
	platform_home: Option<PathBuf>,
) -> Result<PathBuf, DriverError> {
	if let Some(home) = cargo_home {
		let home = PathBuf::from(home);
		if home.is_absolute() {
			return Ok(home);
		}

		return current_dir
			.map(|current_dir| current_dir.join(home))
			.map_err(|source| DriverError::CurrentDirectory { source });
	}

	platform_home
		.map(|home| home.join(".cargo"))
		.ok_or(DriverError::MissingCargoHome)
}

#[cfg(not(windows))]
fn platform_home() -> Option<PathBuf> {
	std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(windows)]
fn platform_home() -> Option<PathBuf> {
	std::env::var_os("USERPROFILE")
		.map(PathBuf::from)
		.or_else(|| {
			let drive = std::env::var_os("HOMEDRIVE")?;
			let path = std::env::var_os("HOMEPATH")?;
			Some(PathBuf::from(drive).join(path))
		})
}

/// Resolve Cargo home using Cargo's environment and platform conventions.
pub fn cargo_home() -> Result<PathBuf, DriverError> {
	resolve_cargo_home(
		std::env::var_os("CARGO_HOME"),
		std::env::current_dir(),
		platform_home(),
	)
}

/// Format the configured lint levels for the driver's `PINA_LINT_LEVELS`
/// variable.
pub fn format_lint_levels<'a, I>(levels: I) -> String
where
	I: IntoIterator<Item = (&'a str, &'a str)>,
{
	levels
		.into_iter()
		.map(|(name, level)| format!("{name}={level}"))
		.collect::<Vec<_>>()
		.join(",")
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
			resolve_cargo_home(None, Err(io::Error::other("unused")), Some(home.clone()))
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
			DriverError::CurrentDirectory { .. }
		));

		let home = resolve_cargo_home(None, Ok(PathBuf::from("/working")), None)
			.expect_err("a platform home should be required");
		assert!(matches!(home, DriverError::MissingCargoHome));
	}

	#[test]
	fn parses_rustc_fingerprint_from_verbose_version_output() {
		let output = b"rustc 1.95.0-nightly (abc 2026-02-20)\nbinary: rustc\nrelease: \
		               1.95.0-nightly\nhost: x86_64-unknown-linux-gnu\n";

		assert_eq!(
			parse_rustc_fingerprint(output).as_deref(),
			Some("1.95.0-nightly-x86_64-unknown-linux-gnu")
		);
		assert!(parse_rustc_fingerprint(b"rustc without fingerprint lines\n").is_none());
	}

	#[test]
	fn sanitizes_unexpected_characters_out_of_the_toolchain_fingerprint() {
		let output = b"rustc 1.95.0 nightly\nbinary: rustc\nrelease: \
		               1.95.0~rolling\nhost: aarch64 unknown linux\n";

		// The tilde is not allowed in a cargo target fingerprint, so the
		// sanitizer must replace it with the safe placeholder. The host token
		// is the first whitespace-separated word of the host line.
		let parsed = parse_rustc_fingerprint(output).expect("the crafted output should parse");
		assert_eq!(parsed, "1.95.0-rolling-aarch64");
	}

	#[test]
	fn formats_lint_levels_for_the_driver_environment() {
		assert_eq!(
			format_lint_levels([("require_empty_before_init", "deny"), ("other", "allow")]),
			"require_empty_before_init=deny,other=allow"
		);
		assert_eq!(format_lint_levels(std::iter::empty::<(&str, &str)>()), "");
	}

	#[test]
	#[cfg(unix)]
	fn executable_check_requires_the_executable_bit() {
		use std::os::unix::fs::PermissionsExt;

		let file = tempfile::NamedTempFile::new().expect("temp file");
		std::fs::set_permissions(file.path(), std::fs::Permissions::from_mode(0o755))
			.expect("permissions");
		assert!(is_executable(file.path()));

		std::fs::set_permissions(file.path(), std::fs::Permissions::from_mode(0o644))
			.expect("permissions");
		assert!(!is_executable(file.path()));
	}
}
