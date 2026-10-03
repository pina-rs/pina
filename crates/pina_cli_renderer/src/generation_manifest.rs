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
	/// finds a symbolic link or a directory is left alone rather than followed.
	/// Directories left empty by the removals are pruned back toward `root`.
	pub(crate) fn remove_under(&self, root: &Path, prefix: &Path) -> Result<()> {
		for entry in &self.paths {
			if !entry.starts_with(prefix) || !is_tracked_entry(entry) {
				continue;
			}

			let target = root.join(entry);
			let Ok(metadata) = fs::symlink_metadata(&target) else {
				continue;
			};

			// Renders record files only. A directory here comes from a manifest
			// Pina did not write, and removing it would take the files a
			// developer keeps inside it.
			if metadata.is_symlink() || metadata.is_dir() {
				continue;
			}

			fs::remove_file(&target).map_err(|source| write_file_error(&target, source))?;

			prune_empty_parents(root, target.parent());
		}

		Ok(())
	}
}

/// Record `paths` (relative to `root`) as the client's tracked files.
pub(crate) fn write(root: &Path, paths: &BTreeSet<PathBuf>) -> Result<()> {
	let manifest = serde_json::json!({
		"paths": paths.iter().map(|path| manifest_entry(path)).collect::<Vec<_>>(),
	});

	fs::create_dir_all(root).map_err(|source| write_file_error(root, source))?;
	// `{:#}` pretty-prints a JSON value, and formatting one cannot fail.
	fs::write(root.join(MANIFEST_FILE), format!("{manifest:#}\n"))
		.map_err(|source| write_file_error(&root.join(MANIFEST_FILE), source))
}

/// Spell a tracked path with forward slashes, as every platform reads it.
///
/// The manifest is committed with the client, so it must not depend on where
/// it was written. A native Windows spelling would name one oddly named file,
/// not a nested one, when the manifest is read on another platform.
fn manifest_entry(path: &Path) -> String {
	path.components()
		.map(|component| component.as_os_str().to_string_lossy())
		.collect::<Vec<_>>()
		.join("/")
}

/// Return whether `path` is a plain relative entry a manifest may record.
///
/// Renders record file paths made of normal components only, so `.` is as
/// illegitimate as `..`: on its own it names the client root.
fn is_tracked_entry(path: &Path) -> bool {
	!path.as_os_str().is_empty()
		&& path
			.components()
			.all(|component| matches!(component, Component::Normal(_)))
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

	/// Write `contents` to `path`, creating its parent directories.
	fn put(path: &Path, contents: &str) {
		let parent = path.parent().expect("the fixture path has a parent");

		fs::create_dir_all(parent).unwrap_or_else(|e| panic!("mkdir {}: {e}", parent.display()));
		fs::write(path, contents).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
	}

	fn read_manifest(root: &Path) -> String {
		fs::read_to_string(root.join(MANIFEST_FILE)).unwrap_or_else(|e| panic!("read failed: {e}"))
	}

	#[test]
	fn entries_are_recorded_with_forward_slashes_on_every_platform() {
		let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("{error}"));
		let root = temp.path();
		// Joined components carry the platform's own separator.
		let nested = Path::new("src").join("generated").join("mod.rs");

		write(root, &BTreeSet::from([nested])).unwrap_or_else(|e| panic!("write failed: {e}"));

		assert!(read_manifest(root).contains("\"src/generated/mod.rs\""));
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
		assert_eq!(
			read_manifest(root),
			"{\n  \"paths\": [\n    \"Cargo.toml\",\n    \"src/generated/mod.rs\"\n  ]\n}\n"
		);
	}

	#[test]
	fn a_manifest_that_cannot_be_written_names_its_path() {
		let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("{error}"));
		let root = temp.path();
		// A directory where the manifest file belongs makes the write fail.
		put(&root.join(MANIFEST_FILE).join("blocker"), "");

		let error = write(root, &BTreeSet::from([PathBuf::from("Cargo.toml")]))
			.expect_err("a directory in the manifest's place must fail the write");

		assert!(
			error.to_string().contains(MANIFEST_FILE),
			"unexpected error: {error}"
		);
	}

	#[test]
	fn malformed_or_absent_manifests_do_not_load() {
		let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("{error}"));
		let root = temp.path();

		assert!(GenerationManifest::load(root).is_none(), "absent manifest");

		for contents in ["", "not json", "{\"paths\": 4}", "{\"other\": []}"] {
			put(&root.join(MANIFEST_FILE), contents);
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
		put(&root.join("src/generated/owned.rs"), "old");
		put(&root.join("foreign.txt"), "keep");

		let manifest = manifest_with(&[
			"src/generated/owned.rs",
			"/etc/passwd",
			"../escape",
			"",
			"C:\\escape",
			".",
			"./foreign.txt",
		]);

		let all = Path::new("");
		GenerationManifest::remove_under(&manifest, root, all).unwrap_or_else(|e| panic!("{e}"));

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
	fn a_directory_entry_never_removes_the_files_inside_it() {
		let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("{error}"));
		let root = temp.path();
		let prefix = Path::new("src/generated");
		put(&root.join("src/generated/nested/developer.rs"), "keep");

		// Renders record files, so this entry was not written by one.
		let manifest = manifest_with(&["src/generated/nested", "src/generated"]);

		GenerationManifest::remove_under(&manifest, root, prefix).unwrap_or_else(|e| panic!("{e}"));

		assert!(root.join("src/generated/nested/developer.rs").is_file());
	}

	#[test]
	fn removal_is_scoped_to_the_prefix_and_skips_links_and_missing_paths() {
		let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("{error}"));
		let root = temp.path();
		let prefix = Path::new("src/generated");
		put(&root.join("src/generated/owned.rs"), "old");
		put(&root.join("scaffold/Cargo.toml"), "keep");

		#[cfg(unix)]
		let link = root.join("src/generated/link.rs");
		#[cfg(unix)]
		std::os::unix::fs::symlink("/etc/passwd", &link).unwrap_or_else(|e| panic!("symlink: {e}"));

		let manifest = manifest_with(&[
			"src/generated/owned.rs",
			"src/generated/link.rs",
			"src/generated/missing.rs",
			"scaffold/Cargo.toml",
		]);

		GenerationManifest::remove_under(&manifest, root, prefix).unwrap_or_else(|e| panic!("{e}"));

		assert!(!root.join("src/generated/owned.rs").exists());
		assert!(
			root.join("scaffold/Cargo.toml").exists(),
			"entries outside the prefix survive"
		);
		#[cfg(unix)]
		assert!(
			link.symlink_metadata()
				.is_ok_and(|metadata| metadata.file_type().is_symlink()),
			"links are skipped, not followed"
		);
	}
}
