//! Identity of the Rust toolchain that compiles the linted project.
//!
//! The prebuilt lint driver links the compiler's unstable `rustc_private`
//! crates, so a driver only loads against the exact compiler revision it was
//! built with. Two dated nightlies can share a release line such as
//! `1.95.0-nightly` while exposing incompatible compiler libraries, so the
//! release line alone cannot identify a driver's compiler.
//!
//! `pina lint` therefore reads the identity from `rustc -vV` and keys the
//! managed driver cache by host triple and full commit hash. Two nightlies
//! installed on one machine resolve to different cache entries and both keep
//! working, and a cached driver is never reused for a compiler it was not
//! built against.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

/// Environment variable overriding the managed lint-driver cache directory.
///
/// The repository's own tests use it to keep a negotiated driver out of the
/// developer's real cache. Nothing else needs to set it: the default location
/// is per-user and content-addressed by toolchain.
pub const PINA_LINT_CACHE_DIR: &str = "PINA_LINT_CACHE_DIR";

/// The shortest compiler commit hash accepted as an identity.
///
/// `rustc -vV` reports the full 40-character hash. Shorter values are rejected
/// so a truncated report can never be mistaken for a distinct toolchain and
/// silently share another revision's driver.
const MIN_COMMIT_HASH: usize = 9;

/// The compiler a lint driver must be built with to load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolchainIdentity {
	/// Release line reported by the compiler, for example `1.95.0-nightly`.
	pub release: String,

	/// Host triple the compiler runs on.
	///
	/// The driver must match this triple: it links the compiler libraries
	/// installed for the host, not the target the project compiles for.
	pub host: String,

	/// Full commit hash of the compiler revision.
	pub commit_hash: String,

	/// Date of the compiler revision, for example `2026-02-19`.
	pub commit_date: String,
}

impl ToolchainIdentity {
	/// Return the cache key that separates one compiler revision from another.
	///
	/// The key carries the host as well as the revision because a driver built
	/// for one host cannot link another host's compiler libraries.
	#[must_use]
	pub fn cache_key(&self) -> String {
		format!("{}-{}", self.host, self.commit_hash)
	}

	/// Return the abbreviated commit hash used in human-readable output.
	#[must_use]
	pub fn short_commit(&self) -> &str {
		let length = self.commit_hash.len().min(MIN_COMMIT_HASH);
		&self.commit_hash[..length]
	}
}

impl std::fmt::Display for ToolchainIdentity {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			formatter,
			"{} ({} {}, {})",
			self.release,
			self.host,
			self.short_commit(),
			self.commit_date,
		)
	}
}

/// Errors produced while identifying the active toolchain.
#[derive(Debug, thiserror::Error)]
pub enum ToolchainError {
	#[error("Could not query the active Rust toolchain with `rustc -vV`: {source}")]
	Query { source: std::io::Error },

	#[error(
		"Could not query the active Rust toolchain because `rustc -vV` exited with status {status}"
	)]
	QueryFailed { status: String },

	#[error("`rustc -vV` did not report a release, host, commit hash, and commit date")]
	IncompleteReport,

	#[error(
		"`rustc -vV` reported commit hash `{hash}`, which is not a full compiler revision hash"
	)]
	TruncatedCommitHash { hash: String },
}

/// Identify the toolchain that compiles `project_root`.
///
/// The compiler is resolved the way Cargo resolves it — through `RUSTC` when
/// the caller set it, and through `PATH` otherwise — with the project as the
/// working directory so a `rust-toolchain.toml` pin in the project selects the
/// release whose identity is reported.
///
/// # Errors
///
/// Returns an error when the compiler cannot be run, exits unsuccessfully, or
/// reports a report that does not identify one exact revision.
pub fn identify(project_root: &Path) -> Result<ToolchainIdentity, ToolchainError> {
	let rustc = crate::lint_driver::rustc_program();
	let output = Command::new(rustc)
		.arg("-vV")
		.current_dir(project_root)
		.output()
		.map_err(|source| ToolchainError::Query { source })?;

	if !output.status.success() {
		return Err(ToolchainError::QueryFailed {
			status: output.status.to_string(),
		});
	}

	parse(&String::from_utf8_lossy(&output.stdout)).ok_or(ToolchainError::IncompleteReport)
}

/// Parse the identity out of a `rustc -vV` report.
///
/// Returns `None` unless the report identifies one exact revision: a report
/// missing the commit hash, or carrying a hash too short to be a revision,
/// cannot address a driver safely. The commit hash is only accepted when it is
/// lowercase hexadecimal, which also makes the abbreviated form safe to slice.
fn parse(report: &str) -> Option<ToolchainIdentity> {
	let field = |name: &str| {
		report
			.lines()
			.find_map(|line| line.strip_prefix(name))
			.map(str::trim)
			.filter(|value| !value.is_empty())
			.map(str::to_owned)
	};

	let release = field("release:")?;
	let host = field("host:")?;
	let commit_hash = field("commit-hash:")?;
	let commit_date = field("commit-date:")?;

	if !commit_hash
		.chars()
		.all(|character| character.is_ascii_digit() || character.is_ascii_lowercase())
		|| commit_hash.len() <= MIN_COMMIT_HASH
	{
		return None;
	}

	Some(ToolchainIdentity {
		release,
		host,
		commit_hash,
		commit_date,
	})
}

/// Return the directory holding negotiated lint drivers.
///
/// Returns `None` when the platform exposes no per-user cache location, in
/// which case the CLI still lints through the driver bundled next to its own
/// executable.
#[must_use]
pub fn cache_root() -> Option<PathBuf> {
	if let Some(directory) = std::env::var_os(PINA_LINT_CACHE_DIR).filter(|value| !value.is_empty())
	{
		return Some(PathBuf::from(directory));
	}

	platform_cache_directory().map(|directory| directory.join("pina").join("lint-driver"))
}

/// Return the driver cache path for one CLI release and toolchain.
///
/// The CLI version is part of the path because the lints themselves change
/// between releases: a driver for another release would run another lint set.
#[must_use]
pub fn cached_driver_path(
	cache_root: &Path,
	cli_version: &str,
	identity: &ToolchainIdentity,
) -> PathBuf {
	cache_root
		.join(cli_version)
		.join(identity.cache_key())
		.join(crate::lint_driver::driver_binary_name())
}

/// Return the per-user cache directory for the platform.
#[cfg(windows)]
fn platform_cache_directory() -> Option<PathBuf> {
	std::env::var_os("LOCALAPPDATA")
		.filter(|value| !value.is_empty())
		.map(PathBuf::from)
}

/// Return the per-user cache directory for the platform.
#[cfg(target_os = "macos")]
fn platform_cache_directory() -> Option<PathBuf> {
	std::env::var_os("HOME")
		.filter(|value| !value.is_empty())
		.map(|home| PathBuf::from(home).join("Library").join("Caches"))
}

/// Return the per-user cache directory for the platform.
#[cfg(all(unix, not(target_os = "macos")))]
fn platform_cache_directory() -> Option<PathBuf> {
	if let Some(directory) = std::env::var_os("XDG_CACHE_HOME").filter(|value| !value.is_empty()) {
		return Some(PathBuf::from(directory));
	}

	std::env::var_os("HOME")
		.filter(|value| !value.is_empty())
		.map(|home| PathBuf::from(home).join(".cache"))
}

#[cfg(test)]
mod tests {
	use super::*;

	/// A report in the exact shape `rustc -vV` prints.
	const REPORT: &str = "\
rustc 1.95.0-nightly (7f99507f5 2026-02-19)
binary: rustc
commit-hash: 7f99507f57e6c4aa0dce3daf6a13cca8cd4dd312
commit-date: 2026-02-19
host: aarch64-apple-darwin
release: 1.95.0-nightly
LLVM version: 22.1.0
";

	#[test]
	fn parses_every_field_of_a_version_report() {
		let identity = parse(REPORT).expect("the report identifies one revision");
		assert_eq!(identity.release, "1.95.0-nightly");
		assert_eq!(identity.host, "aarch64-apple-darwin");
		assert_eq!(
			identity.commit_hash,
			"7f99507f57e6c4aa0dce3daf6a13cca8cd4dd312"
		);
		assert_eq!(identity.commit_date, "2026-02-19");
		assert_eq!(identity.short_commit(), "7f99507f5");
	}

	#[test]
	fn cache_keys_separate_revisions_of_the_same_release_line() {
		let first = parse(REPORT).expect("first revision");
		let second = parse(&REPORT.replace("7f99507f57", "8a10618068"))
			.expect("second revision with the same release line");

		assert_eq!(first.release, second.release);
		assert_ne!(
			first.cache_key(),
			second.cache_key(),
			"two nightlies on one release line must not share a cache entry"
		);
		assert!(first.cache_key().starts_with("aarch64-apple-darwin-"));
	}

	#[test]
	fn cache_keys_separate_hosts_of_one_revision() {
		let linux = parse(&REPORT.replace("aarch64-apple-darwin", "x86_64-unknown-linux-gnu"))
			.expect("linux host");

		assert_ne!(
			parse(REPORT).expect("apple host").cache_key(),
			linux.cache_key(),
			"a driver cannot link another host's compiler libraries"
		);
	}

	#[test]
	fn rejects_reports_that_do_not_identify_one_revision() {
		assert_eq!(parse(""), None);
		assert_eq!(parse("rustc 1.95.0-nightly\n"), None);

		// A missing commit date is as unusable as a missing hash: the cache key
		// and the diagnostics both name the revision.
		let without_date = REPORT.replace("commit-date: 2026-02-19\n", "");
		assert_eq!(parse(&without_date), None);

		// A truncated hash would collide across revisions.
		let truncated =
			parse(&REPORT.replace("7f99507f57e6c4aa0dce3daf6a13cca8cd4dd312", "7f99507f"));
		assert_eq!(truncated, None);

		// Anything that is not lowercase hexadecimal is rejected rather than
		// becoming a cache key with unexpected characters.
		let uppercase = parse(&REPORT.replace(
			"7f99507f57e6c4aa0dce3daf6a13cca8cd4dd312",
			"7F99507F57E6C4AA0DCE3DAF6A13CCA8CD4DD312",
		));
		assert_eq!(uppercase, None);
	}

	#[test]
	fn cached_driver_paths_nest_the_release_and_the_toolchain() {
		let root = Path::new("/cache");
		let identity = parse(REPORT).expect("revision");
		let path = cached_driver_path(root, "0.18.0", &identity);

		assert_eq!(
			path,
			root.join("0.18.0")
				.join("aarch64-apple-darwin-7f99507f57e6c4aa0dce3daf6a13cca8cd4dd312")
				.join(crate::lint_driver::driver_binary_name()),
		);
		assert_ne!(
			path,
			cached_driver_path(root, "0.19.0", &identity),
			"a driver from another release runs another lint set"
		);
	}

	#[test]
	fn identity_display_names_the_release_host_and_revision() {
		let identity = parse(REPORT).expect("revision");

		assert_eq!(
			identity.to_string(),
			"1.95.0-nightly (aarch64-apple-darwin 7f99507f5, 2026-02-19)"
		);
	}
}
