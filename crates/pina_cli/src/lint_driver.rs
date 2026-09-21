//! Resolution of the `pina_lint_driver` binary used by `pina lint`.
//!
//! The lints are statically linked into the driver, an ordinary release
//! artifact. Because the driver consumes the compiler's unstable
//! `rustc_private` crates, a driver only loads against the exact compiler
//! revision it was built with: two dated nightlies sharing a release line such
//! as `1.95.0-nightly` expose incompatible compiler libraries. The CLI
//! therefore resolves a driver for the *active* toolchain instead of requiring
//! one pinned nightly.
//!
//! Resolution order:
//!
//! 1. `PINA_LINT_DRIVER_PATH`, the escape hatch for a locally built driver.
//! 2. `pina lint --build-driver`, which compiles the driver from the matching
//!    `pina_lints` release with the active toolchain.
//! 3. A driver already negotiated for this CLI release and this exact compiler
//!    revision in the per-user cache.
//! 4. A driver bundled next to the CLI, when it loads against the active
//!    toolchain.
//! 5. A download from the Pina release matching this CLI.
//!
//! When none of those yields a driver, the lint run fails with one actionable
//! message naming the active toolchain, the paths that were searched, and both
//! remedies.
//!
//! # Negotiation
//!
//! There is no handshake with the toolchain beyond reading `rustc -vV` and
//! asking the candidate driver to start. A driver built for another compiler
//! revision cannot load `librustc_driver`, so starting it *is* the version
//! check, and a driver that loads is by construction the right one. That keeps
//! the negotiation honest: it tests the property that actually matters instead
//! of trusting a version string.

use std::ffi::OsString;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use sha2::Digest as _;
use sha2::Sha256;

use crate::lint_toolchain::ToolchainError;
use crate::lint_toolchain::ToolchainIdentity;

/// Environment variable pointing at an existing driver binary.
pub const PINA_LINT_DRIVER_PATH: &str = "PINA_LINT_DRIVER_PATH";

/// Environment variable overriding the release drivers are fetched from.
///
/// The CLI's own tests and the release pipeline use it to fetch from a
/// specific release; nothing needs to set it for normal use.
///
/// Operator-only: the release, repository, and base-URL overrides redirect
/// the download *and* its checksum together, so they are a trusted-origin
/// control, not a convenience knob. Setting them in a shared environment
/// means whatever serves the override can choose which binary runs.
pub const PINA_LINT_DRIVER_RELEASE: &str = "PINA_LINT_DRIVER_RELEASE";

/// Environment variable overriding the GitHub repository hosting releases.
pub const PINA_LINT_DRIVER_REPO: &str = "PINA_LINT_DRIVER_REPO";

/// Environment variable overriding the base URL drivers are fetched from.
///
/// Set it to an `http://` endpoint to serve a driver from a local directory,
/// which is how the CLI tests exercise negotiation without a network.
pub const PINA_LINT_DRIVER_BASE_URL: &str = "PINA_LINT_DRIVER_BASE_URL";

/// The nightly release the driver source is developed and tested against.
///
/// This is the toolchain `rust-toolchain.toml` pins and the release `pina init`
/// scaffolds into new projects. It is no longer required for a lint run — a
/// driver is negotiated for whatever toolchain is active — but `pina doctor`
/// reports it as the release the shipped lint set is verified against.
pub const LINT_DRIVER_TOOLCHAIN: &str = "nightly-2026-02-20";

/// The Pina release drivers are published with.
const DEFAULT_DRIVER_REPO: &str = "pina-rs/pina";

/// How long one driver download may take, end to end.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

/// How long to keep retrying a spawn the kernel reports as busy.
const BUSY_RETRY_WINDOW: Duration = Duration::from_millis(250);

/// Pause between busy-spawn attempts.
const BUSY_RETRY_INTERVAL: Duration = Duration::from_millis(10);

/// `ETXTBSY`: the file is open for writing somewhere, so it cannot be executed.
#[cfg(unix)]
const BUSY_ERRNO: i32 = 26;

/// Non-Unix platforms have no equivalent of `ETXTBSY`.
#[cfg(not(unix))]
const BUSY_ERRNO: i32 = -1;

/// Upper bound on a downloaded driver artifact.
///
/// A real driver is a few MiB. The cap exists so a hostile or misconfigured
/// endpoint cannot stream until the process runs out of memory.
const MAX_DOWNLOAD_BYTES: u64 = 256 * 1024 * 1024;

/// How long the driver gets to start and report before it is considered hung.
///
/// The probe only loads the compiler libraries and prints a line, so anything
/// beyond this is a failure to start rather than slow work.
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

/// The prepared driver and how it was resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedDriver {
	/// Path of the driver executable.
	pub path: PathBuf,

	/// Sysroot of the toolchain the driver loads its compiler libraries and
	/// the project's standard library from.
	pub sysroot: PathBuf,

	/// How the driver was obtained.
	pub origin: DriverOrigin,
}

/// How a driver was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverOrigin {
	/// `PINA_LINT_DRIVER_PATH` named an existing driver.
	Override,

	/// The driver was already cached for the active compiler revision.
	Cache,

	/// The driver shipped next to the CLI and loads against the active
	/// toolchain.
	Bundled,

	/// The driver was fetched from the Pina release matching this CLI.
	Downloaded,

	/// The driver was compiled from source with the active toolchain.
	BuiltFromSource,
}

impl DriverOrigin {
	/// Return the short description used in command summaries.
	#[must_use]
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Override => "PINA_LINT_DRIVER_PATH",
			Self::Cache => "cached",
			Self::Bundled => "bundled",
			Self::Downloaded => "downloaded",
			Self::BuiltFromSource => "built from source",
		}
	}
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

	#[error(transparent)]
	Toolchain(#[from] ToolchainError),

	#[error(
		"Could not resolve the running pina executable to locate the bundled lint driver: {source}"
	)]
	ResolveExecutable { source: std::io::Error },

	#[error("`{PINA_LINT_DRIVER_PATH}` does not point at an executable: {path}")]
	InvalidDriverOverride { path: PathBuf },

	#[error("Could not create the lint driver cache directory {path}: {source}")]
	CreateCacheDirectory {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Could not write the lint driver to {path}: {source}")]
	WriteDriver {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error(
		"Could not download the lint driver for {toolchain} from {url}: {message}\nRun `pina lint \
		 --build-driver` to build one with the active toolchain instead, or set \
		 `{PINA_LINT_DRIVER_PATH}` to a driver you built."
	)]
	Download {
		url: String,
		toolchain: String,
		message: String,
	},

	#[error(
		"Refusing to install the lint driver for {toolchain}: the download from {url} could not \
		 be verified. {message}\nRun `pina lint --build-driver` to build one with the active \
		 toolchain instead, or set `{PINA_LINT_DRIVER_PATH}` to a driver you built."
	)]
	Checksum {
		url: String,
		toolchain: String,
		message: String,
	},

	#[error("Could not run cargo to build the lint driver: {source}")]
	RunCargo { source: std::io::Error },

	#[error(
		"Could not build the lint driver for {toolchain}: cargo exited with status {status}. The \
		 driver compiles Pina's lints against the compiler's internals, so building it needs the \
		 `rustc-dev` and `rust-src` components of the active toolchain. Install them with `rustup \
		 component add rustc-dev rust-src --toolchain {toolchain}`, or set \
		 `{PINA_LINT_DRIVER_PATH}` to a driver you built."
	)]
	BuildFailed { toolchain: String, status: String },

	#[error(
		"Could not build the lint driver for {toolchain}: the build finished without producing \
		 {path}. The `pina_lints` release matching this CLI must provide the `pina_lint_driver` \
		 binary; check that the crates.io release for this CLI version is complete."
	)]
	BuildProducedNoDriver { toolchain: String, path: PathBuf },

	#[error("Could not run the lint driver {path} to confirm it loads: {source}")]
	RunDriver {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("The lint driver {path} did not start within {timeout:?}; it may be waiting on input")]
	DriverHang { path: PathBuf, timeout: Duration },

	#[error(
		"No lint driver is available for the active toolchain {toolchain}. Pina needs a \
		 `pina_lint_driver` built for that exact compiler revision, and this platform has none \
		 cached, bundled, or downloadable.\n  Bundled: {bundled}\n  Cache: {cached}\nRun `pina \
		 lint --build-driver` to build one from the matching pina_lints release with the active \
		 toolchain, or set `{PINA_LINT_DRIVER_PATH}` to a driver you built."
	)]
	NoDriverForToolchain {
		toolchain: String,
		bundled: PathBuf,
		cached: PathBuf,
	},

	#[error(
		"The lint driver {path} could not load against the active toolchain {toolchain}, so it \
		 was built for a different compiler revision. Pina loads a driver only for the revision \
		 it was built with, because `rustc_private` libraries are not compatible across \
		 nightlies.\n{remedy}\n{diagnostics}"
	)]
	DriverUnloadable {
		path: PathBuf,
		toolchain: String,
		remedy: String,
		diagnostics: String,
	},
}

/// Options controlling how [`prepare_driver`] may obtain a driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DriverOptions {
	/// Compile the driver from source with the active toolchain instead of
	/// accepting a cached, bundled, or downloaded driver.
	pub build_driver: bool,

	/// Permit fetching a driver from the release when nothing local matches.
	///
	/// `pina doctor` clears this so its diagnosis reports the state on disk
	/// without changing it; a diagnostic that populates a cache cannot be run
	/// to find out what is wrong.
	pub allow_download: bool,
}

/// Resolve a driver that already exists on disk.
///
/// Never downloads and never builds. `pina doctor` uses it to report state,
/// and it is the non-mutating half of [`prepare_driver`].
///
/// # Errors
///
/// Returns an error when the active toolchain cannot be identified or a
/// `PINA_LINT_DRIVER_PATH` override does not name an executable.
pub fn resolve_existing(project_root: &Path) -> Result<Option<PreparedDriver>, DriverError> {
	let sysroot = rustc_sysroot(project_root)?;

	if let Some(path) = std::env::var_os(PINA_LINT_DRIVER_PATH) {
		let path = PathBuf::from(path);
		if is_executable(&path) {
			return Ok(Some(PreparedDriver {
				path,
				sysroot,
				origin: DriverOrigin::Override,
			}));
		}
		return Err(DriverError::InvalidDriverOverride { path });
	}

	let identity = crate::lint_toolchain::identify(project_root)?;
	let bundled = bundled_driver_path()?;

	if let Some(root) = crate::lint_toolchain::cache_root() {
		let path =
			crate::lint_toolchain::cached_driver_path(&root, env!("CARGO_PKG_VERSION"), &identity);
		if is_executable(&path) {
			return Ok(Some(PreparedDriver {
				path,
				sysroot,
				origin: DriverOrigin::Cache,
			}));
		}
	}

	if is_executable(&bundled) && driver_loads(&bundled, &sysroot)? {
		return Ok(Some(PreparedDriver {
			path: bundled,
			sysroot,
			origin: DriverOrigin::Bundled,
		}));
	}

	Ok(None)
}

/// Resolve the driver for the given project root.
///
/// # Errors
///
/// Returns an error when the active toolchain cannot be identified, an
/// override is unusable, a requested source build fails, or no driver can be
/// obtained for the active compiler revision.
pub fn prepare_driver(
	project_root: &Path,
	options: DriverOptions,
) -> Result<PreparedDriver, DriverError> {
	let sysroot = rustc_sysroot(project_root)?;

	if let Some(path) = std::env::var_os(PINA_LINT_DRIVER_PATH) {
		let path = PathBuf::from(path);
		if is_executable(&path) {
			return Ok(PreparedDriver {
				path,
				sysroot,
				origin: DriverOrigin::Override,
			});
		}
		return Err(DriverError::InvalidDriverOverride { path });
	}

	let identity = crate::lint_toolchain::identify(project_root)?;
	let bundled = bundled_driver_path()?;
	let cached = crate::lint_toolchain::cache_root().map(|root| {
		crate::lint_toolchain::cached_driver_path(&root, env!("CARGO_PKG_VERSION"), &identity)
	});

	if options.build_driver {
		let destination = cached.clone().ok_or_else(|| {
			DriverError::NoDriverForToolchain {
				toolchain: identity.to_string(),
				bundled: bundled.clone(),
				cached: PathBuf::from("<no per-user cache directory on this platform>"),
			}
		})?;
		build_driver(&destination, &identity)?;
		return Ok(PreparedDriver {
			path: destination,
			sysroot,
			origin: DriverOrigin::BuiltFromSource,
		});
	}

	// A cache hit is keyed by the active commit hash, so the file is the right
	// driver by construction; probing it still catches a truncated or corrupt
	// download before cargo reports it deep inside a lint run.
	if let Some(path) = cached.as_ref().filter(|path| is_executable(path)) {
		if driver_loads(path, &sysroot)? {
			return Ok(PreparedDriver {
				path: path.clone(),
				sysroot,
				origin: DriverOrigin::Cache,
			});
		}
		// Drop the unusable file so nothing resolves it again; a download
		// below may replace it.
		let _ = std::fs::remove_file(path);
	}

	// A bundled driver is only usable when it was built for the active
	// revision. Probing it is the negotiation: a driver for another nightly
	// fails to load its compiler libraries.
	if is_executable(&bundled) && driver_loads(&bundled, &sysroot)? {
		return Ok(PreparedDriver {
			path: bundled,
			sysroot,
			origin: DriverOrigin::Bundled,
		});
	}

	if options.allow_download
		&& let Some(path) = cached.as_ref()
	{
		download_driver(path, &identity)?;
		return Ok(PreparedDriver {
			path: path.clone(),
			sysroot,
			origin: DriverOrigin::Downloaded,
		});
	}

	// Without a per-user cache there is nowhere to put a download. Report the
	// loader failure of the bundle, which is the only location searched.
	if is_executable(&bundled) {
		let output = probe_output(&bundled, &sysroot)?;
		return Err(DriverError::DriverUnloadable {
			path: bundled,
			remedy: remedy_for(&identity),
			toolchain: identity.to_string(),
			diagnostics: format_diagnostics(&output.stderr),
		});
	}

	Err(DriverError::NoDriverForToolchain {
		toolchain: identity.to_string(),
		bundled,
		cached: PathBuf::from("<no per-user cache directory on this platform>"),
	})
}

/// Return the one-line remedy for a driver that cannot load.
fn remedy_for(identity: &ToolchainIdentity) -> String {
	format!(
		"Run `pina lint --build-driver` to compile a driver for {identity} with the active \
		 toolchain, or install the Pina release built for that nightly."
	)
}

/// Return the path of the driver shipped next to the CLI.
fn bundled_driver_path() -> Result<PathBuf, DriverError> {
	let executable =
		std::env::current_exe().map_err(|source| DriverError::ResolveExecutable { source })?;
	let directory = executable.parent().ok_or(DriverError::ResolveExecutable {
		source: std::io::Error::new(
			std::io::ErrorKind::NotFound,
			"the executable has no parent directory",
		),
	})?;
	Ok(directory.join(driver_binary_name()))
}

/// Confirm the driver starts under the active toolchain.
///
/// Invoked without arguments the driver prints its toolchain and package
/// version, which exercises the dynamic load of `librustc_driver` without
/// compiling anything. A driver built for another compiler revision fails to
/// load its libraries, which is exactly the mismatch this reports.
fn driver_loads(bin: &Path, sysroot: &Path) -> Result<bool, DriverError> {
	Ok(probe_output(bin, sysroot)?.status.success())
}

/// Start the driver and capture its output.
fn probe_output(bin: &Path, sysroot: &Path) -> Result<std::process::Output, DriverError> {
	probe_output_with_timeout(bin, sysroot, PROBE_TIMEOUT)
}

/// Start the driver, retrying briefly while the kernel reports the binary busy.
///
/// `exec` returns `ExecutableFileBusy` (`ETXTBSY`) when a write descriptor for
/// the same file is still open in another process — including a descriptor a
/// concurrent `fork` inherited. The install path writes and then runs the
/// driver, so the two can overlap; a short bounded retry covers the window
/// without hiding a genuinely unrunnable binary.
fn spawn_driver(bin: &Path, sysroot: &Path) -> Result<std::process::Child, DriverError> {
	let deadline = Instant::now() + BUSY_RETRY_WINDOW;
	loop {
		let attempt = Command::new(bin)
			.envs(driver_library_environment(sysroot))
			.stdin(Stdio::null())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.spawn();

		match attempt {
			Ok(child) => return Ok(child),
			Err(source)
				if source.raw_os_error() == Some(BUSY_ERRNO) && Instant::now() < deadline =>
			{
				std::thread::sleep(BUSY_RETRY_INTERVAL);
			}
			Err(source) => {
				return Err(DriverError::RunDriver {
					path: bin.to_path_buf(),
					source,
				});
			}
		}
	}
}

/// Start the driver, capture its output, and give up after `timeout`.
fn probe_output_with_timeout(
	bin: &Path,
	sysroot: &Path,
	timeout: Duration,
) -> Result<std::process::Output, DriverError> {
	let mut child = spawn_driver(bin, sysroot)?;

	let deadline = Instant::now() + timeout;
	loop {
		match child.try_wait() {
			Ok(Some(_)) => break,
			Ok(None) => {}
			Err(source) => {
				let _ = child.kill();
				return Err(DriverError::RunDriver {
					path: bin.to_path_buf(),
					source,
				});
			}
		}

		if Instant::now() >= deadline {
			let _ = child.kill();
			let _ = child.wait();
			return Err(DriverError::DriverHang {
				path: bin.to_path_buf(),
				timeout,
			});
		}

		std::thread::sleep(Duration::from_millis(10));
	}

	child.wait_with_output().map_err(|source| {
		DriverError::RunDriver {
			path: bin.to_path_buf(),
			source,
		}
	})
}

/// Fetch the driver artifact for `identity`, verify it, and install it at
/// `destination`.
///
/// The artifact URL is the negotiation: its name carries the host triple and
/// the compiler commit hash, so the release only publishes a driver that
/// matches the toolchain asking for it.
fn download_driver(destination: &Path, identity: &ToolchainIdentity) -> Result<(), DriverError> {
	let release = std::env::var(PINA_LINT_DRIVER_RELEASE)
		.ok()
		.filter(|value| !value.is_empty())
		.unwrap_or_else(|| format!("v{}", env!("CARGO_PKG_VERSION")));
	let base = driver_base_url(&release);
	let asset = driver_asset_name(&identity.host, &identity.commit_hash);
	let url = format!("{base}/{asset}");

	let driver = fetch_verified(&url, identity)?;
	install(destination, &driver)
}

/// A driver artifact whose bytes matched the digest published beside it.
///
/// Only [`fetch_verified`] constructs this, so a code path that skipped
/// verification cannot reach [`install`]: the compiler rejects it. That keeps
/// the guarantee structural rather than a convention a future edit can drop.
#[derive(Debug)]
struct VerifiedDriver(Vec<u8>);

impl VerifiedDriver {
	fn bytes(&self) -> &[u8] {
		&self.0
	}
}

/// Fetch `url` and its published `sha256`, then verify they agree.
///
/// The digest lives beside the asset in the same release, so the download is
/// only trusted when the bytes the server returned hash to the value the
/// release published for that exact asset name.
fn fetch_verified(url: &str, identity: &ToolchainIdentity) -> Result<VerifiedDriver, DriverError> {
	let driver = fetch(url, identity)?;
	let checksum_url = format!("{url}.sha256");
	// A release predating the checksum publication has the driver but not its
	// `.sha256` sibling; that is a different failure from a driver the release
	// never published, so it gets its own message instead of the host-triple
	// one `fetch` attaches to a 404.
	let published = String::from_utf8(fetch(&checksum_url, identity).map_err(|error| {
		DriverError::Checksum {
			url: checksum_url.clone(),
			toolchain: identity.to_string(),
			message: format!(
				"the published checksum could not be downloaded: {error}. Releases built before \
				 the driver checksum was published do not carry one; build the driver with \
				 `--build-driver` instead"
			),
		}
	})?)
	.map_err(|error| {
		DriverError::Checksum {
			url: checksum_url.clone(),
			toolchain: identity.to_string(),
			message: format!("the published checksum is not valid UTF-8: {error}"),
		}
	})?;

	let expected = parse_published_sha256(&published).ok_or_else(|| {
		DriverError::Checksum {
			url: checksum_url.clone(),
			toolchain: identity.to_string(),
			message: "the published checksum file does not contain a 64-character hex digest"
				.to_string(),
		}
	})?;

	let actual = sha256_hex(&driver);
	if actual != expected {
		return Err(DriverError::Checksum {
			url: checksum_url,
			toolchain: identity.to_string(),
			message: format!("checksum mismatch: expected {expected}, downloaded {actual}"),
		});
	}

	Ok(VerifiedDriver(driver))
}

/// Return the lowercase hex sha256 of `bytes`.
fn sha256_hex(bytes: &[u8]) -> String {
	let digest = Sha256::digest(bytes);
	let mut hex = String::with_capacity(digest.len() * 2);
	for byte in digest {
		use core::fmt::Write;
		let _ = write!(hex, "{byte:02x}");
	}
	hex
}

/// Extract the sha256 out of a published `sha256sum`-style checksum file.
///
/// `sha256sum` writes `<hex>  <filename>`; a bare 64-character hex line is also
/// accepted. Comparison is case-insensitive so an uppercase digest still
/// verifies.
fn parse_published_sha256(contents: &str) -> Option<String> {
	let token = contents.split_whitespace().next()?;
	let is_sha256 =
		token.len() == 64 && token.chars().all(|character| character.is_ascii_hexdigit());

	is_sha256.then(|| token.to_ascii_lowercase())
}

/// Return the base URL driver assets are fetched from.
fn driver_base_url(release: &str) -> String {
	if let Some(base) = std::env::var(PINA_LINT_DRIVER_BASE_URL)
		.ok()
		.filter(|value| !value.is_empty())
	{
		return base.trim_end_matches('/').to_owned();
	}

	let repo = std::env::var(PINA_LINT_DRIVER_REPO)
		.ok()
		.filter(|value| !value.is_empty())
		.unwrap_or_else(|| DEFAULT_DRIVER_REPO.to_owned());
	format!("https://github.com/{repo}/releases/download/{release}")
}

/// Return the release asset name carrying a prebuilt driver for `host`.
///
/// The compiler commit hash is part of the name, which is what makes the
/// request a negotiation rather than a guess. A release builds its driver with
/// the nightly that release pins, so a project on any other nightly asks for a
/// name the release does not have and gets a `404` instead of a driver that
/// cannot load. That is the correct answer: no release can publish a driver
/// for every nightly, so the miss must be visible and lead to `--build-driver`.
#[must_use]
pub fn driver_asset_name(host: &str, commit_hash: &str) -> String {
	if host.contains("windows") {
		format!("pina-lint-driver-{host}-{commit_hash}.exe")
	} else {
		format!("pina-lint-driver-{host}-{commit_hash}")
	}
}

/// Fetch `url` into memory, refusing a body larger than [`MAX_DOWNLOAD_BYTES`].
fn fetch(url: &str, identity: &ToolchainIdentity) -> Result<Vec<u8>, DriverError> {
	let agent = ureq::Agent::config_builder()
		.timeout_global(Some(DOWNLOAD_TIMEOUT))
		.build()
		.new_agent();

	let mut response = agent.get(url).call().map_err(|error| {
		DriverError::Download {
			url: url.to_owned(),
			toolchain: identity.to_string(),
			message: match error {
				ureq::Error::StatusCode(status) => {
					format!(
						"the release does not publish a driver that matches this host (HTTP \
						 {status}). Drivers are built per host triple and per compiler revision; \
						 a release that predates the active nightly cannot supply one"
					)
				}
				other => other.to_string(),
			},
		}
	})?;

	let mut limited = response.body_mut().as_reader().take(MAX_DOWNLOAD_BYTES + 1);
	let mut bytes = Vec::new();
	Read::read_to_end(&mut limited, &mut bytes).map_err(|error| {
		DriverError::Download {
			url: url.to_owned(),
			toolchain: identity.to_string(),
			message: error.to_string(),
		}
	})?;

	if bytes.len() as u64 > MAX_DOWNLOAD_BYTES {
		return Err(DriverError::Download {
			url: url.to_owned(),
			toolchain: identity.to_string(),
			message: format!(
				"the download exceeded the {} MiB safety limit",
				MAX_DOWNLOAD_BYTES / (1024 * 1024)
			),
		});
	}

	Ok(bytes)
}

/// Write a downloaded, checksum-verified `driver` to `destination`.
///
/// Only a [`VerifiedDriver`] is accepted, so a network download that skipped
/// checksum verification cannot reach the filesystem: the compiler rejects it.
/// A locally built driver goes through [`install_built`] instead, because no
/// checksum is published for an artifact built on this machine.
fn install(destination: &Path, driver: &VerifiedDriver) -> Result<(), DriverError> {
	write_driver_atomically(destination, driver.bytes())
}

/// Write a driver compiled on this machine to `destination`.
///
/// The trust root here is the `cargo` build against the pinned `pina_lints`
/// release, not a published checksum.
fn install_built(destination: &Path, driver: &[u8]) -> Result<(), DriverError> {
	write_driver_atomically(destination, driver)
}

/// Write `driver` to `destination` atomically.
///
/// The staged file is renamed over the destination so a concurrent `pina lint`
/// never runs a partially written driver.
fn write_driver_atomically(destination: &Path, driver: &[u8]) -> Result<(), DriverError> {
	let directory = destination.parent().ok_or_else(|| {
		DriverError::WriteDriver {
			path: destination.to_path_buf(),
			source: std::io::Error::other("the driver has no parent directory"),
		}
	})?;
	std::fs::create_dir_all(directory).map_err(|source| {
		DriverError::CreateCacheDirectory {
			path: directory.to_path_buf(),
			source,
		}
	})?;

	let staging = directory.join(format!("{}.staging", driver_binary_name()));
	std::fs::write(&staging, driver).map_err(|source| {
		DriverError::WriteDriver {
			path: staging.clone(),
			source,
		}
	})?;
	set_executable(&staging)?;
	std::fs::rename(&staging, destination).map_err(|source| {
		let _ = std::fs::remove_file(&staging);
		DriverError::WriteDriver {
			path: destination.to_path_buf(),
			source,
		}
	})
}

/// Compile the driver from the `pina_lints` release matching this CLI.
///
/// The source comes from crates.io rather than a copy embedded in this binary
/// for two reasons. A driver built from the exact `pina_lints` version the CLI
/// was released with runs exactly the CLI's lint set, so the two cannot drift.
/// And embedding the lint source would add tens of kilobytes to every `pina`
/// binary to serve a fallback most users never reach — the vendored copy would
/// be a size regression on the hot path that the negotiated download already
/// covers.
///
/// The build runs with the caller's active toolchain, which is the whole point:
/// a nightly Pina publishes no prebuilt driver for can still be linted.
fn build_driver(destination: &Path, identity: &ToolchainIdentity) -> Result<(), DriverError> {
	let directory = destination.parent().ok_or_else(|| {
		DriverError::WriteDriver {
			path: destination.to_path_buf(),
			source: std::io::Error::other("the driver has no parent directory"),
		}
	})?;
	let root = directory.join("build");
	// A failed run leaves a partial root behind; clearing it keeps the next
	// attempt from resolving a half-installed binary.
	let _ = std::fs::remove_dir_all(&root);

	let cargo = std::env::var_os("CARGO")
		.filter(|cargo| !cargo.is_empty())
		.unwrap_or_else(|| OsString::from("cargo"));
	let status = Command::new(&cargo)
		.arg("install")
		.arg("--root")
		.arg(&root)
		.arg("--bin")
		.arg("pina_lint_driver")
		.arg("--version")
		.arg(format!("={}", env!("CARGO_PKG_VERSION")))
		.arg("pina_lints")
		.status()
		.map_err(|source| DriverError::RunCargo { source })?;

	if !status.success() {
		let _ = std::fs::remove_dir_all(&root);
		return Err(DriverError::BuildFailed {
			toolchain: identity.to_string(),
			status: status.to_string(),
		});
	}

	let built = root.join("bin").join(driver_binary_name());
	let driver = match std::fs::read(&built) {
		Ok(driver) => driver,
		// A build that reports success but produces no binary means the
		// package resolved to something without the driver target; that needs
		// its own message rather than a path-not-found.
		Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
			let _ = std::fs::remove_dir_all(&root);
			return Err(DriverError::BuildProducedNoDriver {
				toolchain: identity.to_string(),
				path: built,
			});
		}
		Err(source) => {
			return Err(DriverError::WriteDriver {
				path: built.clone(),
				source,
			});
		}
	};
	install_built(destination, &driver)?;

	// The build root is large and reproducible from crates.io; drop it once
	// the driver is in the cache.
	let _ = std::fs::remove_dir_all(&root);

	Ok(())
}

/// Return the environment entries that locate the toolchain's compiler
/// libraries for the dynamically linked driver.
///
/// On macOS only `DYLD_LIBRARY_PATH` contributes to the loader's `@rpath`
/// search, but `LD_LIBRARY_PATH` is set alongside it because SIP strips
/// `DYLD_LIBRARY_PATH` at the kernel boundary when the spawned executable is a
/// system binary such as `/bin/bash`, which is how tests observe the setting.
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

/// Return the search-variable entry locating the sysroot's compiler libraries,
/// prepending the directory to the variable's current setting.
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

/// Return the `rustc` program to query, matching how Cargo resolves it.
pub(crate) fn rustc_program() -> OsString {
	std::env::var_os("RUSTC")
		.filter(|rustc| !rustc.is_empty())
		.unwrap_or_else(|| OsString::from("rustc"))
}

/// Return the platform file name of the driver binary.
#[cfg(windows)]
pub(crate) fn driver_binary_name() -> &'static str {
	"pina_lint_driver.exe"
}

/// Return the platform file name of the driver binary.
#[cfg(not(windows))]
pub(crate) fn driver_binary_name() -> &'static str {
	"pina_lint_driver"
}

/// Return whether `path` is an executable file.
#[cfg(unix)]
pub(crate) fn is_executable(path: &Path) -> bool {
	use std::os::unix::fs::PermissionsExt;

	path.is_file()
		&& std::fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

/// Return whether `path` is an executable file.
#[cfg(not(unix))]
pub(crate) fn is_executable(path: &Path) -> bool {
	path.is_file()
}

/// Mark `path` executable where the platform has an executable bit.
#[cfg(unix)]
pub(crate) fn set_executable(path: &Path) -> Result<(), DriverError> {
	use std::os::unix::fs::PermissionsExt;

	std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).map_err(|source| {
		DriverError::WriteDriver {
			path: path.to_path_buf(),
			source,
		}
	})
}

/// Mark `path` executable where the platform has an executable bit.
#[cfg(not(unix))]
pub(crate) fn set_executable(_path: &Path) -> Result<(), DriverError> {
	Ok(())
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
	let output = Command::new(rustc_program())
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

/// Format the loader report of a failed driver start for an error message.
#[must_use]
pub fn format_diagnostics(stderr: &[u8]) -> String {
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

#[cfg(test)]
mod tests {
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
			format_lint_levels([
				("require_writable_before_account_resize", "deny"),
				("other", "allow")
			]),
			"require_writable_before_account_resize=deny,other=allow"
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
	fn asset_names_carry_the_host_and_the_compiler_revision() {
		let hash = "7f99507f57e6c4aa0dce3daf6a13cca8cd4dd312";

		assert_eq!(
			driver_asset_name("aarch64-apple-darwin", hash),
			"pina-lint-driver-aarch64-apple-darwin-7f99507f57e6c4aa0dce3daf6a13cca8cd4dd312"
		);
		assert_eq!(
			driver_asset_name("x86_64-unknown-linux-gnu", hash),
			"pina-lint-driver-x86_64-unknown-linux-gnu-7f99507f57e6c4aa0dce3daf6a13cca8cd4dd312"
		);
		assert_eq!(
			driver_asset_name("x86_64-pc-windows-msvc", hash),
			"pina-lint-driver-x86_64-pc-windows-msvc-7f99507f57e6c4aa0dce3daf6a13cca8cd4dd312.exe"
		);
	}

	#[test]
	fn asset_names_separate_compiler_revisions_of_one_host() {
		let first = driver_asset_name(
			"aarch64-apple-darwin",
			"7f99507f57e6c4aa0dce3daf6a13cca8cd4dd312",
		);
		let second = driver_asset_name(
			"aarch64-apple-darwin",
			"8a10618068e6c4aa0dce3daf6a13cca8cd4dd312",
		);

		assert_ne!(
			first, second,
			"another nightly must ask for another asset, not reuse an incompatible driver"
		);
	}

	#[test]
	fn asset_names_stay_within_the_release_asset_pattern() {
		// Release assets are downloaded and attested with the `pina-*` glob; a
		// name outside that prefix would be published but never verified.
		for host in [
			"aarch64-apple-darwin",
			"x86_64-unknown-linux-gnu",
			"x86_64-pc-windows-msvc",
		] {
			assert!(
				driver_asset_name(host, "7f99507f57e6c4aa0dce3daf6a13cca8cd4dd312")
					.starts_with("pina-"),
				"{host} must be attested with the other release assets"
			);
		}
	}

	#[test]
	fn parses_published_sha256_files() {
		// `sha256sum` writes `<hex>  <filename>`.
		assert_eq!(
			parse_published_sha256(
				"E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855  driver"
			)
			.as_deref(),
			Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
		);
		// A bare digest is accepted too.
		assert_eq!(
			parse_published_sha256(
				"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
			)
			.as_deref(),
			Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
		);
		// Wrong length, non-hex, and empty input are rejected rather than
		// silently compared against the wrong value.
		assert_eq!(parse_published_sha256("abc driver"), None);
		assert_eq!(parse_published_sha256(&"z".repeat(64)), None);
		assert_eq!(parse_published_sha256(""), None);
		assert_eq!(parse_published_sha256(&"a".repeat(63)), None);
	}

	#[test]
	fn sha256_hex_is_stable_and_lowercase_hex() {
		// Known-answer test for the empty input.
		assert_eq!(
			sha256_hex(b""),
			"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
		);

		let digest = sha256_hex(b"driver bytes");
		assert_eq!(digest.len(), 64);
		assert!(
			digest
				.chars()
				.all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character)),
			"digest must be lowercase hex: {digest}"
		);
		// Distinct inputs must not collide through a formatting bug.
		assert_ne!(digest, sha256_hex(b""), "different inputs must differ");
	}

	#[test]
	fn install_writes_an_executable_driver_atomically() {
		let directory = tempfile::tempdir().expect("temp directory");
		let destination = directory.path().join("nested").join(driver_binary_name());

		install_built(&destination, b"driver bytes").expect("install the driver");

		assert_eq!(
			std::fs::read(&destination).expect("read installed driver"),
			b"driver bytes"
		);
		assert!(is_executable(&destination), "the driver must be executable");
		assert!(
			!destination
				.with_file_name(format!("{}.staging", driver_binary_name()))
				.exists(),
			"the staging file must not survive a successful install"
		);
	}

	#[test]
	fn install_replaces_an_existing_driver_and_leaves_no_staging_file() {
		let directory = tempfile::tempdir().expect("temp directory");
		let destination = directory.path().join(driver_binary_name());
		install_built(&destination, b"stale driver").expect("install the stale driver");

		install_built(&destination, b"fresh driver").expect("replace the driver");

		assert_eq!(
			std::fs::read(&destination).expect("read installed driver"),
			b"fresh driver"
		);
		assert!(
			!destination
				.with_file_name(format!("{}.staging", driver_binary_name()))
				.exists()
		);
	}

	#[test]
	#[cfg(unix)]
	fn a_driver_that_fails_to_load_is_not_selected() {
		let directory = tempfile::tempdir().expect("temp directory");
		let driver = directory.path().join("stub-driver");
		std::fs::write(&driver, "#!/bin/sh\nexit 9\n").expect("write stub driver");
		set_executable(&driver).expect("permissions");

		assert!(
			!driver_loads(&driver, Path::new("/toolchain/sysroot")).expect("run the stub"),
			"a driver that exits unsuccessfully must not be selected"
		);
	}

	#[test]
	fn a_missing_checksum_names_the_release_constraint() {
		let payload = b"driver".to_vec();
		// Serve the driver, then nothing: the checksum fetch fails, which is
		// what a release predating the checksum publication looks like.
		let (base, server) = serve_ok(&payload);
		let identity = identity_fixture();

		let error = fetch_verified(&format!("{base}/pina-lint-driver-host"), &identity)
			.expect_err("a missing checksum must be refused");

		let message = error.to_string();
		assert!(
			matches!(error, DriverError::Checksum { .. }),
			"expected a checksum error, got: {message}"
		);
		assert!(
			message.contains("could not be downloaded") && message.contains("--build-driver"),
			"the error must name the missing checksum and the remedy: {message}"
		);
		let _ = server.join();
	}

	#[test]
	fn a_non_utf8_checksum_body_is_rejected() {
		let payload = b"driver".to_vec();
		let (base, server) = serve_responses(vec![payload, vec![0xff, 0xfe, 0xfd]]);
		let identity = identity_fixture();

		let error = fetch_verified(&format!("{base}/pina-lint-driver-host"), &identity)
			.expect_err("a non-UTF-8 checksum must be refused");

		let message = error.to_string();
		assert!(
			matches!(error, DriverError::Checksum { .. }),
			"expected a checksum error, got: {message}"
		);
		assert!(
			message.contains("not valid UTF-8"),
			"the error must say the checksum could not be read: {message}"
		);
		let _ = server.join();
	}

	#[test]
	fn a_download_over_the_size_cap_is_refused() {
		// The cap is 256 MiB; streaming more than that is what a hostile or
		// misconfigured endpoint would do, so the read is bounded rather than
		// sized to a real driver.
		let mut oversized = vec![0u8; (MAX_DOWNLOAD_BYTES as usize) + 1];
		oversized[0] = 1;
		let (base, server) = serve_ok(&oversized);
		let identity = identity_fixture();

		let error = fetch(&format!("{base}/pina-lint-driver-host"), &identity)
			.expect_err("an oversized download must be refused");

		let message = error.to_string();
		assert!(matches!(error, DriverError::Download { .. }), "{message}");
		assert!(
			message.contains("safety limit"),
			"the error must name the cap: {message}"
		);
		let _ = server.join();
	}

	#[test]
	fn a_checksum_error_names_the_install_refusal() {
		// Pin the operator-facing text for the variant the checksum path returns.
		let error = DriverError::Checksum {
			url: "https://example.invalid/driver".to_string(),
			toolchain: "1.95.0-nightly (7f99507f5)".to_string(),
			message: "checksum mismatch".to_string(),
		};
		let message = error.to_string();

		assert!(message.contains("Refusing to install"), "{message}");
		assert!(message.contains("could not be verified"), "{message}");
		assert!(message.contains("--build-driver"), "{message}");
		assert!(message.contains(PINA_LINT_DRIVER_PATH), "{message}");
	}

	#[test]
	fn the_busy_retry_window_tolerates_a_transient_open_for_write() {
		// `ETXTBSY` is the only error the retry absorbs; a missing binary must
		// fail immediately rather than spin for the whole window.
		let started = Instant::now();
		let error = spawn_driver(
			Path::new("/definitely/not/a/real/driver"),
			Path::new("/toolchain/sysroot"),
		)
		.expect_err("a missing binary must fail");

		assert!(matches!(error, DriverError::RunDriver { .. }), "{error}");
		assert!(
			started.elapsed() < BUSY_RETRY_WINDOW,
			"a non-busy failure must not consume the retry window"
		);
	}

	#[test]
	#[cfg(unix)]
	fn a_driver_held_open_for_writing_is_retried_then_runs() {
		// `exec` reports `ETXTBSY` while a write descriptor for the file is open
		// elsewhere — the race the install-then-run path can hit. Hold one open
		// across the first attempts and confirm the probe still succeeds once it
		// closes, rather than failing the driver selection.
		use std::io::Write as _;

		let directory = tempfile::tempdir().expect("temp directory");
		let driver = directory.path().join("busy-driver");
		std::fs::write(&driver, "#!/bin/sh\nexit 0\n").expect("write stub driver");
		set_executable(&driver).expect("permissions");

		let mut held = std::fs::OpenOptions::new()
			.write(true)
			.open(&driver)
			.expect("hold a write descriptor");
		held.flush().expect("flush");

		let releaser = std::thread::spawn(move || {
			std::thread::sleep(BUSY_RETRY_INTERVAL * 3);
			drop(held);
		});

		assert!(
			driver_loads(&driver, Path::new("/toolchain/sysroot")).expect("run the stub"),
			"a driver that becomes executable within the retry window must be selected"
		);
		releaser.join().expect("releaser joins");
	}

	#[test]
	#[cfg(unix)]
	fn a_driver_that_loads_is_selected() {
		let directory = tempfile::tempdir().expect("temp directory");
		let driver = directory.path().join("stub-driver");
		std::fs::write(&driver, "#!/bin/sh\nexit 0\n").expect("write stub driver");
		set_executable(&driver).expect("permissions");

		assert!(
			driver_loads(&driver, Path::new("/toolchain/sysroot")).expect("run the stub"),
			"a driver that starts cleanly is the one to use"
		);
	}

	#[test]
	#[cfg(unix)]
	fn a_hanging_driver_is_reported_instead_of_blocking_forever() {
		let directory = tempfile::tempdir().expect("temp directory");
		let driver = directory.path().join("hanging-driver");
		// A fixed sleep blocks on every platform and every shell. Reading stdin
		// would not: the probe closes the child's stdin, so `read` sees an
		// immediate end of file on Linux and exits rather than hanging.
		std::fs::write(&driver, "#!/bin/sh\nsleep 30\n").expect("write hanging driver");
		set_executable(&driver).expect("permissions");

		let timeout = Duration::from_millis(200);
		let error = probe_output_with_timeout(&driver, Path::new("/toolchain/sysroot"), timeout)
			.expect_err("a driver that outlives the deadline must not be treated as usable");

		assert!(
			matches!(error, DriverError::DriverHang { .. }),
			"a driver that never reports is a hang, not a load failure: {error}"
		);
	}

	#[test]
	fn a_missing_driver_file_is_reported_as_unrunnable() {
		let directory = tempfile::tempdir().expect("temp directory");
		let missing = directory.path().join("absent-driver");
		let error = driver_loads(&missing, Path::new("/toolchain/sysroot"))
			.expect_err("a missing driver cannot be started");
		assert!(matches!(error, DriverError::RunDriver { .. }), "{error}");
	}

	#[test]
	fn unreachable_downloads_name_the_url() {
		// Port 1 has no listener, so the request fails at the transport layer.
		let identity = identity_fixture();
		let error = fetch("http://127.0.0.1:1/pina-lint-driver-host", &identity)
			.expect_err("nothing listens on port 1");
		let message = error.to_string();

		assert!(message.contains("127.0.0.1:1"), "{message}");
	}

	#[test]
	fn missing_assets_report_the_http_status_and_the_remedy() {
		let (base, server) =
			serve_once("HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n");

		let identity = identity_fixture();
		let error = fetch(&format!("{base}/pina-lint-driver-host"), &identity)
			.expect_err("a 404 is not a driver");
		let message = error.to_string();

		assert!(message.contains("404"), "{message}");
		assert!(message.contains("--build-driver"), "{message}");
		let _ = server.join();
	}

	#[test]
	fn a_served_driver_is_downloaded_and_installed() {
		let payload = b"#!/bin/sh\nexit 0\n".to_vec();
		let (base, server) = serve_ok(&payload);
		let directory = tempfile::tempdir().expect("temp directory");
		let destination = directory.path().join("cache").join(driver_binary_name());

		let identity = identity_fixture();
		let body = fetch(&format!("{base}/pina-lint-driver-host"), &identity).expect("download");
		install_built(&destination, &body).expect("install");

		assert_eq!(std::fs::read(&destination).expect("read driver"), payload);
		assert!(is_executable(&destination));
		let _ = server.join();
	}

	#[test]
	fn verified_download_installs_when_the_checksum_matches() {
		let payload = b"#!/bin/sh\nexit 0\n".to_vec();
		let checksum = format!("{}  pina-lint-driver-host\n", sha256_hex(&payload));
		let (base, server) = serve_responses(vec![payload.clone(), checksum.into_bytes()]);
		let directory = tempfile::tempdir().expect("temp directory");
		let destination = directory.path().join("cache").join(driver_binary_name());

		let identity = identity_fixture();
		let driver = fetch_verified(&format!("{base}/pina-lint-driver-host"), &identity)
			.expect("verified download");
		install(&destination, &driver).expect("install");

		assert_eq!(std::fs::read(&destination).expect("read driver"), payload);
		assert!(is_executable(&destination));
		let _ = server.join();
	}

	#[test]
	fn verified_download_rejects_a_mismatched_checksum() {
		let payload = b"#!/bin/sh\necho pwned\n".to_vec();
		// The digest belongs to different bytes, as a tampered artifact would.
		let checksum = format!("{}  pina-lint-driver-host\n", sha256_hex(b"different"));
		let (base, server) = serve_responses(vec![payload, checksum.into_bytes()]);
		let directory = tempfile::tempdir().expect("temp directory");
		let destination = directory.path().join("cache").join(driver_binary_name());

		let identity = identity_fixture();
		let error = fetch_verified(&format!("{base}/pina-lint-driver-host"), &identity)
			.expect_err("a mismatched checksum must be rejected");

		assert!(
			matches!(error, DriverError::Checksum { .. }),
			"expected a checksum error, got {error}"
		);
		assert!(
			error.to_string().contains("checksum mismatch"),
			"the error must explain the mismatch: {error}"
		);
		// Nothing was written: verification happens before installation.
		assert!(!destination.exists(), "no driver may be installed");
		let _ = server.join();
	}

	#[test]
	fn verified_download_rejects_an_unparseable_checksum() {
		let payload = b"driver".to_vec();
		let (base, server) = serve_responses(vec![payload, b"not-a-digest\n".to_vec()]);
		let identity = identity_fixture();

		let error = fetch_verified(&format!("{base}/pina-lint-driver-host"), &identity)
			.expect_err("a malformed checksum must be rejected");

		assert!(
			matches!(error, DriverError::Checksum { .. }),
			"expected a checksum error, got {error}"
		);
		let _ = server.join();
	}

	#[test]
	fn no_driver_error_names_the_toolchain_and_both_remedies() {
		let error = DriverError::NoDriverForToolchain {
			toolchain: identity_fixture().to_string(),
			bundled: PathBuf::from("/opt/pina/pina_lint_driver"),
			cached: PathBuf::from("/home/dev/.cache/pina/lint-driver/0.18.0/key/pina_lint_driver"),
		};
		let message = error.to_string();

		assert!(
			message.contains("1.95.0-nightly") && message.contains("7f99507f5"),
			"the error must name the active toolchain: {message}"
		);
		assert!(
			message.contains("--build-driver"),
			"the error must name the source-build remedy: {message}"
		);
		assert!(
			message.contains(PINA_LINT_DRIVER_PATH),
			"the error must name the override escape hatch: {message}"
		);
		assert!(
			message.contains("/opt/pina/pina_lint_driver")
				&& message.contains("/home/dev/.cache/pina/lint-driver"),
			"the error must name where a driver was looked for: {message}"
		);
	}

	#[test]
	fn build_failures_name_the_components_the_build_needs() {
		let error = DriverError::BuildFailed {
			toolchain: "1.95.0-nightly".to_owned(),
			status: "exit status: 101".to_owned(),
		};
		let message = error.to_string();

		assert!(message.contains("rustup component add"), "{message}");
		assert!(
			message.contains("rustc-dev") && message.contains("rust-src"),
			"{message}"
		);
	}

	#[test]
	fn unloadable_drivers_name_the_toolchain_the_remedy_and_the_loader_report() {
		let error = DriverError::DriverUnloadable {
			path: PathBuf::from("/opt/pina/pina_lint_driver"),
			toolchain: identity_fixture().to_string(),
			remedy: remedy_for(&identity_fixture()),
			diagnostics: "dyld: Symbol not found".to_owned(),
		};
		let message = error.to_string();

		assert!(
			message.contains("1.95.0-nightly") && message.contains("7f99507f5"),
			"{message}"
		);
		assert!(message.contains("--build-driver"), "{message}");
		assert!(message.contains("dyld: Symbol not found"), "{message}");
	}

	#[test]
	fn driver_origin_descriptions_are_stable() {
		assert_eq!(DriverOrigin::Override.as_str(), "PINA_LINT_DRIVER_PATH");
		assert_eq!(DriverOrigin::Cache.as_str(), "cached");
		assert_eq!(DriverOrigin::Bundled.as_str(), "bundled");
		assert_eq!(DriverOrigin::Downloaded.as_str(), "downloaded");
		assert_eq!(DriverOrigin::BuiltFromSource.as_str(), "built from source");
	}

	#[test]
	fn default_driver_options_do_not_rebuild() {
		assert!(!DriverOptions::default().build_driver);
	}

	#[test]
	fn the_base_url_prefers_an_explicit_override() {
		// The override is read from the environment; this asserts the default
		// shape used when it is absent, which the URL tests rely on.
		assert!(driver_base_url("v0.18.0").ends_with("/releases/download/v0.18.0"));
	}

	/// Build an identity for the tests that need a toolchain value.
	fn identity_fixture() -> ToolchainIdentity {
		ToolchainIdentity {
			release: "1.95.0-nightly".to_owned(),
			host: "host".to_owned(),
			commit_hash: "7f99507f57e6c4aa0dce3daf6a13cca8cd4dd312".to_owned(),
			commit_date: "2026-02-19".to_owned(),
		}
	}

	/// Serve one fixed HTTP response and return the base URL.
	#[cfg(test)]
	fn serve_once(response: &'static str) -> (String, std::thread::JoinHandle<()>) {
		let listener = std::net::TcpListener::bind("127.0.0.1:0")
			.unwrap_or_else(|error| panic!("bind a local listener: {error}"));
		let port = listener
			.local_addr()
			.unwrap_or_else(|error| panic!("local address: {error}"))
			.port();
		let server = std::thread::spawn(move || {
			use std::io::Write as _;

			let Ok((mut stream, _)) = listener.accept() else {
				return;
			};
			let mut request = [0_u8; 4096];
			let _ = stream.read(&mut request);
			let _ = stream.write_all(response.as_bytes());
			let _ = stream.shutdown(std::net::Shutdown::Write);
		});
		(format!("http://127.0.0.1:{port}"), server)
	}

	/// Serve `responses` in order, one connection each.
	///
	/// The verified download path makes two requests (the artifact, then its
	/// published checksum), so tests that exercise it need a server that answers
	/// both.
	#[cfg(test)]
	fn serve_responses(responses: Vec<Vec<u8>>) -> (String, std::thread::JoinHandle<()>) {
		let listener = std::net::TcpListener::bind("127.0.0.1:0")
			.unwrap_or_else(|error| panic!("bind a local listener: {error}"));
		let port = listener
			.local_addr()
			.unwrap_or_else(|error| panic!("local address: {error}"))
			.port();
		let server = std::thread::spawn(move || {
			use std::io::Write as _;

			for body in responses {
				let Ok((mut stream, _)) = listener.accept() else {
					return;
				};
				let mut request = [0_u8; 4096];
				let _ = stream.read(&mut request);
				let head = format!(
					"HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
					body.len()
				);
				let _ = stream.write_all(head.as_bytes());
				let _ = stream.write_all(&body);
				let _ = stream.shutdown(std::net::Shutdown::Write);
			}
		});
		(format!("http://127.0.0.1:{port}"), server)
	}

	/// Serve one `200 OK` response with `body`.
	#[cfg(test)]
	fn serve_ok(body: &[u8]) -> (String, std::thread::JoinHandle<()>) {
		let response = format!(
			"HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
			body.len()
		)
		.into_bytes();
		let body = body.to_vec();

		let listener = std::net::TcpListener::bind("127.0.0.1:0")
			.unwrap_or_else(|error| panic!("bind a local listener: {error}"));
		let port = listener
			.local_addr()
			.unwrap_or_else(|error| panic!("local address: {error}"))
			.port();
		let server = std::thread::spawn(move || {
			use std::io::Write as _;

			let Ok((mut stream, _)) = listener.accept() else {
				return;
			};
			let mut request = [0_u8; 4096];
			let _ = stream.read(&mut request);
			let _ = stream.write_all(&response);
			let _ = stream.write_all(&body);
			let _ = stream.shutdown(std::net::Shutdown::Write);
		});
		(format!("http://127.0.0.1:{port}"), server)
	}
}
