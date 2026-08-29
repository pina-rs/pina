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
const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_EXTRACTED_BYTES: u64 = 1024 * 1024 * 1024;
const PINA_RELEASES_API: &str = "https://api.github.com/repos/pina-rs/pina/releases/tags";
const PINA_RELEASE_DOWNLOADS: &str = "https://github.com/pina-rs/pina/releases/download";
const TOOL_NAMES: [&str; 2] = ["cargo-dylint", "dylint-link"];

/// Metadata compiled into the CLI and consumed by the release builder.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LintCatalog {
	pub(crate) schema_version: u32,
	pub(crate) dylint_version: String,
	pub(crate) toolchain: String,
	pub(crate) targets: Vec<String>,
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

/// Download once, then validate and reuse the current CLI's native bundle.
pub(crate) fn prepare_bundle(cargo_home: &Path) -> Result<PreparedBundle, BundleError> {
	let catalog = lint_catalog()?;
	let version = env!("CARGO_PKG_VERSION");
	let target = env!("PINA_BUILD_TARGET");
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
	let release = fetch_release(&tag)?;
	let asset = select_asset(&release, &tag, &asset_name)?;
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
	fs::create_dir(&extracted).map_err(|source| {
		BundleError::CreateDirectory {
			path: extracted.clone(),
			source,
		}
	})?;
	unpack_archive(&archive, &extracted, &asset.name, ArchiveContents::Lints)?;
	let bundle = validate_bundle(&extracted, &catalog, version, target)?;

	if fs::symlink_metadata(&bundle_path).is_ok() {
		let metadata = fs::symlink_metadata(&bundle_path).map_err(|source| {
			BundleError::ReplaceCachedBundle {
				path: bundle_path.clone(),
				source,
			}
		})?;
		if metadata.file_type().is_symlink() || !metadata.is_dir() {
			return Err(BundleError::InvalidBundle {
				path: bundle_path,
				reason: "the cache entry is not a regular directory".to_owned(),
			});
		}
		fs::remove_dir_all(&bundle_path).map_err(|source| {
			BundleError::ReplaceCachedBundle {
				path: bundle_path.clone(),
				source,
			}
		})?;
	}
	fs::rename(&extracted, &bundle_path).map_err(|source| {
		BundleError::CacheBundle {
			path: bundle_path.clone(),
			source,
		}
	})?;

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
	let release = fetch_release(&tag)?;
	let asset = select_asset(&release, &tag, &asset_name)?;
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
	fs::create_dir(&extracted).map_err(|source| {
		BundleError::CreateDirectory {
			path: extracted.clone(),
			source,
		}
	})?;
	unpack_archive(&archive, &extracted, &asset.name, ArchiveContents::Tools)?;
	validate_tool_bundle(&extracted, &version, target)?;

	if fs::symlink_metadata(&bundle_path).is_ok() {
		let metadata = fs::symlink_metadata(&bundle_path).map_err(|source| {
			BundleError::ReplaceCachedBundle {
				path: bundle_path.clone(),
				source,
			}
		})?;
		if metadata.file_type().is_symlink() || !metadata.is_dir() {
			return Err(BundleError::InvalidBundle {
				path: bundle_path,
				reason: "the cache entry is not a regular directory".to_owned(),
			});
		}
		fs::remove_dir_all(&bundle_path).map_err(|source| {
			BundleError::ReplaceCachedBundle {
				path: bundle_path.clone(),
				source,
			}
		})?;
	}
	fs::rename(&extracted, &bundle_path).map_err(|source| {
		BundleError::CacheBundle {
			path: bundle_path.clone(),
			source,
		}
	})?;
	validate_tool_bundle(&bundle_path, &version, target)
}

fn validate_catalog(catalog: &LintCatalog) -> Result<(), BundleError> {
	if catalog.schema_version != BUNDLE_SCHEMA_VERSION {
		return Err(BundleError::InvalidCatalog {
			reason: format!("schema version {} is not supported", catalog.schema_version),
		});
	}
	if semver::Version::parse(&catalog.dylint_version).is_err() {
		return Err(BundleError::InvalidCatalog {
			reason: "Dylint version must use semantic versioning".to_owned(),
		});
	}
	if catalog.toolchain.is_empty() || catalog.targets.is_empty() || catalog.libraries.is_empty() {
		return Err(BundleError::InvalidCatalog {
			reason: "toolchain, targets, and libraries must not be empty".to_owned(),
		});
	}
	let unique_targets = catalog.targets.iter().collect::<BTreeSet<_>>();
	if unique_targets.len() != catalog.targets.len()
		|| catalog
			.targets
			.iter()
			.any(|target| !is_safe_release_component(target))
		|| !catalog
			.targets
			.iter()
			.any(|target| target == env!("PINA_BUILD_TARGET"))
	{
		return Err(BundleError::InvalidCatalog {
			reason: "targets must be unique safe components and include this CLI host".to_owned(),
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

fn fetch_release(tag: &str) -> Result<GithubRelease, BundleError> {
	let url = format!("{PINA_RELEASES_API}/{tag}");
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
	let expected_url = format!("{PINA_RELEASE_DOWNLOADS}/{tag}/{}", asset.name);
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
		if !expected_files.insert(executable.file.clone()) {
			return invalid_bundle(
				path,
				&format!("duplicate Dylint tool file {}", executable.file),
			);
		}
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
		if !expected_files.insert(library.file.clone()) {
			return invalid_bundle(path, &format!("duplicate library file {}", library.file));
		}
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
	use flate2::Compression;
	use flate2::write::GzEncoder;
	use serde_json::json;

	use super::*;
	const TEST_DYLINT_VERSION: &str = "6.0.4";

	#[test]
	fn embedded_catalog_is_valid_and_sorted() {
		let catalog = lint_catalog().expect("embedded catalog should be valid");
		let mut sorted = catalog.libraries.clone();
		sorted.sort();
		assert_eq!(catalog.libraries, sorted);
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
			select_asset(&release, "v1.2.3", "pina-lints-test-v1.2.3.tar.gz"),
			Err(BundleError::UnexpectedDownloadUrl { .. })
		));
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
	fn bundle_validation_detects_tampered_library_bytes() {
		let catalog = LintCatalog {
			schema_version: 1,
			dylint_version: TEST_DYLINT_VERSION.to_owned(),
			toolchain: "nightly-test".to_owned(),
			targets: vec![env!("PINA_BUILD_TARGET").to_owned()],
			libraries: vec!["secure_lint".to_owned()],
		};
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

		fs::write(directory.path().join(&file), b"tampered native library")
			.expect("library should be replaced");
		assert!(matches!(
			validate_bundle(directory.path(), &catalog, "1.2.3", target),
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

		fs::write(&tools.cargo_dylint, b"tampered executable").expect("tool should be replaced");
		assert!(matches!(
			validate_tool_bundle(directory.path(), TEST_DYLINT_VERSION, target),
			Err(BundleError::InvalidBundle { .. })
		));
	}
}
