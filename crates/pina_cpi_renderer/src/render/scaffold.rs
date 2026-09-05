use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use super::helpers::snake;
use crate::error::RenderError;
use crate::error::Result;

/// Scaffolds a standalone CPI crate for the rendered program.
///
/// Creates `src/lib.rs` and `Cargo.toml` when they do not exist yet and never
/// overwrites either file, so consumers can pin dependencies themselves.
pub(crate) fn ensure_crate_scaffold(crate_dir: &Path, program_name: &str) -> Result<()> {
	fs::create_dir_all(crate_dir.join("src"))
		.map_err(|source| write_file_error(crate_dir, source))?;

	let lib_rs_path = crate_dir.join("src/lib.rs");
	if !lib_rs_path.exists() {
		fs::write(
			&lib_rs_path,
			"#![no_std]\n\npub mod generated;\npub use generated::*;\n",
		)
		.map_err(|source| write_file_error(&lib_rs_path, source))?;
	}

	let cargo_toml_path = crate_dir.join("Cargo.toml");
	if !cargo_toml_path.exists() {
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
		fs::write(&cargo_toml_path, cargo_toml)
			.map_err(|source| write_file_error(&cargo_toml_path, source))?;
	}

	Ok(())
}

/// Writes the generated file map, deleting a previous generated directory first.
pub(crate) fn write_files(base: &Path, files: &BTreeMap<PathBuf, String>) -> Result<()> {
	for (path, content) in files {
		let full_path = base.join(path);
		let parent = full_path.parent().unwrap_or(base);
		fs::create_dir_all(parent).map_err(|source| write_file_error(parent, source))?;
		fs::write(&full_path, content).map_err(|source| write_file_error(&full_path, source))?;
	}

	Ok(())
}

fn write_file_error(path: &Path, source: std::io::Error) -> RenderError {
	RenderError::WriteFile {
		path: path.to_path_buf(),
		source,
	}
}
