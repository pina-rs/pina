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
			// miss beneath it is an ordinary absent entry.
			Ok(metadata) if is_link_like(&metadata) => parent_is_directory = current.is_dir(),
			Ok(metadata) => parent_is_directory = metadata.is_dir(),
			// Plain misses below an existing directory are not link-like.
			// Everything else through this lookup is either a real error or a
			// traversal into a non-directory (`ENOTDIR` on unix, `NotFound` on
			// Windows); both normalize to one synthetic error so the reported
			// kind is identical on every platform.
			Err(error) => {
				let not_found = error.kind() == std::io::ErrorKind::NotFound;
				if not_found && parent_is_directory {
					return Ok(false);
				}
				if !not_found && error.kind() != std::io::ErrorKind::NotADirectory {
					return Err(error);
				}
				return Err(std::io::Error::from(std::io::ErrorKind::NotADirectory));
			}
		}
	}

	Ok(false)
}

fn is_untrusted_link(metadata: &fs::Metadata, alias_position: bool) -> bool {
	is_link_like(metadata) && !(alias_position && is_owned_by_root(metadata))
}

#[cfg(unix)]
fn is_owned_by_root(metadata: &fs::Metadata) -> bool {
	use std::os::unix::fs::MetadataExt as _;

	metadata.uid() == 0
}

#[cfg(not(unix))]
fn is_owned_by_root(_metadata: &fs::Metadata) -> bool {
	false
}

pub(crate) fn is_link_like(metadata: &fs::Metadata) -> bool {
	#[cfg(windows)]
	{
		use std::os::windows::fs::MetadataExt;

		const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

		metadata.file_type().is_symlink()
			|| metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
	}

	#[cfg(not(windows))]
	metadata.file_type().is_symlink()
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

		assert!(has_untrusted_link_component(&invalid).is_err());
	}

	#[test]
	fn traversal_through_a_file_is_an_error_on_every_platform() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"));
		let blocked = root.join("blocked");
		fs::write(&blocked, b"not a directory")
			.unwrap_or_else(|error| panic!("write failed: {error}"));

		assert!(has_untrusted_link_component(&blocked.join("migrations")).is_err());
		assert!(has_untrusted_link_component(&blocked.join("migrations/.lock")).is_err());
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
		use std::os::unix::fs::symlink;

		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"));
		let link = root.join("link");
		symlink(&root, &link).unwrap_or_else(|error| panic!("symlink failed: {error}"));
		let link_metadata =
			fs::symlink_metadata(&link).unwrap_or_else(|error| panic!("metadata failed: {error}"));
		let directory_metadata =
			fs::symlink_metadata(&root).unwrap_or_else(|error| panic!("metadata failed: {error}"));

		// Below the top level every link is untrusted, whoever owns it, so a
		// command running as root still rejects a project's own links.
		assert!(is_untrusted_link(&link_metadata, false));
		assert_eq!(
			is_untrusted_link(&link_metadata, true),
			!is_owned_by_root(&link_metadata)
		);
		assert!(!is_untrusted_link(&directory_metadata, true));
		assert!(!is_untrusted_link(&directory_metadata, false));
	}

	#[cfg(unix)]
	#[test]
	fn system_aliases_are_trusted_only_as_ancestors() {
		let aliases = fs::read_dir("/")
			.unwrap_or_else(|error| panic!("read root failed: {error}"))
			.map(|entry| entry.unwrap_or_else(|error| panic!("root entry failed: {error}")))
			.map(|entry| entry.path())
			.filter(|path| {
				fs::symlink_metadata(path)
					.is_ok_and(|metadata| is_link_like(&metadata) && is_owned_by_root(&metadata))
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
			let destination = alias.join("pina-missing-directory/secret.json");

			assert!(
				!has_untrusted_link_component(&destination)
					.unwrap_or_else(|error| panic!("alias inspection failed: {error}")),
				"{} should be trusted",
				alias.display()
			);
			// Writing to the alias itself would replace it, so it is never
			// trusted as the final component.
			assert!(
				has_untrusted_link_component(&alias)
					.unwrap_or_else(|error| panic!("alias inspection failed: {error}")),
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
