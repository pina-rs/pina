//! Link-aware path validation for commands that publish sensitive files.

use std::fs;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

/// Return whether an existing path component is a link that could redirect a
/// write.
///
/// Every symbolic link and reparse point is untrusted except a system alias: a
/// root-owned link directly below the filesystem root, such as macOS's `/var`
/// and `/tmp` or a merged-`/usr` system's `/bin`, traversed as an ancestor.
/// Only root can create or replace an entry there, and no repository or
/// project tree occupies that depth, so the exception cannot admit a
/// project's own links even when the command runs as root. The final
/// component is never trusted, because a write to it would replace the alias
/// itself. Windows reparse points are always untrusted because
/// their owner is not available through the portable metadata API.
///
/// A path that traverses a non-directory component can never resolve, and the
/// failure surfaces as different io error kinds per platform (`NotADirectory`
/// on unix, `NotFound` on Windows), so that case is reported as an error in
/// both.
pub(crate) fn has_untrusted_link_component(path: &Path) -> Result<bool, std::io::Error> {
	let absolute = if path.is_absolute() {
		path.to_path_buf()
	} else {
		std::env::current_dir()?.join(path)
	};
	let mut current = PathBuf::new();
	let mut depth = 0_usize;
	let mut parent_is_directory = true;

	let mut components = absolute.components().peekable();

	while let Some(component) = components.next() {
		current.push(component);

		if matches!(component, Component::Prefix(_) | Component::RootDir) {
			continue;
		}

		depth += 1;
		let alias_position = depth == 1 && components.peek().is_some();

		match fs::symlink_metadata(&current) {
			Ok(metadata) if is_untrusted_link(&metadata, alias_position) => return Ok(true),
			// A trusted alias is traversed, so its target decides whether a
			// miss beneath it is an ordinary absent entry. Non-link entries
			// resolve identically through `Path::is_dir`.
			Ok(_) => parent_is_directory = current.is_dir(),
			// Plain misses below an existing directory are not link-like.
			// Everything else through this lookup is either a real error or a
			// traversal into a non-directory (`ENOTDIR` on unix, `NotFound` on
			// Windows); both normalize to one synthetic error so the reported
			// kind is identical on every platform.
			Err(error) => return classify_metadata_error(error, parent_is_directory),
		}
	}

	Ok(false)
}

fn classify_metadata_error(
	error: std::io::Error,
	parent_is_directory: bool,
) -> Result<bool, std::io::Error> {
	match error.kind() {
		std::io::ErrorKind::NotFound if parent_is_directory => Ok(false),
		std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory => {
			Err(std::io::Error::from(std::io::ErrorKind::NotADirectory))
		}
		_ => Err(error),
	}
}

pub(crate) fn is_untrusted_link(metadata: &fs::Metadata, alias_position: bool) -> bool {
	is_link_like(metadata) && !(alias_position && is_owned_by_root(metadata))
}

fn is_owned_by_root(metadata: &fs::Metadata) -> bool {
	#[cfg(unix)]
	{
		use std::os::unix::fs::MetadataExt as _;

		uid_is_root(metadata.uid())
	}

	#[cfg(not(unix))]
	{
		let _ = metadata;
		false
	}
}

#[cfg(any(unix, test))]
const fn uid_is_root(uid: u32) -> bool {
	uid == 0
}

pub(crate) fn is_link_like(metadata: &fs::Metadata) -> bool {
	#[cfg(windows)]
	{
		use std::os::windows::fs::MetadataExt;

		is_windows_link_like(
			metadata.file_type().is_symlink(),
			metadata.file_attributes(),
		)
	}

	#[cfg(not(windows))]
	metadata.file_type().is_symlink()
}

/// Return whether `symlink_metadata` describes a regular file rather than a
/// directory, symbolic link, or Windows reparse point.
pub(crate) fn is_plain_file(metadata: &fs::Metadata) -> bool {
	#[cfg(windows)]
	{
		use std::os::windows::fs::MetadataExt;

		is_windows_plain_file(
			metadata.file_type().is_file(),
			metadata.file_type().is_symlink(),
			metadata.file_attributes(),
		)
	}

	#[cfg(not(windows))]
	metadata.file_type().is_file()
}

#[cfg(any(windows, test))]
const fn is_windows_link_like(is_symlink: bool, file_attributes: u32) -> bool {
	const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

	is_symlink || file_attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(any(windows, test))]
const fn is_windows_plain_file(is_file: bool, is_symlink: bool, file_attributes: u32) -> bool {
	is_file && !is_windows_link_like(is_symlink, file_attributes)
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
			!has_untrusted_link_component(&ordinary)
				.unwrap_or_else(|error| { panic!("ordinary path inspection failed: {error}") })
		);
		assert!(
			!has_untrusted_link_component(&ordinary.join("missing/file"))
				.unwrap_or_else(|error| { panic!("missing path inspection failed: {error}") })
		);
		assert!(
			!has_untrusted_link_component(Path::new("."))
				.unwrap_or_else(|error| panic!("relative path inspection failed: {error}"))
		);
	}

	#[test]
	fn path_inspection_errors_are_propagated() {
		let invalid = PathBuf::from("x".repeat(32 * 1024));
		let error = has_untrusted_link_component(&invalid)
			.expect_err("oversize path component must return an inspection error");
		assert_ne!(error.kind(), std::io::ErrorKind::NotADirectory);
	}

	#[test]
	fn classify_metadata_error_normalizes_non_directory_traversals_and_preserves_other_errors() {
		assert!(
			!classify_metadata_error(std::io::Error::from(std::io::ErrorKind::NotFound), true)
				.expect("missing child below directory must succeed")
		);

		let windows_not_dir =
			classify_metadata_error(std::io::Error::from(std::io::ErrorKind::NotFound), false)
				.expect_err("missing child below file must fail");
		assert_eq!(windows_not_dir.kind(), std::io::ErrorKind::NotADirectory);

		let unix_not_dir = classify_metadata_error(
			std::io::Error::new(std::io::ErrorKind::NotADirectory, "os error"),
			false,
		)
		.expect_err("not-a-directory lookup must fail");
		assert_eq!(unix_not_dir.kind(), std::io::ErrorKind::NotADirectory);
		assert_eq!(unix_not_dir.raw_os_error(), None);

		let denied = classify_metadata_error(
			std::io::Error::from(std::io::ErrorKind::PermissionDenied),
			true,
		)
		.expect_err("permission errors must propagate");
		assert_eq!(denied.kind(), std::io::ErrorKind::PermissionDenied);
	}

	#[test]
	fn traversal_through_a_file_is_an_error_on_every_platform() {
		let temp = TempDir::new().expect("temp dir");
		let root = fs::canonicalize(temp.path()).expect("canonicalize temp dir");
		let blocked = root.join("blocked");
		fs::write(&blocked, b"not a directory").expect("write file");

		let direct = has_untrusted_link_component(&blocked.join("migrations"))
			.expect_err("direct child beneath file must fail");
		assert_eq!(direct.kind(), std::io::ErrorKind::NotADirectory);

		let nested = has_untrusted_link_component(&blocked.join("migrations/.lock"))
			.expect_err("nested child beneath file must fail");
		assert_eq!(nested.kind(), std::io::ErrorKind::NotADirectory);
	}

	#[test]
	fn plain_file_and_windows_reparse_point_checks_distinguish_entries() {
		let temp = TempDir::new().expect("temp dir");
		let file = temp.path().join("ordinary.txt");
		fs::write(&file, b"ok").expect("write file");
		let file_metadata = fs::symlink_metadata(&file).expect("file metadata");
		let dir_metadata = fs::symlink_metadata(temp.path()).expect("dir metadata");

		assert!(is_plain_file(&file_metadata));
		assert!(!is_plain_file(&dir_metadata));

		#[cfg(unix)]
		{
			let link = temp.path().join("link.txt");
			std::os::unix::fs::symlink(&file, &link).expect("create symlink");
			let link_metadata = fs::symlink_metadata(&link).expect("link metadata");
			assert!(!is_plain_file(&link_metadata));
		}

		assert!(!is_windows_link_like(false, 0));
		assert!(!is_windows_link_like(false, 0x0020));
		assert!(is_windows_link_like(false, 0x0400));
		assert!(is_windows_link_like(false, 0x0420));
		assert!(is_windows_link_like(true, 0));

		assert!(is_windows_plain_file(true, false, 0));
		assert!(is_windows_plain_file(true, false, 0x0020));
		assert!(!is_windows_plain_file(true, false, 0x0400));
		assert!(!is_windows_plain_file(true, true, 0));
		assert!(!is_windows_plain_file(false, false, 0));

		assert!(uid_is_root(0));
		assert!(!uid_is_root(1_000));
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
			has_untrusted_link_component(&link.join("secret.json"))
				.unwrap_or_else(|error| { panic!("link inspection failed: {error}") })
		);
	}

	#[cfg(unix)]
	#[test]
	fn only_top_level_root_owned_links_are_trusted() {
		use std::os::unix::fs::MetadataExt as _;
		use std::os::unix::fs::symlink;

		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
		let root = fs::canonicalize(temp.path());
		let root = root.unwrap_or_else(|error| panic!("canonicalize failed: {error}"));
		let link = root.join("link");
		symlink(&root, &link).unwrap_or_else(|error| panic!("symlink failed: {error}"));
		let link_metadata =
			fs::symlink_metadata(&link).unwrap_or_else(|error| panic!("metadata failed: {error}"));
		let directory_metadata =
			fs::symlink_metadata(&root).unwrap_or_else(|error| panic!("metadata failed: {error}"));
		let root_dir_metadata = fs::symlink_metadata("/").expect("root metadata");

		assert!(is_owned_by_root(&root_dir_metadata));
		assert_eq!(is_owned_by_root(&link_metadata), link_metadata.uid() == 0);

		// Below the top level every link is untrusted, whoever owns it, so a
		// command running as root still rejects a project's own links.
		assert!(is_untrusted_link(&link_metadata, false));
		assert_eq!(
			is_untrusted_link(&link_metadata, true),
			link_metadata.uid() != 0
		);
		assert!(!is_untrusted_link(&directory_metadata, true));
		assert!(!is_untrusted_link(&directory_metadata, false));
	}

	#[cfg(unix)]
	#[test]
	fn system_aliases_are_trusted_only_as_ancestors() {
		use std::os::unix::fs::MetadataExt as _;

		let entries = fs::read_dir("/").unwrap_or_else(|error| panic!("read / failed: {error}"));
		let aliases = entries
			.map(|entry| entry.unwrap_or_else(|error| panic!("root entry failed: {error}")))
			.map(|entry| entry.path())
			.filter(|path| {
				fs::symlink_metadata(path)
					.is_ok_and(|metadata| is_link_like(&metadata) && metadata.uid() == 0)
					&& path.is_dir()
			})
			.collect::<Vec<_>>();

		// macOS resolves every temporary directory through `/var` or `/tmp`.
		#[cfg(target_os = "macos")]
		for expected in ["/var", "/tmp"] {
			assert!(
				aliases.iter().any(|alias| alias == Path::new(expected)),
				"{expected} should be a root-owned system alias"
			);
		}

		for alias in aliases {
			let ancestor = has_untrusted_link_component(&alias.join("missing/secret.json"));
			let ancestor = ancestor.unwrap_or_else(|error| panic!("inspection failed: {error}"));
			// Writing to the alias itself would replace it, so it is never
			// trusted as the final component.
			let destination = has_untrusted_link_component(&alias);
			let destination =
				destination.unwrap_or_else(|error| panic!("inspection failed: {error}"));

			assert!(
				!ancestor,
				"{} should be trusted as an ancestor",
				alias.display()
			);
			assert!(
				destination,
				"{} should be refused as a destination",
				alias.display()
			);
		}
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
			has_untrusted_link_component(&link.join("secret.json"))
				.unwrap_or_else(|error| { panic!("reparse inspection failed: {error}") })
		);
	}
}
