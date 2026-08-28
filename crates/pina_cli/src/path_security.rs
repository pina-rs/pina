//! Link-aware path validation for commands that publish sensitive files.

use std::fs;
use std::path::Path;
use std::path::PathBuf;

/// Return whether any existing path component is link-like.
pub(crate) fn has_link_like_component(path: &Path) -> Result<bool, std::io::Error> {
	let absolute = if path.is_absolute() {
		path.to_path_buf()
	} else {
		std::env::current_dir()?.join(path)
	};
	let mut current = PathBuf::new();

	for component in absolute.components() {
		current.push(component);

		if matches!(
			component,
			std::path::Component::Prefix(_) | std::path::Component::RootDir
		) {
			continue;
		}

		match fs::symlink_metadata(&current) {
			Ok(metadata) if is_link_like(&metadata) => return Ok(true),
			Ok(_) => {}
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
			Err(error) => return Err(error),
		}
	}

	Ok(false)
}

pub(crate) fn is_link_like(metadata: &fs::Metadata) -> bool {
	if metadata.file_type().is_symlink() {
		return true;
	}

	#[cfg(windows)]
	{
		use std::os::windows::fs::MetadataExt;

		const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

		return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
	}

	#[cfg(not(windows))]
	false
}

#[cfg(test)]
mod tests {
	use tempfile::TempDir;

	use super::*;

	#[test]
	fn ordinary_and_missing_paths_are_not_link_like() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"));
		let ordinary = root.join("ordinary");
		fs::create_dir(&ordinary).unwrap_or_else(|error| panic!("create failed: {error}"));

		assert!(
			!has_link_like_component(&ordinary)
				.unwrap_or_else(|error| { panic!("ordinary path inspection failed: {error}") })
		);
		assert!(
			!has_link_like_component(&ordinary.join("missing/file"))
				.unwrap_or_else(|error| { panic!("missing path inspection failed: {error}") })
		);
		assert!(
			!has_link_like_component(Path::new("."))
				.unwrap_or_else(|error| panic!("relative path inspection failed: {error}"))
		);
	}

	#[test]
	fn path_inspection_errors_are_propagated() {
		let invalid = PathBuf::from("x".repeat(32 * 1024));

		assert!(has_link_like_component(&invalid).is_err());
	}

	#[cfg(unix)]
	#[test]
	fn detects_a_symlinked_ancestor() {
		use std::os::unix::fs::symlink;

		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"));
		let target = root.join("target");
		let link = root.join("link");
		fs::create_dir(&target).unwrap_or_else(|error| panic!("create failed: {error}"));
		symlink(&target, &link).unwrap_or_else(|error| panic!("symlink failed: {error}"));

		assert!(
			has_link_like_component(&link.join("secret.json"))
				.unwrap_or_else(|error| { panic!("link inspection failed: {error}") })
		);
	}

	#[cfg(windows)]
	#[test]
	fn ordinary_windows_files_are_not_reparse_points() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
		let file = temp.path().join("ordinary.txt");
		fs::write(&file, []).unwrap_or_else(|error| panic!("write failed: {error}"));
		let metadata =
			fs::symlink_metadata(&file).unwrap_or_else(|error| panic!("metadata failed: {error}"));

		assert!(!is_link_like(&metadata));
	}

	#[cfg(windows)]
	#[test]
	fn detects_a_windows_directory_reparse_point() {
		use std::os::windows::fs::symlink_dir;

		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
		let target = temp.path().join("target");
		let link = temp.path().join("link");
		fs::create_dir(&target).unwrap_or_else(|error| panic!("create failed: {error}"));
		symlink_dir(&target, &link).unwrap_or_else(|error| panic!("reparse point failed: {error}"));

		assert!(
			has_link_like_component(&link.join("secret.json"))
				.unwrap_or_else(|error| { panic!("reparse inspection failed: {error}") })
		);
	}
}
