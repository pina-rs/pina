//! Downloading and validating release-built Dylint libraries.

use std::collections::BTreeSet;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use flate2::read::GzDecoder;
use fs2::FileExt;
use serde::Deserialize;
use sha2::Digest;
use sha2::Sha256;

const BUNDLE_SCHEMA_VERSION: u32 = 1;
const CATALOG_SCHEMA_VERSION: u32 = 2;
const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_EXTRACTED_BYTES: u64 = 1024 * 1024 * 1024;
const PINA_RELEASES_API: &str = "https://api.github.com/repos/pina-rs/pina/releases/tags";
const PINA_RELEASE_DOWNLOADS: &str = "https://github.com/pina-rs/pina/releases/download";
const TOOL_NAMES: [&str; 2] = ["cargo-dylint", "dylint-link"];

/// Metadata compiled into the CLI and consumed by the release builder.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LintCatalog {
	pub(crate) schema_version: u32,
	pub(crate) dylint_version: String,
	pub(crate) toolchain: String,
	pub(crate) tool_targets: Vec<String>,
	pub(crate) lint_targets: Vec<String>,
	pub(crate) libraries: Vec<String>,
}

/// One verified release bundle ready for Dylint.
#[derive(Debug)]
pub(crate) struct PreparedBundle {
	pub(crate) library_paths: Vec<PathBuf>,
}

/// Verified Dylint executables ready to run from Pina's managed cache.
#[derive(Debug)]
pub(crate) struct PreparedTools {
	pub(crate) bin_dir: PathBuf,
	pub(crate) cargo_dylint: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BundleManifest {
	schema_version: u32,
	version: String,
	target: String,
	toolchain: String,
	dylint_version: String,
	libraries: Vec<BundleLibrary>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BundleLibrary {
	name: String,
	file: String,
	sha256: String,
	size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolManifest {
	schema_version: u32,
	dylint_version: String,
	target: String,
	executables: Vec<ToolExecutable>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolExecutable {
	name: String,
	file: String,
	sha256: String,
	size: u64,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
	tag_name: String,
	assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
	name: String,
	browser_download_url: String,
	size: u64,
	digest: Option<String>,
}

/// Failures while resolving or validating native lint libraries.
#[derive(Debug, thiserror::Error)]
pub enum BundleError {
	#[error("Could not create the managed lint bundle directory at {path}: {source}")]
	CreateDirectory {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Could not open the managed lint bundle lock at {path}: {source}")]
	OpenLock {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Could not lock the managed lint bundle at {path}: {source}")]
	Lock {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Could not fetch {resource}: {source}")]
	Http {
		resource: String,
		#[source]
		source: ureq::Error,
	},

	#[error("Release metadata for {tag} did not contain {asset}")]
	MissingAsset { tag: String, asset: String },

	#[error("Release asset {asset} did not include GitHub's SHA-256 digest")]
	MissingAssetDigest { asset: String },

	#[error("Release asset {asset} has an unsupported digest: {digest}")]
	UnsupportedAssetDigest { asset: String, digest: String },

	#[error("Release metadata returned tag {actual} while {expected} was requested")]
	UnexpectedReleaseTag { expected: String, actual: String },

	#[error("Release asset {asset} used an unexpected download URL: {url}")]
	UnexpectedDownloadUrl { asset: String, url: String },

	#[error("Release asset {asset} is {actual} bytes; metadata declared {expected} bytes")]
	UnexpectedAssetSize {
		asset: String,
		expected: u64,
		actual: u64,
	},

	#[error(
		"Release asset {asset} failed SHA-256 verification (expected {expected}, got {actual})"
	)]
	AssetDigestMismatch {
		asset: String,
		expected: String,
		actual: String,
	},

	#[error("Could not read the embedded lint catalog: {source}")]
	Catalog { source: serde_json::Error },

	#[error("The embedded lint catalog is invalid: {reason}")]
	InvalidCatalog { reason: String },

	#[error("Rust compiler host {host} cannot load Pina's precompiled lint libraries")]
	UnsupportedLintHost { host: String },

	#[error("Could not read lint bundle manifest at {path}: {source}")]
	ReadManifest {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Could not parse lint bundle manifest at {path}: {source}")]
	ParseManifest {
		path: PathBuf,
		source: serde_json::Error,
	},

	#[error("Lint bundle at {path} is invalid: {reason}")]
	InvalidBundle { path: PathBuf, reason: String },

	#[error("Could not create a temporary lint bundle below {path}: {source}")]
	CreateTemporaryBundle {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Could not unpack lint bundle {asset}: {source}")]
	Unpack {
		asset: String,
		source: std::io::Error,
	},

	#[error("Lint bundle {asset} contains an unsafe archive entry: {entry}")]
	UnsafeArchiveEntry { asset: String, entry: String },

	#[error("Could not replace the invalid cached lint bundle at {path}: {source}")]
	ReplaceCachedBundle {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Could not cache the verified lint bundle at {path}: {source}")]
	CacheBundle {
		path: PathBuf,
		source: std::io::Error,
	},
}

/// Load the versioned catalog embedded in the `pina_cli` crate.
pub(crate) fn lint_catalog() -> Result<LintCatalog, BundleError> {
	let catalog: LintCatalog = serde_json::from_str(include_str!("../lints.json"))
		.map_err(|source| BundleError::Catalog { source })?;
	validate_catalog(&catalog)?;
	Ok(catalog)
}

/// Download once, then validate and reuse the Rust compiler host's native bundle.
pub(crate) fn prepare_bundle(
	cargo_home: &Path,
	target: &str,
) -> Result<PreparedBundle, BundleError> {
	prepare_bundle_from(
		cargo_home,
		target,
		PINA_RELEASES_API,
		PINA_RELEASE_DOWNLOADS,
	)
}

fn prepare_bundle_from(
	cargo_home: &Path,
	target: &str,
	releases_api: &str,
	release_downloads: &str,
) -> Result<PreparedBundle, BundleError> {
	let catalog = lint_catalog()?;
	let version = env!("CARGO_PKG_VERSION");

	if !catalog
		.lint_targets
		.iter()
		.any(|candidate| candidate == target)
	{
		return Err(BundleError::UnsupportedLintHost {
			host: target.to_owned(),
		});
	}

	let root = cargo_home
		.join("pina/lints")
		.join(format!("v{version}"))
		.join(target);
	fs::create_dir_all(&root).map_err(|source| {
		BundleError::CreateDirectory {
			path: root.clone(),
			source,
		}
	})?;

	let _lock = lock_cache(&root)?;

	let bundle_path = root.join("bundle");
	if let Ok(metadata) = fs::symlink_metadata(&bundle_path)
		&& metadata.is_dir()
		&& !metadata.file_type().is_symlink()
		&& let Ok(bundle) = validate_bundle(&bundle_path, &catalog, version, target)
	{
		return Ok(bundle);
	}

	let tag = format!("v{version}");
	let asset_name = format!("pina-lints-{target}-{tag}.tar.gz");
	let release = fetch_release(releases_api, &tag)?;
	let asset = select_asset(&release, release_downloads, &tag, &asset_name)?;
	let archive = fetch_archive(asset)?;
	verify_archive(asset, &archive)?;

	let temporary = tempfile::Builder::new()
		.prefix("download-")
		.tempdir_in(&root)
		.map_err(|source| {
			BundleError::CreateTemporaryBundle {
				path: root.clone(),
				source,
			}
		})?;
	let extracted = temporary.path().join("bundle");
	create_directory(&extracted)?;
	unpack_archive(&archive, &extracted, &asset.name, ArchiveContents::Lints)?;
	let bundle = validate_bundle(&extracted, &catalog, version, target)?;
	replace_cached_directory(&extracted, &bundle_path)?;

	Ok(PreparedBundle {
		library_paths: bundle
			.library_paths
			.into_iter()
			.map(|path| bundle_path.join(path.file_name().expect("validated file has a name")))
			.collect(),
	})
}

/// Download once, then validate and reuse the pinned native Dylint tools.
pub(crate) fn prepare_dylint_tools(cargo_home: &Path) -> Result<PreparedTools, BundleError> {
	prepare_dylint_tools_from(cargo_home, PINA_RELEASES_API, PINA_RELEASE_DOWNLOADS)
}

fn prepare_dylint_tools_from(
	cargo_home: &Path,
	releases_api: &str,
	release_downloads: &str,
) -> Result<PreparedTools, BundleError> {
	let version = lint_catalog()?.dylint_version;
	let target = env!("PINA_BUILD_TARGET");
	let root = cargo_home
		.join("pina/tools")
		.join(format!("dylint-v{version}"))
		.join(target);
	fs::create_dir_all(&root).map_err(|source| {
		BundleError::CreateDirectory {
			path: root.clone(),
			source,
		}
	})?;

	let _lock = lock_cache(&root)?;

	let bundle_path = root.join("bundle");
	if let Ok(metadata) = fs::symlink_metadata(&bundle_path)
		&& metadata.is_dir()
		&& !metadata.file_type().is_symlink()
		&& let Ok(tools) = validate_tool_bundle(&bundle_path, &version, target)
	{
		return Ok(tools);
	}

	let tag = format!("dylint-v{version}");
	let asset_name = format!("pina-dylint-tools-{target}-v{version}.tar.gz");
	let release = fetch_release(releases_api, &tag)?;
	let asset = select_asset(&release, release_downloads, &tag, &asset_name)?;
	let archive = fetch_archive(asset)?;
	verify_archive(asset, &archive)?;

	let temporary = tempfile::Builder::new()
		.prefix("download-")
		.tempdir_in(&root)
		.map_err(|source| {
			BundleError::CreateTemporaryBundle {
				path: root.clone(),
				source,
			}
		})?;
	let extracted = temporary.path().join("bundle");
	create_directory(&extracted)?;
	unpack_archive(&archive, &extracted, &asset.name, ArchiveContents::Tools)?;
	validate_tool_bundle(&extracted, &version, target)?;
	replace_cached_directory(&extracted, &bundle_path)?;
	validate_tool_bundle(&bundle_path, &version, target)
}

fn lock_cache(root: &Path) -> Result<File, BundleError> {
	let lock_path = root.join("bundle.lock");
	let lock = OpenOptions::new()
		.read(true)
		.write(true)
		.create(true)
		.truncate(false)
		.open(&lock_path)
		.map_err(|source| {
			BundleError::OpenLock {
				path: lock_path.clone(),
				source,
			}
		})?;
	lock.lock_exclusive().map_err(|source| {
		BundleError::Lock {
			path: lock_path,
			source,
		}
	})?;
	Ok(lock)
}

fn create_directory(path: &Path) -> Result<(), BundleError> {
	fs::create_dir(path).map_err(|source| {
		BundleError::CreateDirectory {
			path: path.to_path_buf(),
			source,
		}
	})
}

fn replace_cached_directory(source: &Path, destination: &Path) -> Result<(), BundleError> {
	if fs::symlink_metadata(destination).is_ok() {
		let metadata = fs::symlink_metadata(destination).map_err(|source| {
			BundleError::ReplaceCachedBundle {
				path: destination.to_path_buf(),
				source,
			}
		})?;
		if metadata.file_type().is_symlink() || !metadata.is_dir() {
			return Err(BundleError::InvalidBundle {
				path: destination.to_path_buf(),
				reason: "the cache entry is not a regular directory".to_owned(),
			});
		}
		fs::remove_dir_all(destination).map_err(|source| {
			BundleError::ReplaceCachedBundle {
				path: destination.to_path_buf(),
				source,
			}
		})?;
	}
	fs::rename(source, destination).map_err(|source| {
		BundleError::CacheBundle {
			path: destination.to_path_buf(),
			source,
		}
	})
}

fn validate_catalog(catalog: &LintCatalog) -> Result<(), BundleError> {
	if catalog.schema_version != CATALOG_SCHEMA_VERSION {
		return Err(BundleError::InvalidCatalog {
			reason: format!("schema version {} is not supported", catalog.schema_version),
		});
	}
	if semver::Version::parse(&catalog.dylint_version).is_err() {
		return Err(BundleError::InvalidCatalog {
			reason: "Dylint version must use semantic versioning".to_owned(),
		});
	}
	if catalog.toolchain.is_empty()
		|| catalog.tool_targets.is_empty()
		|| catalog.lint_targets.is_empty()
		|| catalog.libraries.is_empty()
	{
		return Err(BundleError::InvalidCatalog {
			reason: "toolchain, tool targets, lint targets, and libraries must not be empty"
				.to_owned(),
		});
	}
	let unique_tool_targets = catalog.tool_targets.iter().collect::<BTreeSet<_>>();
	if unique_tool_targets.len() != catalog.tool_targets.len()
		|| catalog
			.tool_targets
			.iter()
			.any(|target| !is_safe_release_component(target))
		|| !catalog
			.tool_targets
			.iter()
			.any(|target| target == env!("PINA_BUILD_TARGET"))
	{
		return Err(BundleError::InvalidCatalog {
			reason: "tool targets must be unique safe components and include this CLI host"
				.to_owned(),
		});
	}
	let unique_lint_targets = catalog.lint_targets.iter().collect::<BTreeSet<_>>();
	if unique_lint_targets.len() != catalog.lint_targets.len()
		|| catalog
			.lint_targets
			.iter()
			.any(|target| !is_safe_release_component(target))
		|| !catalog
			.lint_targets
			.iter()
			.all(|target| unique_tool_targets.contains(target))
	{
		return Err(BundleError::InvalidCatalog {
			reason: "lint targets must be unique safe components and have matching Dylint tools"
				.to_owned(),
		});
	}
	let unique = catalog.libraries.iter().collect::<BTreeSet<_>>();
	if unique.len() != catalog.libraries.len()
		|| catalog
			.libraries
			.iter()
			.any(|name| !is_safe_component(name))
	{
		return Err(BundleError::InvalidCatalog {
			reason: "library names must be unique safe path components".to_owned(),
		});
	}
	Ok(())
}

fn fetch_release(releases_api: &str, tag: &str) -> Result<GithubRelease, BundleError> {
	let url = format!("{releases_api}/{tag}");
	let mut response = ureq::get(&url)
		.header("Accept", "application/vnd.github+json")
		.header(
			"User-Agent",
			concat!("pina-cli/", env!("CARGO_PKG_VERSION")),
		)
		.header("X-GitHub-Api-Version", "2022-11-28")
		.call()
		.map_err(|source| {
			BundleError::Http {
				resource: format!("release metadata for {tag}"),
				source,
			}
		})?;
	response.body_mut().read_json().map_err(|source| {
		BundleError::Http {
			resource: format!("release metadata for {tag}"),
			source,
		}
	})
}

fn select_asset<'a>(
	release: &'a GithubRelease,
	release_downloads: &str,
	tag: &str,
	asset_name: &str,
) -> Result<&'a GithubAsset, BundleError> {
	if release.tag_name != tag {
		return Err(BundleError::UnexpectedReleaseTag {
			expected: tag.to_owned(),
			actual: release.tag_name.clone(),
		});
	}
	let asset = release
		.assets
		.iter()
		.find(|asset| asset.name == asset_name)
		.ok_or_else(|| {
			BundleError::MissingAsset {
				tag: tag.to_owned(),
				asset: asset_name.to_owned(),
			}
		})?;
	let expected_url = format!("{release_downloads}/{tag}/{}", asset.name);
	if asset.browser_download_url != expected_url {
		return Err(BundleError::UnexpectedDownloadUrl {
			asset: asset.name.clone(),
			url: asset.browser_download_url.clone(),
		});
	}
	Ok(asset)
}

fn fetch_archive(asset: &GithubAsset) -> Result<Vec<u8>, BundleError> {
	let mut response = ureq::get(&asset.browser_download_url)
		.header("Accept", "application/octet-stream")
		.header(
			"User-Agent",
			concat!("pina-cli/", env!("CARGO_PKG_VERSION")),
		)
		.call()
		.map_err(|source| {
			BundleError::Http {
				resource: asset.name.clone(),
				source,
			}
		})?;
	response
		.body_mut()
		.with_config()
		.limit(MAX_ARCHIVE_BYTES)
		.read_to_vec()
		.map_err(|source| {
			BundleError::Http {
				resource: asset.name.clone(),
				source,
			}
		})
}

fn verify_archive(asset: &GithubAsset, archive: &[u8]) -> Result<(), BundleError> {
	let actual_size = u64::try_from(archive.len()).unwrap_or(u64::MAX);
	if actual_size != asset.size {
		return Err(BundleError::UnexpectedAssetSize {
			asset: asset.name.clone(),
			expected: asset.size,
			actual: actual_size,
		});
	}
	let digest = asset.digest.as_deref().ok_or_else(|| {
		BundleError::MissingAssetDigest {
			asset: asset.name.clone(),
		}
	})?;
	let expected = digest.strip_prefix("sha256:").ok_or_else(|| {
		BundleError::UnsupportedAssetDigest {
			asset: asset.name.clone(),
			digest: digest.to_owned(),
		}
	})?;
	let actual = sha256(archive);
	if !actual.eq_ignore_ascii_case(expected) {
		return Err(BundleError::AssetDigestMismatch {
			asset: asset.name.clone(),
			expected: expected.to_owned(),
			actual,
		});
	}
	Ok(())
}

#[derive(Clone, Copy)]
enum ArchiveContents {
	Lints,
	Tools,
}

fn unpack_archive(
	archive: &[u8],
	destination: &Path,
	asset: &str,
	contents: ArchiveContents,
) -> Result<(), BundleError> {
	let decoder = GzDecoder::new(Cursor::new(archive));
	let mut archive = tar::Archive::new(decoder);
	let entries = archive.entries().map_err(|source| {
		BundleError::Unpack {
			asset: asset.to_owned(),
			source,
		}
	})?;
	let mut total_size = 0_u64;
	for entry in entries {
		let mut entry = entry.map_err(|source| {
			BundleError::Unpack {
				asset: asset.to_owned(),
				source,
			}
		})?;
		let path = entry.path().map_err(|source| {
			BundleError::Unpack {
				asset: asset.to_owned(),
				source,
			}
		})?;
		let mut components = path.components();
		let Some(Component::Normal(file_name)) = components.next() else {
			return Err(BundleError::UnsafeArchiveEntry {
				asset: asset.to_owned(),
				entry: path.display().to_string(),
			});
		};
		if components.next().is_some() || !entry.header().entry_type().is_file() {
			return Err(BundleError::UnsafeArchiveEntry {
				asset: asset.to_owned(),
				entry: path.display().to_string(),
			});
		}
		let file_name = file_name.to_string_lossy();
		let allowed = file_name == "manifest.json"
			|| match contents {
				ArchiveContents::Lints => is_dynamic_library(&file_name),
				ArchiveContents::Tools => is_tool_executable(&file_name),
			};
		if !allowed {
			return Err(BundleError::UnsafeArchiveEntry {
				asset: asset.to_owned(),
				entry: path.display().to_string(),
			});
		}
		total_size = total_size.saturating_add(entry.size());
		if total_size > MAX_EXTRACTED_BYTES {
			return Err(BundleError::UnsafeArchiveEntry {
				asset: asset.to_owned(),
				entry: "archive exceeds the extraction size limit".to_owned(),
			});
		}
		let output_path = destination.join(file_name.as_ref());
		let mut output = OpenOptions::new()
			.write(true)
			.create_new(true)
			.open(&output_path)
			.map_err(|source| {
				BundleError::Unpack {
					asset: asset.to_owned(),
					source,
				}
			})?;
		std::io::copy(&mut entry, &mut output).map_err(|source| {
			BundleError::Unpack {
				asset: asset.to_owned(),
				source,
			}
		})?;
		output.flush().map_err(|source| {
			BundleError::Unpack {
				asset: asset.to_owned(),
				source,
			}
		})?;
	}
	Ok(())
}

fn validate_tool_bundle(
	path: &Path,
	version: &str,
	target: &str,
) -> Result<PreparedTools, BundleError> {
	let manifest_path = path.join("manifest.json");
	let bytes = fs::read(&manifest_path).map_err(|source| {
		BundleError::ReadManifest {
			path: manifest_path.clone(),
			source,
		}
	})?;
	let manifest: ToolManifest = serde_json::from_slice(&bytes).map_err(|source| {
		BundleError::ParseManifest {
			path: manifest_path,
			source,
		}
	})?;
	if manifest.schema_version != BUNDLE_SCHEMA_VERSION
		|| manifest.dylint_version != version
		|| manifest.target != target
	{
		return invalid_bundle(
			path,
			"Dylint tool metadata does not match this CLI and host",
		);
	}

	let names = manifest
		.executables
		.iter()
		.map(|executable| executable.name.as_str())
		.collect::<Vec<_>>();
	if names != TOOL_NAMES {
		return invalid_bundle(
			path,
			"Dylint tool bundle does not contain the required tools",
		);
	}

	let mut expected_files = BTreeSet::from(["manifest.json".to_owned()]);
	for executable in &manifest.executables {
		let expected_file = tool_filename(&executable.name, target);
		if executable.file != expected_file || !is_safe_component(&executable.file) {
			return invalid_bundle(
				path,
				&format!("unexpected Dylint tool filename {}", executable.file),
			);
		}
		expected_files.insert(executable.file.clone());
		let executable_path = path.join(&executable.file);
		let metadata = fs::symlink_metadata(&executable_path).map_err(|source| {
			BundleError::InvalidBundle {
				path: path.to_path_buf(),
				reason: format!("could not inspect {}: {source}", executable.file),
			}
		})?;
		if !metadata.is_file()
			|| metadata.file_type().is_symlink()
			|| metadata.len() != executable.size
		{
			return invalid_bundle(path, &format!("invalid Dylint tool {}", executable.file));
		}
		let actual = sha256_file(&executable_path).map_err(|source| {
			BundleError::InvalidBundle {
				path: path.to_path_buf(),
				reason: format!("could not hash {}: {source}", executable.file),
			}
		})?;
		if !actual.eq_ignore_ascii_case(&executable.sha256) {
			return invalid_bundle(
				path,
				&format!("digest mismatch for Dylint tool {}", executable.file),
			);
		}
	}

	let actual_files = fs::read_dir(path)
		.map_err(|source| {
			BundleError::InvalidBundle {
				path: path.to_path_buf(),
				reason: format!("could not list Dylint tool files: {source}"),
			}
		})?
		.map(|entry| {
			entry
				.map(|entry| entry.file_name().to_string_lossy().into_owned())
				.map_err(|source| {
					BundleError::InvalidBundle {
						path: path.to_path_buf(),
						reason: format!("could not inspect Dylint tool entry: {source}"),
					}
				})
		})
		.collect::<Result<BTreeSet<_>, _>>()?;
	if actual_files != expected_files {
		return invalid_bundle(
			path,
			"Dylint tool bundle contains unexpected or missing files",
		);
	}

	set_executable_permissions(path, target)?;
	Ok(PreparedTools {
		bin_dir: path.to_path_buf(),
		cargo_dylint: path.join(tool_filename("cargo-dylint", target)),
	})
}

fn validate_bundle(
	path: &Path,
	catalog: &LintCatalog,
	version: &str,
	target: &str,
) -> Result<PreparedBundle, BundleError> {
	let manifest_path = path.join("manifest.json");
	let bytes = fs::read(&manifest_path).map_err(|source| {
		BundleError::ReadManifest {
			path: manifest_path.clone(),
			source,
		}
	})?;
	let manifest: BundleManifest = serde_json::from_slice(&bytes).map_err(|source| {
		BundleError::ParseManifest {
			path: manifest_path,
			source,
		}
	})?;
	let expected_toolchain = format!("{}-{target}", catalog.toolchain);
	if manifest.schema_version != BUNDLE_SCHEMA_VERSION
		|| manifest.version != version
		|| manifest.target != target
		|| manifest.toolchain != expected_toolchain
		|| manifest.dylint_version != catalog.dylint_version
	{
		return invalid_bundle(path, "bundle metadata does not match this CLI and host");
	}

	let names = manifest
		.libraries
		.iter()
		.map(|library| library.name.as_str())
		.collect::<Vec<_>>();
	let expected_names = catalog
		.libraries
		.iter()
		.map(String::as_str)
		.collect::<Vec<_>>();
	if names != expected_names {
		return invalid_bundle(
			path,
			"bundle libraries do not match the embedded lint catalog",
		);
	}

	let mut library_paths = Vec::with_capacity(manifest.libraries.len());
	let mut expected_files = BTreeSet::from(["manifest.json".to_owned()]);
	for library in manifest.libraries {
		let expected_file = library_filename(&library.name, &manifest.toolchain);
		if library.file != expected_file || !is_safe_component(&library.file) {
			return invalid_bundle(
				path,
				&format!("unexpected library filename {}", library.file),
			);
		}
		expected_files.insert(library.file.clone());
		let library_path = path.join(&library.file);
		let metadata = fs::symlink_metadata(&library_path).map_err(|source| {
			BundleError::InvalidBundle {
				path: path.to_path_buf(),
				reason: format!("could not inspect {}: {source}", library.file),
			}
		})?;
		if !metadata.is_file()
			|| metadata.file_type().is_symlink()
			|| metadata.len() != library.size
		{
			return invalid_bundle(path, &format!("invalid library file {}", library.file));
		}
		let actual = sha256_file(&library_path).map_err(|source| {
			BundleError::InvalidBundle {
				path: path.to_path_buf(),
				reason: format!("could not hash {}: {source}", library.file),
			}
		})?;
		if !actual.eq_ignore_ascii_case(&library.sha256) {
			return invalid_bundle(path, &format!("digest mismatch for {}", library.file));
		}
		library_paths.push(library_path);
	}

	let actual_files = fs::read_dir(path)
		.map_err(|source| {
			BundleError::InvalidBundle {
				path: path.to_path_buf(),
				reason: format!("could not list bundle files: {source}"),
			}
		})?
		.map(|entry| {
			entry
				.map(|entry| entry.file_name().to_string_lossy().into_owned())
				.map_err(|source| {
					BundleError::InvalidBundle {
						path: path.to_path_buf(),
						reason: format!("could not inspect bundle entry: {source}"),
					}
				})
		})
		.collect::<Result<BTreeSet<_>, _>>()?;
	if actual_files != expected_files {
		return invalid_bundle(path, "bundle contains unexpected or missing files");
	}

	Ok(PreparedBundle { library_paths })
}

fn invalid_bundle<T>(path: &Path, reason: &str) -> Result<T, BundleError> {
	Err(BundleError::InvalidBundle {
		path: path.to_path_buf(),
		reason: reason.to_owned(),
	})
}

fn library_filename(name: &str, toolchain: &str) -> String {
	if cfg!(target_os = "windows") {
		format!("{name}@{toolchain}.dll")
	} else if cfg!(target_os = "macos") {
		format!("lib{name}@{toolchain}.dylib")
	} else {
		format!("lib{name}@{toolchain}.so")
	}
}

fn is_safe_component(value: &str) -> bool {
	!value.is_empty()
		&& Path::new(value).components().count() == 1
		&& matches!(
			Path::new(value).components().next(),
			Some(Component::Normal(_))
		)
}

fn is_safe_release_component(value: &str) -> bool {
	!value.is_empty()
		&& value.chars().all(|character| {
			character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
		})
}

fn is_dynamic_library(file_name: &str) -> bool {
	matches!(
		Path::new(file_name)
			.extension()
			.and_then(|value| value.to_str()),
		Some("so" | "dylib" | "dll")
	)
}

fn is_tool_executable(file_name: &str) -> bool {
	TOOL_NAMES
		.iter()
		.any(|name| file_name == tool_filename(name, env!("PINA_BUILD_TARGET")))
}

fn tool_filename(name: &str, target: &str) -> String {
	if target.contains("windows") {
		format!("{name}.exe")
	} else {
		name.to_owned()
	}
}

#[cfg(unix)]
fn set_executable_permissions(path: &Path, target: &str) -> Result<(), BundleError> {
	for name in TOOL_NAMES {
		let executable = path.join(tool_filename(name, target));
		let mut permissions = fs::metadata(&executable)
			.map_err(|source| {
				BundleError::InvalidBundle {
					path: path.to_path_buf(),
					reason: format!(
						"could not inspect {} permissions: {source}",
						executable.display()
					),
				}
			})?
			.permissions();
		permissions.set_mode(0o755);
		fs::set_permissions(&executable, permissions).map_err(|source| {
			BundleError::InvalidBundle {
				path: path.to_path_buf(),
				reason: format!(
					"could not set {} executable: {source}",
					executable.display()
				),
			}
		})?;
	}
	Ok(())
}

#[cfg(windows)]
fn set_executable_permissions(_path: &Path, _target: &str) -> Result<(), BundleError> {
	Ok(())
}

fn sha256(bytes: &[u8]) -> String {
	hex_digest(Sha256::digest(bytes).as_ref())
}

fn sha256_file(path: &Path) -> Result<String, std::io::Error> {
	let mut file = File::open(path)?;
	let mut hasher = Sha256::new();
	let mut buffer = [0_u8; 16 * 1024];
	loop {
		let count = file.read(&mut buffer)?;
		if count == 0 {
			break;
		}
		hasher.update(&buffer[..count]);
	}
	Ok(hex_digest(hasher.finalize().as_ref()))
}

fn hex_digest(bytes: &[u8]) -> String {
	const HEX: &[u8; 16] = b"0123456789abcdef";
	let mut encoded = String::with_capacity(bytes.len() * 2);
	for byte in bytes {
		encoded.push(char::from(HEX[usize::from(byte >> 4)]));
		encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
	}
	encoded
}

#[cfg(test)]
mod tests {
	use std::net::TcpListener;
	use std::thread;

	use flate2::Compression;
	use flate2::write::GzEncoder;
	use serde_json::json;

	use super::*;
	const TEST_DYLINT_VERSION: &str = "6.0.4";

	fn test_catalog() -> LintCatalog {
		LintCatalog {
			schema_version: CATALOG_SCHEMA_VERSION,
			dylint_version: TEST_DYLINT_VERSION.to_owned(),
			toolchain: "nightly-test".to_owned(),
			tool_targets: vec![env!("PINA_BUILD_TARGET").to_owned()],
			lint_targets: vec![env!("PINA_BUILD_TARGET").to_owned()],
			libraries: vec!["secure_lint".to_owned()],
		}
	}

	fn archive(files: impl IntoIterator<Item = (String, Vec<u8>)>) -> Vec<u8> {
		let encoder = GzEncoder::new(Vec::new(), Compression::default());
		let mut archive = tar::Builder::new(encoder);
		for (name, contents) in files {
			let mut header = tar::Header::new_gnu();
			header.set_size(contents.len() as u64);
			header.set_mode(0o644);
			header.set_cksum();
			archive
				.append_data(&mut header, name, contents.as_slice())
				.expect("test archive entry should be created");
		}
		let encoder = archive.into_inner().expect("tar stream should finish");
		encoder.finish().expect("gzip stream should finish")
	}

	fn lint_archive(catalog: &LintCatalog, version: &str, target: &str) -> Vec<u8> {
		let toolchain = format!("{}-{target}", catalog.toolchain);
		let mut files = Vec::new();
		let libraries = catalog
			.libraries
			.iter()
			.map(|name| {
				let file = library_filename(name, &toolchain);
				let contents = format!("trusted native library {name}").into_bytes();
				files.push((file.clone(), contents.clone()));
				json!({
					"name": name,
					"file": file,
					"sha256": sha256(&contents),
					"size": contents.len(),
				})
			})
			.collect::<Vec<_>>();
		let manifest = json!({
			"schema_version": 1,
			"version": version,
			"target": target,
			"toolchain": toolchain,
			"dylint_version": catalog.dylint_version,
			"libraries": libraries,
		});
		files.push((
			"manifest.json".to_owned(),
			serde_json::to_vec(&manifest).expect("manifest should encode"),
		));
		archive(files)
	}

	fn tool_archive(version: &str, target: &str) -> Vec<u8> {
		let mut files = Vec::new();
		let executables = TOOL_NAMES
			.into_iter()
			.map(|name| {
				let file = tool_filename(name, target);
				let contents = format!("trusted executable {name}").into_bytes();
				files.push((file.clone(), contents.clone()));
				json!({
					"name": name,
					"file": file,
					"sha256": sha256(&contents),
					"size": contents.len(),
				})
			})
			.collect::<Vec<_>>();
		let manifest = json!({
			"schema_version": 1,
			"dylint_version": version,
			"target": target,
			"executables": executables,
		});
		files.push((
			"manifest.json".to_owned(),
			serde_json::to_vec(&manifest).expect("manifest should encode"),
		));
		archive(files)
	}

	fn serve_release(
		tag: &str,
		asset_name: &str,
		asset_bytes: Vec<u8>,
	) -> (String, String, thread::JoinHandle<()>) {
		let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
		let base = format!(
			"http://{}",
			listener
				.local_addr()
				.expect("server address should resolve")
		);
		let releases_api = format!("{base}/releases/tags");
		let release_downloads = format!("{base}/releases/download");
		let metadata = serde_json::to_vec(&json!({
			"tag_name": tag,
			"assets": [{
				"name": asset_name,
				"browser_download_url": format!("{release_downloads}/{tag}/{asset_name}"),
				"size": asset_bytes.len(),
				"digest": format!("sha256:{}", sha256(&asset_bytes)),
			}],
		}))
		.expect("release metadata should encode");
		let expected_paths = [
			format!("/releases/tags/{tag}"),
			format!("/releases/download/{tag}/{asset_name}"),
		];
		let handle = thread::spawn(move || {
			for (index, body) in [metadata, asset_bytes].into_iter().enumerate() {
				let (mut stream, _) = listener.accept().expect("request should arrive");
				let mut request = Vec::new();
				let mut buffer = [0_u8; 1024];
				while !request.windows(4).any(|window| window == b"\r\n\r\n") {
					let count = stream
						.read(&mut buffer)
						.expect("request should be readable");
					assert!(count > 0, "request ended before its headers");
					request.extend_from_slice(&buffer[..count]);
					assert!(request.len() <= 16 * 1024, "request headers were too large");
				}
				let first_line = String::from_utf8_lossy(&request)
					.lines()
					.next()
					.expect("request should have a first line")
					.to_owned();
				assert_eq!(
					first_line,
					format!("GET {} HTTP/1.1", expected_paths[index])
				);
				write!(
					stream,
					"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
					body.len()
				)
				.expect("response headers should be written");
				stream
					.write_all(&body)
					.expect("response body should be written");
			}
		});
		(releases_api, release_downloads, handle)
	}

	fn serve_once(body: Vec<u8>, declared_length: usize) -> (String, thread::JoinHandle<()>) {
		let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
		let url = format!(
			"http://{}",
			listener
				.local_addr()
				.expect("server address should resolve")
		);
		let handle = thread::spawn(move || {
			let (mut stream, _) = listener.accept().expect("request should arrive");
			let mut request = [0_u8; 16 * 1024];
			assert!(stream.read(&mut request).expect("request should be read") > 0);
			write!(
				stream,
				"HTTP/1.1 200 OK\r\nContent-Length: {declared_length}\r\nConnection: close\r\n\r\n"
			)
			.expect("response headers should be written");
			stream
				.write_all(&body)
				.expect("response body should be written");
		});
		(url, handle)
	}

	fn extract_fixture(bytes: &[u8], contents: ArchiveContents) -> tempfile::TempDir {
		let directory = tempfile::tempdir().expect("temporary bundle should exist");
		unpack_archive(bytes, directory.path(), "fixture.tar.gz", contents)
			.expect("fixture should extract");
		directory
	}

	fn rewrite_manifest(path: &Path, mutate: impl FnOnce(&mut serde_json::Value)) {
		let manifest_path = path.join("manifest.json");
		let mut manifest: serde_json::Value = serde_json::from_slice(
			&fs::read(&manifest_path).expect("fixture manifest should be readable"),
		)
		.expect("fixture manifest should parse");
		mutate(&mut manifest);
		fs::write(
			manifest_path,
			serde_json::to_vec(&manifest).expect("fixture manifest should encode"),
		)
		.expect("fixture manifest should be replaced");
	}

	fn lint_fixture() -> (tempfile::TempDir, LintCatalog) {
		let catalog = test_catalog();
		let directory = extract_fixture(
			&lint_archive(&catalog, "1.2.3", env!("PINA_BUILD_TARGET")),
			ArchiveContents::Lints,
		);
		(directory, catalog)
	}

	fn tool_fixture() -> tempfile::TempDir {
		extract_fixture(
			&tool_archive(TEST_DYLINT_VERSION, env!("PINA_BUILD_TARGET")),
			ArchiveContents::Tools,
		)
	}

	#[test]
	fn embedded_catalog_is_valid_and_sorted() {
		let catalog = lint_catalog().expect("embedded catalog should be valid");
		let mut sorted = catalog.libraries.clone();
		sorted.sort();
		assert_eq!(catalog.libraries, sorted);
	}

	#[test]
	fn managed_bundle_rejects_a_rust_host_without_dynamic_libraries() {
		let cargo_home = tempfile::tempdir().expect("temporary Cargo home should exist");

		assert!(matches!(
			prepare_bundle_from(
				cargo_home.path(),
				"x86_64-unknown-linux-musl",
				"unused",
				"unused",
			),
			Err(BundleError::UnsupportedLintHost { host })
				if host == "x86_64-unknown-linux-musl"
		));
	}

	#[test]
	fn catalog_validation_rejects_every_unsafe_contract_shape() {
		let invalid_catalogs = [
			LintCatalog {
				schema_version: 1,
				..test_catalog()
			},
			LintCatalog {
				dylint_version: "not-semver".to_owned(),
				..test_catalog()
			},
			LintCatalog {
				toolchain: String::new(),
				..test_catalog()
			},
			LintCatalog {
				tool_targets: vec!["duplicate".to_owned(), "duplicate".to_owned()],
				..test_catalog()
			},
			LintCatalog {
				tool_targets: vec!["../unsafe".to_owned(), env!("PINA_BUILD_TARGET").to_owned()],
				..test_catalog()
			},
			LintCatalog {
				tool_targets: vec!["safe-but-not-this-host".to_owned()],
				lint_targets: vec!["safe-but-not-this-host".to_owned()],
				..test_catalog()
			},
			LintCatalog {
				lint_targets: vec!["duplicate".to_owned(), "duplicate".to_owned()],
				..test_catalog()
			},
			LintCatalog {
				lint_targets: vec!["../unsafe".to_owned()],
				..test_catalog()
			},
			LintCatalog {
				lint_targets: vec!["safe-but-missing-tools".to_owned()],
				..test_catalog()
			},
			LintCatalog {
				libraries: vec!["duplicate".to_owned(), "duplicate".to_owned()],
				..test_catalog()
			},
			LintCatalog {
				libraries: vec!["../unsafe".to_owned()],
				..test_catalog()
			},
		];
		for catalog in invalid_catalogs {
			assert!(matches!(
				validate_catalog(&catalog),
				Err(BundleError::InvalidCatalog { .. })
			));
		}
		assert!(!is_safe_component(""));
		assert!(!is_safe_release_component(""));
		assert!(is_safe_release_component("aarch64-unknown-linux-gnu"));
		assert_eq!(
			tool_filename("cargo-dylint", "x86_64-pc-windows-msvc"),
			"cargo-dylint.exe"
		);
	}

	#[test]
	fn asset_selection_rejects_redirectable_release_inputs() {
		let release = GithubRelease {
			tag_name: "v1.2.3".to_owned(),
			assets: vec![GithubAsset {
				name: "pina-lints-test-v1.2.3.tar.gz".to_owned(),
				browser_download_url: "https://example.com/native-code.tar.gz".to_owned(),
				size: 1,
				digest: Some("sha256:00".to_owned()),
			}],
		};
		assert!(matches!(
			select_asset(
				&release,
				PINA_RELEASE_DOWNLOADS,
				"v1.2.3",
				"pina-lints-test-v1.2.3.tar.gz",
			),
			Err(BundleError::UnexpectedDownloadUrl { .. })
		));
		assert!(matches!(
			select_asset(
				&release,
				PINA_RELEASE_DOWNLOADS,
				"v9.9.9",
				"pina-lints-test-v1.2.3.tar.gz",
			),
			Err(BundleError::UnexpectedReleaseTag { .. })
		));
		assert!(matches!(
			select_asset(
				&GithubRelease {
					tag_name: "v1.2.3".to_owned(),
					assets: Vec::new(),
				},
				PINA_RELEASE_DOWNLOADS,
				"v1.2.3",
				"missing.tar.gz",
			),
			Err(BundleError::MissingAsset { .. })
		));
	}

	#[test]
	fn downloads_verifies_caches_and_reuses_native_release_assets() {
		let cargo_home = tempfile::tempdir().expect("temporary Cargo home should exist");
		let catalog = lint_catalog().expect("embedded catalog should be valid");
		let target = env!("PINA_BUILD_TARGET");
		let version = env!("CARGO_PKG_VERSION");

		let lint_root = cargo_home
			.path()
			.join("pina/lints")
			.join(format!("v{version}"))
			.join(target);
		fs::create_dir_all(lint_root.join("bundle")).expect("stale lint bundle should be created");
		fs::write(lint_root.join("bundle/stale"), b"stale")
			.expect("stale lint entry should be written");
		let lint_tag = format!("v{version}");
		let lint_asset = format!("pina-lints-{target}-{lint_tag}.tar.gz");
		let (api, downloads, server) = serve_release(
			&lint_tag,
			&lint_asset,
			lint_archive(&catalog, version, target),
		);
		let bundle = prepare_bundle_from(cargo_home.path(), target, &api, &downloads)
			.expect("lint bundle should download");
		server.join().expect("lint release server should finish");
		assert_eq!(bundle.library_paths.len(), catalog.libraries.len());
		assert!(bundle.library_paths.iter().all(|path| path.is_file()));
		assert!(!lint_root.join("bundle/stale").exists());
		prepare_bundle(cargo_home.path(), target).expect("cached lint bundle should be reused");

		let tool_root = cargo_home
			.path()
			.join(format!("pina/tools/dylint-v{}", catalog.dylint_version))
			.join(target);
		fs::create_dir_all(tool_root.join("bundle")).expect("stale tool bundle should be created");
		fs::write(tool_root.join("bundle/stale"), b"stale")
			.expect("stale tool entry should be written");
		let tool_tag = format!("dylint-v{}", catalog.dylint_version);
		let tool_asset = format!(
			"pina-dylint-tools-{target}-v{}.tar.gz",
			catalog.dylint_version
		);
		let (api, downloads, server) = serve_release(
			&tool_tag,
			&tool_asset,
			tool_archive(&catalog.dylint_version, target),
		);
		let tools = prepare_dylint_tools_from(cargo_home.path(), &api, &downloads)
			.expect("Dylint tools should download");
		server.join().expect("tool release server should finish");
		assert!(tools.bin_dir.is_dir());
		assert!(tools.cargo_dylint.is_file());
		assert!(!tool_root.join("bundle/stale").exists());
		prepare_dylint_tools(cargo_home.path()).expect("cached Dylint tools should be reused");
	}

	#[test]
	fn managed_cache_setup_reports_directory_and_lock_failures() {
		let temporary = tempfile::tempdir().expect("temporary root should exist");
		let blocked_home = temporary.path().join("blocked-home");
		fs::write(&blocked_home, b"not a directory").expect("blocked home should be written");
		assert!(matches!(
			prepare_bundle_from(&blocked_home, env!("PINA_BUILD_TARGET"), "unused", "unused",),
			Err(BundleError::CreateDirectory { .. })
		));
		assert!(matches!(
			prepare_dylint_tools_from(&blocked_home, "unused", "unused"),
			Err(BundleError::CreateDirectory { .. })
		));

		let cargo_home = tempfile::tempdir().expect("temporary Cargo home should exist");
		let lint_lock = cargo_home
			.path()
			.join("pina/lints")
			.join(format!("v{}", env!("CARGO_PKG_VERSION")))
			.join(env!("PINA_BUILD_TARGET"))
			.join("bundle.lock");
		fs::create_dir_all(&lint_lock).expect("directory lock should be created");
		assert!(matches!(
			prepare_bundle_from(
				cargo_home.path(),
				env!("PINA_BUILD_TARGET"),
				"unused",
				"unused",
			),
			Err(BundleError::OpenLock { .. })
		));

		let tool_lock = cargo_home
			.path()
			.join(format!("pina/tools/dylint-v{TEST_DYLINT_VERSION}"))
			.join(env!("PINA_BUILD_TARGET"))
			.join("bundle.lock");
		fs::create_dir_all(&tool_lock).expect("directory lock should be created");
		assert!(matches!(
			prepare_dylint_tools_from(cargo_home.path(), "unused", "unused"),
			Err(BundleError::OpenLock { .. })
		));

		let existing = tempfile::tempdir().expect("temporary directory should exist");
		assert!(matches!(
			create_directory(existing.path()),
			Err(BundleError::CreateDirectory { .. })
		));

		let replacement = tempfile::tempdir().expect("temporary directory should exist");
		assert!(matches!(
			replace_cached_directory(
				&replacement.path().join("missing-source"),
				&replacement.path().join("destination"),
			),
			Err(BundleError::CacheBundle { .. })
		));
	}

	#[test]
	fn managed_downloads_reject_non_directory_cache_entries() {
		let cargo_home = tempfile::tempdir().expect("temporary Cargo home should exist");
		let catalog = lint_catalog().expect("embedded catalog should be valid");
		let target = env!("PINA_BUILD_TARGET");
		let version = env!("CARGO_PKG_VERSION");
		let lint_root = cargo_home
			.path()
			.join("pina/lints")
			.join(format!("v{version}"))
			.join(target);
		fs::create_dir_all(&lint_root).expect("lint cache root should be created");
		fs::write(lint_root.join("bundle"), b"not a directory")
			.expect("invalid lint cache should be written");
		let tag = format!("v{version}");
		let asset = format!("pina-lints-{target}-{tag}.tar.gz");
		let (api, downloads, server) =
			serve_release(&tag, &asset, lint_archive(&catalog, version, target));
		assert!(matches!(
			prepare_bundle_from(cargo_home.path(), target, &api, &downloads),
			Err(BundleError::InvalidBundle { .. })
		));
		server.join().expect("lint release server should finish");

		let tool_root = cargo_home
			.path()
			.join(format!("pina/tools/dylint-v{}", catalog.dylint_version))
			.join(target);
		fs::create_dir_all(&tool_root).expect("tool cache root should be created");
		fs::write(tool_root.join("bundle"), b"not a directory")
			.expect("invalid tool cache should be written");
		let tag = format!("dylint-v{}", catalog.dylint_version);
		let asset = format!(
			"pina-dylint-tools-{target}-v{}.tar.gz",
			catalog.dylint_version
		);
		let (api, downloads, server) =
			serve_release(&tag, &asset, tool_archive(&catalog.dylint_version, target));
		assert!(matches!(
			prepare_dylint_tools_from(cargo_home.path(), &api, &downloads),
			Err(BundleError::InvalidBundle { .. })
		));
		server.join().expect("tool release server should finish");
	}

	#[cfg(unix)]
	#[test]
	fn managed_downloads_report_temporary_directory_failures() {
		let catalog = lint_catalog().expect("embedded catalog should be valid");
		let target = env!("PINA_BUILD_TARGET");
		let version = env!("CARGO_PKG_VERSION");
		let cargo_home = tempfile::tempdir().expect("temporary Cargo home should exist");
		let lint_root = cargo_home
			.path()
			.join("pina/lints")
			.join(format!("v{version}"))
			.join(target);
		fs::create_dir_all(&lint_root).expect("lint root should be created");
		fs::write(lint_root.join("bundle.lock"), b"").expect("lint lock should be written");
		fs::set_permissions(&lint_root, fs::Permissions::from_mode(0o555))
			.expect("lint root should become read-only");
		let tag = format!("v{version}");
		let asset = format!("pina-lints-{target}-{tag}.tar.gz");
		let (api, downloads, server) =
			serve_release(&tag, &asset, lint_archive(&catalog, version, target));
		assert!(matches!(
			prepare_bundle_from(cargo_home.path(), target, &api, &downloads),
			Err(BundleError::CreateTemporaryBundle { .. })
		));
		fs::set_permissions(&lint_root, fs::Permissions::from_mode(0o755))
			.expect("lint root permissions should be restored");
		server.join().expect("lint release server should finish");

		let cargo_home = tempfile::tempdir().expect("temporary Cargo home should exist");
		let tool_root = cargo_home
			.path()
			.join(format!("pina/tools/dylint-v{}", catalog.dylint_version))
			.join(target);
		fs::create_dir_all(&tool_root).expect("tool root should be created");
		fs::write(tool_root.join("bundle.lock"), b"").expect("tool lock should be written");
		fs::set_permissions(&tool_root, fs::Permissions::from_mode(0o555))
			.expect("tool root should become read-only");
		let tag = format!("dylint-v{}", catalog.dylint_version);
		let asset = format!(
			"pina-dylint-tools-{target}-v{}.tar.gz",
			catalog.dylint_version
		);
		let (api, downloads, server) =
			serve_release(&tag, &asset, tool_archive(&catalog.dylint_version, target));
		assert!(matches!(
			prepare_dylint_tools_from(cargo_home.path(), &api, &downloads),
			Err(BundleError::CreateTemporaryBundle { .. })
		));
		fs::set_permissions(&tool_root, fs::Permissions::from_mode(0o755))
			.expect("tool root permissions should be restored");
		server.join().expect("tool release server should finish");
	}

	#[test]
	fn archive_digest_and_size_are_both_required() {
		let bytes = b"verified bundle";
		let asset = GithubAsset {
			name: "bundle.tar.gz".to_owned(),
			browser_download_url: String::new(),
			size: bytes.len() as u64,
			digest: Some(format!("sha256:{}", sha256(bytes))),
		};
		verify_archive(&asset, bytes).expect("matching archive should verify");

		let wrong_size = GithubAsset { size: 1, ..asset };
		assert!(matches!(
			verify_archive(&wrong_size, bytes),
			Err(BundleError::UnexpectedAssetSize { .. })
		));

		let missing_digest = GithubAsset {
			digest: None,
			..wrong_size
		};
		let missing_digest = GithubAsset {
			size: bytes.len() as u64,
			..missing_digest
		};
		assert!(matches!(
			verify_archive(&missing_digest, bytes),
			Err(BundleError::MissingAssetDigest { .. })
		));
		let unsupported = GithubAsset {
			digest: Some("sha512:00".to_owned()),
			..missing_digest
		};
		assert!(matches!(
			verify_archive(&unsupported, bytes),
			Err(BundleError::UnsupportedAssetDigest { .. })
		));
		let mismatched = GithubAsset {
			digest: Some("sha256:00".to_owned()),
			..unsupported
		};
		assert!(matches!(
			verify_archive(&mismatched, bytes),
			Err(BundleError::AssetDigestMismatch { .. })
		));
	}

	#[test]
	fn http_failures_preserve_release_and_asset_context() {
		assert!(matches!(
			fetch_release("http://127.0.0.1:1/releases/tags", "v1.2.3"),
			Err(BundleError::Http { resource, .. }) if resource.contains("v1.2.3")
		));
		let asset = GithubAsset {
			name: "native.tar.gz".to_owned(),
			browser_download_url: "http://127.0.0.1:1/native.tar.gz".to_owned(),
			size: 0,
			digest: None,
		};
		assert!(matches!(
			fetch_archive(&asset),
			Err(BundleError::Http { resource, .. }) if resource == "native.tar.gz"
		));

		let (url, server) = serve_once(b"{".to_vec(), 1);
		assert!(matches!(
			fetch_release(&url, "malformed"),
			Err(BundleError::Http { resource, .. }) if resource.contains("malformed")
		));
		server
			.join()
			.expect("malformed release server should finish");

		let (url, server) = serve_once(b"short".to_vec(), 100);
		let truncated = GithubAsset {
			name: "truncated.tar.gz".to_owned(),
			browser_download_url: url,
			size: 100,
			digest: None,
		};
		assert!(matches!(
			fetch_archive(&truncated),
			Err(BundleError::Http { resource, .. }) if resource == "truncated.tar.gz"
		));
		server.join().expect("truncated asset server should finish");
	}

	#[test]
	fn extraction_rejects_nested_archive_paths() {
		let encoder = GzEncoder::new(Vec::new(), Compression::default());
		let mut archive = tar::Builder::new(encoder);
		let contents = b"{}";
		let mut header = tar::Header::new_gnu();
		header.set_size(contents.len() as u64);
		header.set_mode(0o644);
		header.set_cksum();
		archive
			.append_data(&mut header, "nested/manifest.json", contents.as_slice())
			.expect("test archive should be created");
		let encoder = archive.into_inner().expect("tar stream should finish");
		let bytes = encoder.finish().expect("gzip stream should finish");
		let destination = tempfile::tempdir().expect("temporary directory should exist");
		assert!(matches!(
			unpack_archive(
				&bytes,
				destination.path(),
				"bundle.tar.gz",
				ArchiveContents::Lints,
			),
			Err(BundleError::UnsafeArchiveEntry { .. })
		));
	}

	#[test]
	fn extraction_rejects_malformed_disallowed_duplicate_and_oversized_entries() {
		let destination = tempfile::tempdir().expect("temporary directory should exist");
		assert!(matches!(
			unpack_archive(
				b"not a gzip stream",
				destination.path(),
				"malformed.tar.gz",
				ArchiveContents::Lints,
			),
			Err(BundleError::Unpack { .. })
		));

		let disallowed = archive([("source.rs".to_owned(), b"unsafe".to_vec())]);
		assert!(matches!(
			unpack_archive(
				&disallowed,
				destination.path(),
				"disallowed.tar.gz",
				ArchiveContents::Lints,
			),
			Err(BundleError::UnsafeArchiveEntry { .. })
		));

		let mut absolute_header = tar::Header::new_gnu();
		absolute_header.set_size(0);
		absolute_header.set_mode(0o644);
		absolute_header.as_mut_bytes()[..15].copy_from_slice(b"/manifest.json\0");
		absolute_header.set_cksum();
		let mut absolute_encoder = GzEncoder::new(Vec::new(), Compression::default());
		absolute_encoder
			.write_all(absolute_header.as_bytes())
			.expect("absolute header should be written");
		absolute_encoder
			.write_all(&[0_u8; 1024])
			.expect("tar trailer should be written");
		let absolute = absolute_encoder
			.finish()
			.expect("gzip stream should finish");
		let absolute_destination = tempfile::tempdir().expect("temporary directory should exist");
		assert!(matches!(
			unpack_archive(
				&absolute,
				absolute_destination.path(),
				"absolute.tar.gz",
				ArchiveContents::Lints,
			),
			Err(BundleError::UnsafeArchiveEntry { .. })
		));

		let duplicate = archive([
			("manifest.json".to_owned(), b"{}".to_vec()),
			("manifest.json".to_owned(), b"{}".to_vec()),
		]);
		let duplicate_destination = tempfile::tempdir().expect("temporary directory should exist");
		assert!(matches!(
			unpack_archive(
				&duplicate,
				duplicate_destination.path(),
				"duplicate.tar.gz",
				ArchiveContents::Lints,
			),
			Err(BundleError::Unpack { .. })
		));

		let mut header = tar::Header::new_gnu();
		header
			.set_path("manifest.json")
			.expect("safe path should be accepted");
		header.set_size(MAX_EXTRACTED_BYTES + 1);
		header.set_mode(0o644);
		header.set_cksum();
		let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
		encoder
			.write_all(header.as_bytes())
			.expect("oversized header should be written");
		encoder
			.write_all(&[0_u8; 1024])
			.expect("tar trailer should be written");
		let oversized = encoder.finish().expect("gzip stream should finish");
		let oversized_destination = tempfile::tempdir().expect("temporary directory should exist");
		assert!(matches!(
			unpack_archive(
				&oversized,
				oversized_destination.path(),
				"oversized.tar.gz",
				ArchiveContents::Lints,
			),
			Err(BundleError::UnsafeArchiveEntry { entry, .. }) if entry.contains("size limit")
		));
	}

	#[test]
	fn bundle_validation_detects_tampered_library_bytes() {
		let catalog = test_catalog();
		let target = env!("PINA_BUILD_TARGET");
		let toolchain = format!("{}-{target}", catalog.toolchain);
		let file = library_filename("secure_lint", &toolchain);
		let directory = tempfile::tempdir().expect("temporary directory should exist");
		let contents = b"trusted native library";
		fs::write(directory.path().join(&file), contents).expect("library should be written");
		let manifest = json!({
			"schema_version": 1,
			"version": "1.2.3",
			"target": target,
			"toolchain": toolchain,
			"dylint_version": TEST_DYLINT_VERSION,
			"libraries": [{
				"name": "secure_lint",
				"file": file,
				"sha256": sha256(contents),
				"size": contents.len(),
			}],
		});
		fs::write(
			directory.path().join("manifest.json"),
			serde_json::to_vec(&manifest).expect("manifest should encode"),
		)
		.expect("manifest should be written");
		validate_bundle(directory.path(), &catalog, "1.2.3", target)
			.expect("untampered bundle should validate");

		fs::write(directory.path().join(&file), b"altered native library")
			.expect("library should be replaced");
		assert!(matches!(
			validate_bundle(directory.path(), &catalog, "1.2.3", target),
			Err(BundleError::InvalidBundle { .. })
		));
	}

	#[test]
	fn bundle_validation_rejects_malformed_metadata_files_and_layouts() {
		let missing = tempfile::tempdir().expect("temporary directory should exist");
		assert!(matches!(
			validate_bundle(
				missing.path(),
				&test_catalog(),
				"1.2.3",
				env!("PINA_BUILD_TARGET"),
			),
			Err(BundleError::ReadManifest { .. })
		));

		let malformed = tempfile::tempdir().expect("temporary directory should exist");
		fs::write(malformed.path().join("manifest.json"), b"{")
			.expect("malformed manifest should be written");
		assert!(matches!(
			validate_bundle(
				malformed.path(),
				&test_catalog(),
				"1.2.3",
				env!("PINA_BUILD_TARGET"),
			),
			Err(BundleError::ParseManifest { .. })
		));

		let (metadata, catalog) = lint_fixture();
		rewrite_manifest(metadata.path(), |manifest| {
			manifest["schema_version"] = json!(2);
		});
		assert!(matches!(
			validate_bundle(
				metadata.path(),
				&catalog,
				"1.2.3",
				env!("PINA_BUILD_TARGET"),
			),
			Err(BundleError::InvalidBundle { .. })
		));

		let (names, catalog) = lint_fixture();
		rewrite_manifest(names.path(), |manifest| {
			manifest["libraries"] = json!([]);
		});
		assert!(matches!(
			validate_bundle(names.path(), &catalog, "1.2.3", env!("PINA_BUILD_TARGET"),),
			Err(BundleError::InvalidBundle { .. })
		));

		let (filename, catalog) = lint_fixture();
		rewrite_manifest(filename.path(), |manifest| {
			manifest["libraries"][0]["file"] = json!("unexpected.so");
		});
		assert!(matches!(
			validate_bundle(
				filename.path(),
				&catalog,
				"1.2.3",
				env!("PINA_BUILD_TARGET"),
			),
			Err(BundleError::InvalidBundle { .. })
		));

		let (missing_file, catalog) = lint_fixture();
		let expected_file = library_filename(
			"secure_lint",
			&format!("{}-{}", catalog.toolchain, env!("PINA_BUILD_TARGET")),
		);
		fs::remove_file(missing_file.path().join(&expected_file))
			.expect("fixture library should be removed");
		assert!(matches!(
			validate_bundle(
				missing_file.path(),
				&catalog,
				"1.2.3",
				env!("PINA_BUILD_TARGET"),
			),
			Err(BundleError::InvalidBundle { .. })
		));

		let (wrong_size, catalog) = lint_fixture();
		rewrite_manifest(wrong_size.path(), |manifest| {
			manifest["libraries"][0]["size"] = json!(1);
		});
		assert!(matches!(
			validate_bundle(
				wrong_size.path(),
				&catalog,
				"1.2.3",
				env!("PINA_BUILD_TARGET"),
			),
			Err(BundleError::InvalidBundle { .. })
		));

		let (extra, catalog) = lint_fixture();
		fs::write(extra.path().join("unexpected"), b"unexpected")
			.expect("unexpected file should be written");
		assert!(matches!(
			validate_bundle(extra.path(), &catalog, "1.2.3", env!("PINA_BUILD_TARGET"),),
			Err(BundleError::InvalidBundle { .. })
		));
	}

	#[test]
	fn tool_validation_requires_exact_version_target_and_digests() {
		let target = env!("PINA_BUILD_TARGET");
		let directory = tempfile::tempdir().expect("temporary directory should exist");
		let executables = TOOL_NAMES
			.into_iter()
			.map(|name| {
				let file = tool_filename(name, target);
				let contents = format!("trusted executable {name}");
				fs::write(directory.path().join(&file), contents.as_bytes())
					.expect("tool should be written");
				json!({
					"name": name,
					"file": file,
					"sha256": sha256(contents.as_bytes()),
					"size": contents.len(),
				})
			})
			.collect::<Vec<_>>();
		let manifest = json!({
			"schema_version": 1,
			"dylint_version": TEST_DYLINT_VERSION,
			"target": target,
			"executables": executables,
		});
		fs::write(
			directory.path().join("manifest.json"),
			serde_json::to_vec(&manifest).expect("manifest should encode"),
		)
		.expect("manifest should be written");
		let tools = validate_tool_bundle(directory.path(), TEST_DYLINT_VERSION, target)
			.expect("untampered tools should validate");
		assert_eq!(
			tools.cargo_dylint,
			directory.path().join(tool_filename("cargo-dylint", target))
		);

		fs::write(&tools.cargo_dylint, b"altered executable cargo-dylint")
			.expect("tool should be replaced");
		assert!(matches!(
			validate_tool_bundle(directory.path(), TEST_DYLINT_VERSION, target),
			Err(BundleError::InvalidBundle { .. })
		));
	}

	#[test]
	fn tool_validation_rejects_malformed_metadata_files_and_layouts() {
		let target = env!("PINA_BUILD_TARGET");
		let missing = tempfile::tempdir().expect("temporary directory should exist");
		assert!(matches!(
			validate_tool_bundle(missing.path(), TEST_DYLINT_VERSION, target),
			Err(BundleError::ReadManifest { .. })
		));

		let malformed = tempfile::tempdir().expect("temporary directory should exist");
		fs::write(malformed.path().join("manifest.json"), b"{")
			.expect("malformed manifest should be written");
		assert!(matches!(
			validate_tool_bundle(malformed.path(), TEST_DYLINT_VERSION, target),
			Err(BundleError::ParseManifest { .. })
		));

		let metadata = tool_fixture();
		rewrite_manifest(metadata.path(), |manifest| {
			manifest["target"] = json!("wrong-target");
		});
		assert!(matches!(
			validate_tool_bundle(metadata.path(), TEST_DYLINT_VERSION, target),
			Err(BundleError::InvalidBundle { .. })
		));

		let names = tool_fixture();
		rewrite_manifest(names.path(), |manifest| {
			manifest["executables"] = json!([]);
		});
		assert!(matches!(
			validate_tool_bundle(names.path(), TEST_DYLINT_VERSION, target),
			Err(BundleError::InvalidBundle { .. })
		));

		let filename = tool_fixture();
		rewrite_manifest(filename.path(), |manifest| {
			manifest["executables"][0]["file"] = json!("unexpected");
		});
		assert!(matches!(
			validate_tool_bundle(filename.path(), TEST_DYLINT_VERSION, target),
			Err(BundleError::InvalidBundle { .. })
		));

		let missing_file = tool_fixture();
		fs::remove_file(
			missing_file
				.path()
				.join(tool_filename("cargo-dylint", target)),
		)
		.expect("fixture tool should be removed");
		assert!(matches!(
			validate_tool_bundle(missing_file.path(), TEST_DYLINT_VERSION, target),
			Err(BundleError::InvalidBundle { .. })
		));

		let wrong_size = tool_fixture();
		rewrite_manifest(wrong_size.path(), |manifest| {
			manifest["executables"][0]["size"] = json!(1);
		});
		assert!(matches!(
			validate_tool_bundle(wrong_size.path(), TEST_DYLINT_VERSION, target),
			Err(BundleError::InvalidBundle { .. })
		));

		let extra = tool_fixture();
		fs::write(extra.path().join("unexpected"), b"unexpected")
			.expect("unexpected file should be written");
		assert!(matches!(
			validate_tool_bundle(extra.path(), TEST_DYLINT_VERSION, target),
			Err(BundleError::InvalidBundle { .. })
		));
	}

	#[cfg(unix)]
	#[test]
	fn validation_propagates_file_and_permission_failures() {
		let target = env!("PINA_BUILD_TARGET");
		let (lint_hash, catalog) = lint_fixture();
		let lint_file = lint_hash.path().join(library_filename(
			"secure_lint",
			&format!("{}-{target}", catalog.toolchain),
		));
		fs::set_permissions(&lint_file, fs::Permissions::from_mode(0o000))
			.expect("lint file should become unreadable");
		assert!(matches!(
			validate_bundle(lint_hash.path(), &catalog, "1.2.3", target),
			Err(BundleError::InvalidBundle { .. })
		));
		fs::set_permissions(&lint_file, fs::Permissions::from_mode(0o644))
			.expect("lint file permissions should be restored");

		let tool_hash = tool_fixture();
		let tool_file = tool_hash.path().join(tool_filename("cargo-dylint", target));
		fs::set_permissions(&tool_file, fs::Permissions::from_mode(0o000))
			.expect("tool file should become unreadable");
		assert!(matches!(
			validate_tool_bundle(tool_hash.path(), TEST_DYLINT_VERSION, target),
			Err(BundleError::InvalidBundle { .. })
		));
		fs::set_permissions(&tool_file, fs::Permissions::from_mode(0o644))
			.expect("tool file permissions should be restored");

		let missing_tools = tempfile::tempdir().expect("temporary directory should exist");
		assert!(matches!(
			set_executable_permissions(missing_tools.path(), target),
			Err(BundleError::InvalidBundle { .. })
		));
		assert!(is_safe_component("manifest.json"));
		assert!(is_dynamic_library("lint.so"));
		assert!(is_dynamic_library("lint.dylib"));
		assert!(is_dynamic_library("lint.dll"));
		assert!(!is_dynamic_library("lint.rs"));
	}
}
