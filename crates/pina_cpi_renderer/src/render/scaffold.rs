use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use cap_fs_ext::FollowSymlinks;
use cap_fs_ext::OpenOptionsFollowExt;
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use cap_std::fs::Metadata;
use cap_std::fs::OpenOptions;

use super::helpers::rust_string_literal;
use super::helpers::snake;
use crate::error::RenderError;
use crate::error::Result;

/// Opens and pins the crate output directory for capability-relative writes.
pub(crate) fn open_crate_dir(crate_dir: &Path) -> Result<Dir> {
	fs::create_dir_all(crate_dir).map_err(|source| write_file_error(crate_dir, source))?;
	Dir::open_ambient_dir(crate_dir, ambient_authority())
		.map_err(|source| write_file_error(crate_dir, source))
}

/// Scaffolds a standalone CPI crate for the rendered program.
///
/// Creates `src/lib.rs` and `Cargo.toml` when they do not exist yet and never
/// overwrites either file, so consumers can pin dependencies themselves.
pub(crate) fn ensure_crate_scaffold(
	crate_dir: &Dir,
	crate_path: &Path,
	program_name: &str,
	generated_folder: &Path,
) -> Result<()> {
	ensure_relative_directory(crate_dir, crate_path, Path::new("src"))?;

	let generated_module = generated_folder
		.strip_prefix("src")
		.unwrap_or(generated_folder);
	let lib_rs = if generated_module == Path::new("generated") {
		"#![no_std]\n\npub mod generated;\npub use generated::*;\n".to_string()
	} else {
		let module_path = generated_module.join("mod.rs");
		format!(
			"#![no_std]\n\n#[path = {}]\npub mod generated;\npub use generated::*;\n",
			rust_string_literal(&module_path.to_string_lossy())
		)
	};
	create_scaffold_file(crate_dir, crate_path, Path::new("src/lib.rs"), &lib_rs)?;

	let package_name = format!("{}-cpi", snake(program_name).replace('_', "-"));
	let cargo_toml = [
		"[package]".to_string(),
		format!("name = \"{package_name}\""),
		"version = \"0.0.0\"".to_string(),
		"edition = \"2021\"".to_string(),
		"publish = false".to_string(),
		String::new(),
		"[dependencies]".to_string(),
		"pina = { version = \"0.12\", default-features = false }".to_string(),
		String::new(),
	]
	.join("\n");
	create_scaffold_file(crate_dir, crate_path, Path::new("Cargo.toml"), &cargo_toml)
}

/// Writes the generated file map relative to the pinned crate directory.
pub(crate) fn write_files(
	crate_dir: &Dir,
	crate_path: &Path,
	generated_folder: &Path,
	files: &BTreeMap<PathBuf, String>,
) -> Result<()> {
	files.iter().try_for_each(|(path, content)| {
		let relative = generated_folder.join(path);
		let parent = relative.parent().unwrap_or(generated_folder);
		ensure_relative_directory(crate_dir, crate_path, parent)?;
		ensure_not_symlink(crate_dir, crate_path, &relative)?;
		write_relative_file(crate_dir, crate_path, &relative, content, true)
	})
}

fn create_scaffold_file(
	crate_dir: &Dir,
	crate_path: &Path,
	relative: &Path,
	content: &str,
) -> Result<()> {
	match crate_dir.symlink_metadata(relative) {
		Ok(metadata) if metadata.file_type().is_symlink() => {
			return Err(unsafe_output(
				crate_path.join(relative),
				"refusing to follow a scaffold-file symlink",
			));
		}
		Ok(_) => return Ok(()),
		Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
		Err(source) => return Err(read_file_error(&crate_path.join(relative), source)),
	}

	write_relative_file(crate_dir, crate_path, relative, content, false)
}

fn write_relative_file(
	crate_dir: &Dir,
	crate_path: &Path,
	relative: &Path,
	content: &str,
	overwrite: bool,
) -> Result<()> {
	let mut options = OpenOptions::new();
	options.write(true).follow(FollowSymlinks::No);
	if overwrite {
		options.create(true).truncate(true);
	} else {
		options.create_new(true);
	}

	let display_path = crate_path.join(relative);
	let mut file = crate_dir
		.open_with(relative, &options)
		.map_err(|source| write_file_error(&display_path, source))?;
	file.write_all(content.as_bytes())
		.map_err(|source| write_file_error(&display_path, source))
}

fn ensure_relative_directory(crate_dir: &Dir, crate_path: &Path, relative: &Path) -> Result<()> {
	let mut current = PathBuf::new();
	for component in relative.components() {
		current.push(component.as_os_str());
		let display_path = crate_path.join(&current);
		match optional_metadata(&display_path, crate_dir.symlink_metadata(&current))? {
			Some(metadata) if metadata.file_type().is_symlink() => {
				return Err(unsafe_output(
					display_path,
					"output directories must not traverse symlinks",
				));
			}
			Some(metadata) if !metadata.is_dir() => {
				return Err(unsafe_output(display_path, "expected an output directory"));
			}
			Some(_) => {}
			None => create_relative_directory(crate_dir, &display_path, &current)?,
		}
	}

	Ok(())
}

fn create_relative_directory(crate_dir: &Dir, display_path: &Path, relative: &Path) -> Result<()> {
	crate_dir
		.create_dir(relative)
		.map_err(|source| write_file_error(display_path, source))
}

fn optional_metadata(path: &Path, metadata: std::io::Result<Metadata>) -> Result<Option<Metadata>> {
	match metadata {
		Ok(metadata) => Ok(Some(metadata)),
		Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
		Err(source) => Err(read_file_error(path, source)),
	}
}

fn ensure_not_symlink(crate_dir: &Dir, crate_path: &Path, relative: &Path) -> Result<()> {
	match crate_dir.symlink_metadata(relative) {
		Ok(metadata) if metadata.file_type().is_symlink() => {
			Err(unsafe_output(
				crate_path.join(relative),
				"refusing to follow a generated-file symlink",
			))
		}
		Ok(_) => Ok(()),
		Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
		Err(source) => Err(read_file_error(&crate_path.join(relative), source)),
	}
}

fn unsafe_output(path: PathBuf, reason: &str) -> RenderError {
	RenderError::UnsafeOutputPath {
		path,
		reason: reason.to_string(),
	}
}

fn read_file_error(path: &Path, source: std::io::Error) -> RenderError {
	RenderError::ReadFile {
		path: path.to_path_buf(),
		source,
	}
}

fn write_file_error(path: &Path, source: std::io::Error) -> RenderError {
	RenderError::WriteFile {
		path: path.to_path_buf(),
		source,
	}
}

#[cfg(test)]
mod tests {
	use std::fs;
	use std::time::SystemTime;
	use std::time::UNIX_EPOCH;

	use super::*;

	#[cfg(unix)]
	#[test]
	fn descriptor_relative_guards_cover_every_existing_path_shape() {
		use std::os::unix::fs::symlink;

		let nonce = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap_or_default()
			.as_nanos();
		let root = std::env::temp_dir().join(format!("pina-cpi-scaffold-{nonce}"));
		let handle = open_crate_dir(&root).unwrap_or_else(|error| panic!("open failed: {error}"));
		fs::create_dir(root.join("directory"))
			.unwrap_or_else(|error| panic!("directory failed: {error}"));
		fs::write(root.join("file"), "sentinel")
			.unwrap_or_else(|error| panic!("file failed: {error}"));
		symlink(root.join("directory"), root.join("link"))
			.unwrap_or_else(|error| panic!("symlink failed: {error}"));

		assert!(matches!(
			ensure_relative_directory(&handle, &root, Path::new("link/child")),
			Err(RenderError::UnsafeOutputPath { .. })
		));
		assert!(matches!(
			ensure_relative_directory(&handle, &root, Path::new("file/child")),
			Err(RenderError::UnsafeOutputPath { .. })
		));
		assert!(matches!(
			ensure_not_symlink(&handle, &root, Path::new("link")),
			Err(RenderError::UnsafeOutputPath { .. })
		));
		assert!(matches!(
			ensure_not_symlink(&handle, &root, Path::new("file/child")),
			Err(RenderError::ReadFile { .. })
		));
		assert!(matches!(
			create_scaffold_file(&handle, &root, Path::new("file/child"), "source"),
			Err(RenderError::ReadFile { .. })
		));
		assert!(matches!(
			read_file_error(Path::new("read"), std::io::Error::other("failure")),
			RenderError::ReadFile { .. }
		));
		assert!(matches!(
			optional_metadata(Path::new("metadata"), Err(std::io::Error::other("failure"))),
			Err(RenderError::ReadFile { .. })
		));

		fs::remove_dir_all(root).unwrap_or_else(|error| panic!("cleanup failed: {error}"));
	}
}
