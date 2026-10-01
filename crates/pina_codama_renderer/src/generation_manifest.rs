//! Manifest-bounded cleanup for generated client trees.
//!
//! Every render records the files it owns in [`MANIFEST_FILE`] at the client
//! root, and later runs remove only those recorded paths. A generated tree
//! therefore never loses files its developer added, and `overwrite` refuses a
//! directory Pina never generated instead of removing it. A committed
//! manifest is untrusted input: only plain relative entries are honored, so
//! it cannot direct deletion outside its root or through parent directories,
//! and a path whose lookup finds a symbolic link is skipped rather than
//! followed.

use std::collections::BTreeSet;
use std::fs;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use crate::error::Result;
use crate::write_file_error;

/// The tracked-files record written at a generated client's root.
pub(crate) const MANIFEST_FILE: &str = ".pina-generated.json";

/// The tracked paths recorded in a client root.
#[derive(Debug, Clone)]
pub(crate) struct GenerationManifest {
	paths: Vec<PathBuf>,
}

impl GenerationManifest {
	/// Load the manifest at `root`, or `None` when it is absent, unreadable,
	/// malformed, or shaped like something Pina did not write.
	pub(crate) fn load(root: &Path) -> Option<Self> {
		let contents = fs::read_to_string(root.join(MANIFEST_FILE)).ok()?;
		let parsed: serde_json::Value = serde_json::from_str(&contents).ok()?;
		let entries = parsed.get("paths")?.as_array()?;

		Some(Self {
			paths: entries
				.iter()
				.filter_map(|entry| entry.as_str())
				.filter(|entry| is_tracked_entry(Path::new(entry)))
				.map(PathBuf::from)
				.collect(),
		})
	}

	/// Remove the tracked paths under `prefix` inside `root`.
	///
	/// Entries are re-validated here as well as at load time, so a manifest
	/// constructed without [`GenerationManifest::load`] is bounded the same
	/// way. Paths that no longer exist are skipped, and a path whose lookup
	/// finds a symbolic link is left alone rather than followed. Directories
	/// left empty by the removals are pruned back toward `root`.
	pub(crate) fn remove_under(&self, root: &Path, prefix: &Path) -> Result<()> {
		for entry in &self.paths {
			if !entry.starts_with(prefix) || !is_tracked_entry(entry) {
				continue;
			}

			let target = root.join(entry);
			let Ok(metadata) = fs::symlink_metadata(&target) else {
				continue;
			};

			if metadata.is_symlink() {
				continue;
			}

			if metadata.is_dir() {
				fs::remove_dir_all(&target).map_err(|source| write_file_error(&target, source))?;
			} else {
				fs::remove_file(&target).map_err(|source| write_file_error(&target, source))?;
			}

			prune_empty_parents(root, target.parent());
		}

		Ok(())
	}
}

/// Record `paths` (relative to `root`) as the client's tracked files.
pub(crate) fn write(root: &Path, paths: &BTreeSet<PathBuf>) -> Result<()> {
	let payload = serde_json::to_string_pretty(&serde_json::json!({
		"paths": paths.iter().map(|path| path.to_string_lossy()).collect::<Vec<_>>(),
	}))
	.map_err(|source| write_file_error(&root.join(MANIFEST_FILE), std::io::Error::other(source)))?;

	fs::create_dir_all(root).map_err(|source| write_file_error(root, source))?;
	fs::write(root.join(MANIFEST_FILE), format!("{payload}\n"))
		.map_err(|source| write_file_error(&root.join(MANIFEST_FILE), source))
}

/// Return whether `path` is a plain relative entry a manifest may record.
fn is_tracked_entry(path: &Path) -> bool {
	!path.as_os_str().is_empty()
		&& path
			.components()
			.all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

/// Remove directories that became empty up to, but never including, `root`.
fn prune_empty_parents(root: &Path, mut directory: Option<&Path>) {
	while let Some(current) = directory {
		if current == root {
			return;
		}

		let empty = fs::read_dir(current).is_ok_and(|mut entries| entries.next().is_none());

		if !empty || fs::remove_dir(current).is_err() {
			return;
		}

		directory = current.parent();
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn manifest_with(paths: &[&str]) -> GenerationManifest {
		GenerationManifest {
			paths: paths.iter().map(PathBuf::from).collect(),
		}
	}

	#[test]
	fn round_trip_through_disk_keeps_entries_sorted() {
		let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("{error}"));
		let root = temp.path();
		let paths = BTreeSet::from([
			PathBuf::from("src/generated/mod.rs"),
			PathBuf::from("Cargo.toml"),
		]);

		write(root, &paths).unwrap_or_else(|error| panic!("write failed: {error}"));
		let loaded = GenerationManifest::load(root).expect("manifest should load");

		assert_eq!(
			loaded.paths,
			vec![
				PathBuf::from("Cargo.toml"),
				PathBuf::from("src/generated/mod.rs")
			]
		);
		assert!(
			fs::read_to_string(root.join(MANIFEST_FILE))
				.unwrap_or_else(|error| panic!("read failed: {error}"))
				.ends_with("}\n")
		);
	}

	#[test]
	fn malformed_or_absent_manifests_do_not_load() {
		let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("{error}"));
		let root = temp.path();

		assert!(GenerationManifest::load(root).is_none(), "absent manifest");

		for contents in ["", "not json", "{\"paths\": 4}", "{\"other\": []}"] {
			fs::write(root.join(MANIFEST_FILE), contents)
				.unwrap_or_else(|error| panic!("write failed: {error}"));
			assert!(
				GenerationManifest::load(root).is_none(),
				"{contents} must not load"
			);
		}
	}

	#[test]
	fn untrusted_entries_are_ignored_but_sane_entries_still_remove() {
		let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("{error}"));
		let root = temp.path();
		fs::create_dir_all(root.join("src/generated")).unwrap_or_else(|error| panic!("{error}"));
		fs::write(root.join("src/generated/owned.rs"), "old")
			.unwrap_or_else(|error| panic!("{error}"));
		fs::write(root.join("foreign.txt"), "keep").unwrap_or_else(|error| panic!("{error}"));

		let manifest = manifest_with(&[
			"src/generated/owned.rs",
			"/etc/passwd",
			"../escape",
			"",
			"C:\\escape",
		]);

		manifest
			.remove_under(root, Path::new(""))
			.unwrap_or_else(|error| panic!("remove failed: {error}"));

		assert!(
			!root.join("src/generated/owned.rs").exists(),
			"tracked file must be removed"
		);
		assert!(
			root.join("foreign.txt").exists(),
			"untracked file must survive"
		);
		assert!(
			!root.join("src/generated").exists(),
			"emptied directories are pruned"
		);
	}

	#[test]
	fn removal_is_scoped_to_the_prefix_and_skips_links_and_missing_paths() {
		let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("{error}"));
		let root = temp.path();
		fs::create_dir_all(root.join("src/generated")).unwrap_or_else(|error| panic!("{error}"));
		fs::create_dir_all(root.join("scaffold")).unwrap_or_else(|error| panic!("{error}"));
		fs::write(root.join("src/generated/owned.rs"), "old")
			.unwrap_or_else(|error| panic!("{error}"));
		fs::write(root.join("scaffold/Cargo.toml"), "keep")
			.unwrap_or_else(|error| panic!("{error}"));

		#[cfg(unix)]
		std::os::unix::fs::symlink("/etc/passwd", root.join("src/generated/link.rs"))
			.unwrap_or_else(|error| panic!("symlink failed: {error}"));

		let manifest = manifest_with(&[
			"src/generated/owned.rs",
			"src/generated/link.rs",
			"src/generated/missing.rs",
			"scaffold/Cargo.toml",
		]);

		manifest
			.remove_under(root, Path::new("src/generated"))
			.unwrap_or_else(|error| panic!("remove failed: {error}"));

		assert!(!root.join("src/generated/owned.rs").exists());
		assert!(
			root.join("scaffold/Cargo.toml").exists(),
			"entries outside the prefix survive"
		);
		#[cfg(unix)]
		assert!(
			root.join("src/generated/link.rs")
				.symlink_metadata()
				.is_ok_and(|metadata| metadata.file_type().is_symlink()),
			"links are skipped, not followed"
		);
	}
}
