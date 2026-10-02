//! Host-side updates to the generated-client tracked-files manifests.
//!
//! The render script and the Rust renderers write `.pina-generated.json` at
//! each client root; the post-render hardening steps (JavaScript helper
//! modules, Dart package barrels) add files of their own afterwards. This
//! module appends those paths to the manifest the renderer wrote, so the
//! next bounded cleanup knows about every file Pina owns. Entries are
//! validated with the same rules the renderers enforce: plain relative
//! paths only, so a committed manifest cannot direct deletion outside its
//! root or through parent directories.

use std::collections::BTreeSet;
use std::fs;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

/// The tracked-files record read and written at each generated client root.
pub(crate) const MANIFEST_FILE: &str = ".pina-generated.json";

/// Add `added` (relative to `root`) to the manifest at `root`.
///
/// A missing or malformed manifest starts a fresh record; existing entries
/// survive. Untrusted entries — absolute paths, `..` components — are
/// dropped rather than honored.
pub(crate) fn append_generated_paths(root: &Path, added: &[PathBuf]) -> std::io::Result<()> {
	let mut paths = load(root);
	paths.extend(
		added
			.iter()
			.filter(|path| is_tracked_entry(path))
			.map(PathBuf::from),
	);

	let payload = serde_json::to_string_pretty(&serde_json::json!({
		"paths": paths.iter().map(|path| manifest_entry(path)).collect::<Vec<_>>(),
	}))
	.map_err(std::io::Error::other)?;

	fs::create_dir_all(root)?;
	fs::write(root.join(MANIFEST_FILE), format!("{payload}\n"))
}

/// Spell a tracked path with forward slashes, as every platform reads it.
///
/// The render script records its paths that way, and the manifest is
/// committed with the client. A native Windows spelling would name one oddly
/// named file, not a nested one, when the manifest is read on another
/// platform.
fn manifest_entry(path: &Path) -> String {
	path.components()
		.map(|component| component.as_os_str().to_string_lossy())
		.collect::<Vec<_>>()
		.join("/")
}

/// The files under `root` before and after a step, so the step's new files
/// can be recorded without threading a writer through it.
pub(crate) struct FileSnapshot {
	files: BTreeSet<PathBuf>,
}

impl FileSnapshot {
	/// Record every regular file under `root`, relative to `root`.
	///
	/// A missing root is an empty snapshot: the renderer creates the tree
	/// after the snapshot is taken, and its own errors surface first.
	pub(crate) fn take(root: &Path) -> std::io::Result<Self> {
		let mut files = BTreeSet::new();

		match fs::read_dir(root) {
			Ok(_) => Self::walk(root, root, &mut files)?,
			Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
			Err(source) => return Err(source),
		}

		Ok(Self { files })
	}

	/// The files that appeared since the snapshot, relative to `root`.
	pub(crate) fn added_since(self, root: &Path) -> std::io::Result<Vec<PathBuf>> {
		let mut current = BTreeSet::new();
		Self::walk(root, root, &mut current)?;

		Ok(current
			.into_iter()
			.filter(|path| !self.files.contains(path))
			.collect())
	}

	fn walk(root: &Path, directory: &Path, files: &mut BTreeSet<PathBuf>) -> std::io::Result<()> {
		for entry in fs::read_dir(directory)? {
			let entry = entry?;
			let metadata = entry.metadata()?;

			if metadata.is_dir() {
				Self::walk(root, &entry.path(), files)?;
			} else if metadata.is_file() {
				files.insert(
					entry
						.path()
						.strip_prefix(root)
						.unwrap_or(&entry.path())
						.to_path_buf(),
				);
			}
		}

		Ok(())
	}
}

fn load(root: &Path) -> BTreeSet<PathBuf> {
	fs::read_to_string(root.join(MANIFEST_FILE))
		.ok()
		.and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok())
		.and_then(|parsed| {
			parsed
				.get("paths")
				.and_then(|paths| paths.as_array().cloned())
		})
		.map(|entries| {
			entries
				.iter()
				.filter_map(|entry| entry.as_str())
				.filter(|entry| is_tracked_entry(Path::new(entry)))
				.map(PathBuf::from)
				.collect()
		})
		.unwrap_or_default()
}

fn is_tracked_entry(path: &Path) -> bool {
	!path.as_os_str().is_empty()
		&& path
			.components()
			.all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn appending_extends_the_record_and_survives_untrusted_entries() {
		let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("{error}"));
		let root = temp.path();

		append_generated_paths(root, &[PathBuf::from("src/generated/mod.rs")])
			.unwrap_or_else(|error| panic!("append failed: {error}"));
		append_generated_paths(
			root,
			&[PathBuf::from("lib/barrel.dart"), PathBuf::from("../escape")],
		)
		.unwrap_or_else(|error| panic!("append failed: {error}"));

		let contents = fs::read_to_string(root.join(MANIFEST_FILE))
			.unwrap_or_else(|error| panic!("read failed: {error}"));

		assert!(contents.contains("src/generated/mod.rs"));
		assert!(contents.contains("lib/barrel.dart"));
		assert!(
			!contents.contains("escape"),
			"untrusted entries are dropped"
		);
	}

	#[test]
	fn entries_are_recorded_with_forward_slashes_on_every_platform() {
		let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("{error}"));
		let root = temp.path();
		// Joined components carry the platform's own separator, as the paths a
		// file snapshot reports do.
		let nested = Path::new("src").join("generated").join("mod.rs");

		append_generated_paths(root, &[nested])
			.unwrap_or_else(|error| panic!("append failed: {error}"));

		assert!(
			fs::read_to_string(root.join(MANIFEST_FILE))
				.unwrap_or_else(|error| panic!("read failed: {error}"))
				.contains("\"src/generated/mod.rs\"")
		);
	}

	#[test]
	fn appending_over_a_malformed_manifest_starts_a_clean_record() {
		let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("{error}"));
		let root = temp.path();
		fs::write(root.join(MANIFEST_FILE), "not json")
			.unwrap_or_else(|error| panic!("write failed: {error}"));

		append_generated_paths(root, &[PathBuf::from("helpers.ts")])
			.unwrap_or_else(|error| panic!("append failed: {error}"));

		let contents = fs::read_to_string(root.join(MANIFEST_FILE))
			.unwrap_or_else(|error| panic!("read failed: {error}"));
		assert!(contents.contains("helpers.ts"));
		assert!(!contents.contains("not json"));
	}

	#[test]
	fn snapshots_report_only_new_files_under_the_root() {
		let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("{error}"));
		let root = temp.path().join("client");
		fs::create_dir_all(root.join("src/generated"))
			.unwrap_or_else(|error| panic!("mkdir failed: {error}"));
		fs::write(root.join("src/generated/index.ts"), "old")
			.unwrap_or_else(|error| panic!("write failed: {error}"));

		let snapshot =
			FileSnapshot::take(&root).unwrap_or_else(|error| panic!("snapshot failed: {error}"));
		fs::write(root.join("src/generated/pinaPodCodecs.ts"), "new")
			.unwrap_or_else(|error| panic!("write failed: {error}"));

		let added = snapshot
			.added_since(&root)
			.unwrap_or_else(|error| panic!("diff failed: {error}"));

		assert_eq!(added, vec![PathBuf::from("src/generated/pinaPodCodecs.ts")]);
	}
}
