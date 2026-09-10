//! Resolution of the prebuilt `pina_lint_driver` binary used by `pina lint`.
//!
//! The lints are statically linked into the driver, which is an ordinary
//! release artifact shipped next to the `pina` CLI in every release archive
//! and npm platform package. The CLI never builds or installs anything: it
//! runs the driver beside its own executable. Because the driver consumes the
//! compiler's unstable `rustc_private` crates, its compiler libraries load
//! from the toolchain the project activates; the CLI therefore points the
//! platform's library-path variable at the active sysroot and confirms the
//! driver loads before running cargo. The `PINA_LINT_DRIVER_PATH`
//! environment variable points at a locally built driver instead; the
//! repository's own tasks and tests use it.

use std::ffi::OsString;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use sha2::Digest as _;
use sha2::Sha256;

/// Environment variable pointing at an existing driver binary.
const PINA_LINT_DRIVER_PATH: &str = "PINA_LINT_DRIVER_PATH";

/// The nightly release the prebuilt lint driver targets.
///
/// Keep in sync with `rust-toolchain.toml` and the toolchain template that
/// `pina init` writes: the shipped driver links the compiler libraries of
/// that nightly, so the project must activate the same release for its
/// sysroot to supply them.
pub const LINT_DRIVER_TOOLCHAIN: &str = "nightly-2026-02-20";

/// The prepared driver and how it was resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedDriver {
	/// Path of the driver executable.
	pub path: PathBuf,

	/// Sysroot of the toolchain the driver loads its compiler libraries and
	/// the project's standard library from.
	pub sysroot: PathBuf,
}

/// Errors produced while preparing the lint driver.
#[derive(Debug, thiserror::Error)]
pub enum DriverError {
	#[error("Could not query the Rust compiler sysroot: {source}")]
	QuerySysroot { source: std::io::Error },

	#[error("Could not query the Rust compiler sysroot because rustc exited with status {status}")]
	SysrootFailed { status: String },

	#[error("Could not parse a sysroot from rustc --print sysroot")]
	MissingSysroot,

	#[error(
		"Could not resolve the running pina executable to locate the bundled lint driver: {source}"
	)]
	ResolveExecutable { source: std::io::Error },

	#[error("`PINA_LINT_DRIVER_PATH` does not point at an executable: {path}")]
	InvalidDriverOverride { path: PathBuf },

	#[error(
		"Could not find the prebuilt lint driver {path}. Pina ships the security-lint driver next \
		 to its CLI in release archives and npm platform packages, and a CLI installed with \
		 `cargo install pina_cli` does not include it. Install the prebuilt CLI, or set \
		 `PINA_LINT_DRIVER_PATH` to a `pina_lint_driver` built with the project's toolchain."
	)]
	MissingDriver { path: PathBuf },

	#[error("Could not run the lint driver to confirm it loads: {source}")]
	RunDriver { source: std::io::Error },

	#[error(
		"Could not load the lint driver {path} (status {status}); the prebuilt driver links the \
		 compiler libraries of the {LINT_DRIVER_TOOLCHAIN} release it was built with, resolved \
		 here from sysroot {sysroot}. Pin {LINT_DRIVER_TOOLCHAIN} in the project's \
		 rust-toolchain.toml so that nightly is active, or set `PINA_LINT_DRIVER_PATH` to a \
		 driver built with the active toolchain.\n{diagnostics}"
	)]
	DriverUnloadable {
		path: PathBuf,
		sysroot: PathBuf,
		status: String,
		diagnostics: String,
	},
}

/// Resolve the driver for the given project root.
///
/// The bundled driver sits next to the CLI executable and is started once to
/// confirm it loads against the active toolchain. The `PINA_LINT_DRIVER_PATH`
/// override skips the bundled location and the check because its driver is
/// built and managed outside the CLI.
pub fn prepare_driver(project_root: &Path) -> Result<PreparedDriver, DriverError> {
	let sysroot = rustc_sysroot(project_root)?;

	if let Some(path) = std::env::var_os(PINA_LINT_DRIVER_PATH) {
		let path = PathBuf::from(path);
		if is_executable(&path) {
			return Ok(PreparedDriver { path, sysroot });
		}
		return Err(DriverError::InvalidDriverOverride { path });
	}

	let executable =
		std::env::current_exe().map_err(|source| DriverError::ResolveExecutable { source })?;
	let directory = executable.parent().ok_or(DriverError::ResolveExecutable {
		source: std::io::Error::new(
			std::io::ErrorKind::NotFound,
			"the executable has no parent directory",
		),
	})?;
	let bin = directory.join(driver_binary_name());

	if is_executable(&bin) {
		verify_driver_loads(&bin, &sysroot)?;
		return Ok(PreparedDriver { path: bin, sysroot });
	}
	Err(DriverError::MissingDriver { path: bin })
}

/// Confirm the bundled driver starts under the active toolchain.
///
/// Invoked without arguments the driver prints its toolchain and package
/// version, which exercises the dynamic load of `librustc_driver` without
/// compiling anything. Checking here turns a toolchain mismatch into an error
/// that names the required nightly instead of the opaque exit the lint cargo
/// run would surface later.
fn verify_driver_loads(bin: &Path, sysroot: &Path) -> Result<(), DriverError> {
	let output = Command::new(bin)
		.envs(driver_library_environment(sysroot))
		.output()
		.map_err(|source| DriverError::RunDriver { source })?;

	if !output.status.success() {
		return Err(DriverError::DriverUnloadable {
			path: bin.to_path_buf(),
			sysroot: sysroot.to_path_buf(),
			status: output.status.to_string(),
			diagnostics: format_diagnostics(&output.stderr),
		});
	}
	Ok(())
}

/// Return loader output suitable for an error message.
fn format_diagnostics(stderr: &[u8]) -> String {
	const LIMIT: usize = 2000;

	let diagnostics = String::from_utf8_lossy(stderr).trim().to_owned();
	if diagnostics.is_empty() {
		return "(the loader printed no output)".to_owned();
	}

	// Keep the tail, where the dynamic loader states the paths it tried.
	let mut start = diagnostics.len().saturating_sub(LIMIT);
	while !diagnostics.is_char_boundary(start) {
		start += 1;
	}
	if start == 0 {
		diagnostics
	} else {
		format!("...{}", &diagnostics[start..])
	}
}

/// Return the environment entries that locate the toolchain's compiler
/// libraries for the dynamically linked driver.
///
/// On macOS only `DYLD_LIBRARY_PATH` contributes to the loader's `@rpath`
/// search, but `LD_LIBRARY_PATH` is set alongside it because SIP strips
/// `DYLD_LIBRARY_PATH` at the kernel boundary when the spawned executable is
/// a system binary such as `/bin/bash`, which is how tests observe the
/// setting.
pub fn driver_library_environment(sysroot: &Path) -> Vec<(&'static str, OsString)> {
	library_path_variables()
		.iter()
		.map(|variable| library_environment(sysroot, variable, std::env::var_os(*variable)))
		.collect()
}

/// Return the platform's dynamic-library search variables.
#[cfg(target_os = "macos")]
fn library_path_variables() -> &'static [&'static str] {
	&["DYLD_LIBRARY_PATH", "LD_LIBRARY_PATH"]
}

/// Return the platform's dynamic-library search variables.
#[cfg(all(unix, not(target_os = "macos")))]
fn library_path_variables() -> &'static [&'static str] {
	&["LD_LIBRARY_PATH"]
}

/// Return the platform's dynamic-library search variables.
#[cfg(windows)]
fn library_path_variables() -> &'static [&'static str] {
	&["PATH"]
}

/// Return the search-variable entry locating the sysroot's compiler
/// libraries, prepending the directory to the variable's current setting.
fn library_environment(
	sysroot: &Path,
	variable: &'static str,
	current: Option<OsString>,
) -> (&'static str, OsString) {
	(
		variable,
		prepend_path(current, &sysroot.join(library_directory())),
	)
}

/// Return the sysroot directory holding the compiler's shared libraries.
#[cfg(unix)]
fn library_directory() -> &'static str {
	"lib"
}

/// Return the sysroot directory holding the compiler's shared libraries.
#[cfg(windows)]
fn library_directory() -> &'static str {
	"bin"
}

/// Prepend `directory` to a path-list variable's current value.
fn prepend_path(current: Option<OsString>, directory: &Path) -> OsString {
	let directory = directory.to_path_buf();
	let mut entries = vec![directory.clone()];
	if let Some(current) = current.filter(|current| !current.is_empty()) {
		entries.extend(std::env::split_paths(&current));
	}

	std::env::join_paths(entries).unwrap_or_else(|_| directory.into_os_string())
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

/// Return a deterministic content identity for the prepared driver.
///
/// The identity is forwarded into rustc dep-info so Cargo invalidates a prior
/// lint result when an upgraded driver occupies the same path.
pub fn driver_build_identity(path: &Path) -> std::io::Result<String> {
	let file = File::open(path)?;
	let mut reader = BufReader::new(file);
	let mut buffer = [0u8; 16 * 1024];
	let mut hash = Sha256::new();

	loop {
		let read = reader.read(&mut buffer)?;
		if read == 0 {
			break;
		}
		hash.update(&buffer[..read]);
	}

	let digest = hash.finalize();
	Ok(hex(&digest))
}

/// Resolve the sysroot of the compiler used for `project_root`.
///
/// The sysroot holds the `librustc_driver` library the driver loads, so its
/// library directory anchors the dynamic-library search path handed to every
/// lint process.
fn rustc_sysroot(project_root: &Path) -> Result<PathBuf, DriverError> {
	let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
	let output = Command::new(rustc)
		.arg("--print")
		.arg("sysroot")
		.current_dir(project_root)
		.output()
		.map_err(|source| DriverError::QuerySysroot { source })?;

	if !output.status.success() {
		return Err(DriverError::SysrootFailed {
			status: output.status.to_string(),
		});
	}

	parse_sysroot(&output.stdout).ok_or(DriverError::MissingSysroot)
}

/// Parse a sysroot path from `rustc --print sysroot` output.
fn parse_sysroot(output: &[u8]) -> Option<PathBuf> {
	let sysroot = String::from_utf8_lossy(output).trim().to_owned();
	(!sysroot.is_empty()).then(|| PathBuf::from(sysroot))
}

fn hex(bytes: &[u8]) -> String {
	const HEX: &[u8; 16] = b"0123456789abcdef";
	let mut encoded = String::with_capacity(bytes.len() * 2);
	for byte in bytes {
		encoded.push(char::from(HEX[usize::from(byte >> 4)]));
		encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
	}

	encoded
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
	fn parses_sysroot_output_and_rejects_blank_reports() {
		assert_eq!(
			parse_sysroot(b"/nix/store/2nv9qzwmlnrxa6vh4h0d7c9wkc0ns9hb-rust-default\n"),
			Some(PathBuf::from(
				"/nix/store/2nv9qzwmlnrxa6vh4h0d7c9wkc0ns9hb-rust-default"
			)),
		);
		assert_eq!(parse_sysroot(b"  \n"), None);
		assert_eq!(parse_sysroot(b""), None);
	}

	#[test]
	fn library_environment_prepends_the_sysroot_library_directory() {
		let sysroot = PathBuf::from("/toolchain/sysroot");
		#[cfg(windows)]
		let inherited = OsString::from("C:\\other\\bin");
		#[cfg(not(windows))]
		let inherited = OsString::from("/other/lib");

		let (name, value) = library_environment(
			&sysroot,
			library_path_variables()[0],
			Some(inherited.clone()),
		);
		assert_eq!(name, library_path_variables()[0]);
		let mut entries = std::env::split_paths(&value);
		assert_eq!(
			entries.next(),
			Some(sysroot.join(library_directory())),
			"the sysroot library directory must come first"
		);
		assert_eq!(
			entries.next(),
			Some(PathBuf::from(&inherited)),
			"an inherited search path must survive the prepend"
		);
		assert_eq!(entries.next(), None);
	}

	#[test]
	fn library_environment_handles_an_unset_search_variable() {
		let sysroot = PathBuf::from("/toolchain/sysroot");

		let (_, value) = library_environment(&sysroot, library_path_variables()[0], None);

		let mut entries = std::env::split_paths(&value);
		assert_eq!(entries.next(), Some(sysroot.join(library_directory())));
		assert_eq!(entries.next(), None);
	}

	#[test]
	fn diagnostics_keep_the_tail_of_long_loader_reports() {
		assert_eq!(format_diagnostics(b""), "(the loader printed no output)");
		assert_eq!(format_diagnostics(b"  dyld: boom  \n"), "dyld: boom");

		let long = "x".repeat(5000);
		let formatted = format_diagnostics(long.as_bytes());
		assert_eq!(formatted.len(), 2003);
		assert!(formatted.starts_with("..."));

		// A truncation point inside a multibyte character advances to the next
		// char boundary instead of splitting the character.
		let multibyte = "€".repeat(834);
		let formatted = format_diagnostics(multibyte.as_bytes());
		assert!(formatted.starts_with("...€"));
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

	#[test]
	fn driver_build_identity_tracks_binary_contents() {
		let first = tempfile::NamedTempFile::new().expect("first temp file");
		std::fs::write(first.path(), b"first driver").expect("write first driver");
		let second = tempfile::NamedTempFile::new().expect("second temp file");
		std::fs::write(second.path(), b"second driver").expect("write second driver");

		let first_identity = driver_build_identity(first.path()).expect("fingerprint first driver");
		assert_eq!(
			first_identity,
			driver_build_identity(first.path()).expect("fingerprint first driver again")
		);
		assert_ne!(
			first_identity,
			driver_build_identity(second.path()).expect("fingerprint second driver")
		);
		assert_eq!(first_identity.len(), 64);
	}

	#[test]
	#[cfg(unix)]
	fn unloadable_driver_reports_the_required_toolchain() {
		let directory = tempfile::tempdir().expect("temp directory");
		let driver = directory.path().join("stub-driver");
		std::fs::write(&driver, "#!/bin/sh\nexit 9\n").expect("write stub driver");
		use std::os::unix::fs::PermissionsExt;
		std::fs::set_permissions(&driver, std::fs::Permissions::from_mode(0o755))
			.expect("permissions");

		let error = verify_driver_loads(&driver, Path::new("/toolchain/sysroot"))
			.expect_err("a failing driver should not load");
		let message = error.to_string();
		assert!(
			message.contains("Could not load the lint driver"),
			"unexpected error: {message}",
		);
		assert!(
			message.contains(LINT_DRIVER_TOOLCHAIN) && message.contains("rust-toolchain.toml"),
			"the error should name the required toolchain and the pin file: {message}",
		);
	}
}
