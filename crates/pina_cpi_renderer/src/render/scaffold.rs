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
	fs::create_dir_all(crate_dir.join("src")).map_err(|source| {
		RenderError::WriteFile {
			path: crate_dir.to_path_buf(),
			source,
		}
	})?;

	let lib_rs_path = crate_dir.join("src/lib.rs");
	if !lib_rs_path.exists() {
		fs::write(&lib_rs_path, "pub mod generated;\npub use generated::*;\n").map_err(
			|source| {
				RenderError::WriteFile {
					path: lib_rs_path.clone(),
					source,
				}
			},
		)?;
	}

	let cargo_toml_path = crate_dir.join("Cargo.toml");
	if !cargo_toml_path.exists() {
		let package_name = format!("{}-cpi", snake(program_name).replace('_', "-"));
		let cargo_toml = format!(
			r#"[package]
name = "{package_name}"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
pinocchio = {{ version = "0.11", features = ["cpi"] }}
solana-address = {{ version = "2", features = ["decode"] }}
"#
		);
		fs::write(&cargo_toml_path, cargo_toml).map_err(|source| {
			RenderError::WriteFile {
				path: cargo_toml_path.clone(),
				source,
			}
		})?;
	}

	Ok(())
}

/// Writes the generated file map, deleting a previous generated directory first.
pub(crate) fn write_files(base: &Path, files: &BTreeMap<PathBuf, String>) -> Result<()> {
	for (path, content) in files {
		let full_path = base.join(path);
		if let Some(parent) = full_path.parent() {
			fs::create_dir_all(parent).map_err(|source| {
				RenderError::WriteFile {
					path: parent.to_path_buf(),
					source,
				}
			})?;
		}
		fs::write(&full_path, content).map_err(|source| {
			RenderError::WriteFile {
				path: full_path.clone(),
				source,
			}
		})?;
	}

	Ok(())
}
