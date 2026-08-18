use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use super::helpers::snake;
use crate::error::RenderError;
use crate::error::Result;

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
	let package_name = format!("{}-client", snake(program_name).replace('_', "-"));
	let cargo_toml = format!(
		r#"[package]
name = "{package_name}"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
bytemuck = {{ workspace = true, default-features = true }}
num-derive = {{ workspace = true, default-features = true }}
num-traits = {{ workspace = true, default-features = true }}
pina_pod_primitives = {{ workspace = true }}
solana-account-info = {{ workspace = true, default-features = true }}
solana-cpi = {{ workspace = true, default-features = true }}
solana-instruction = {{ workspace = true, default-features = true }}
solana-program-error = {{ workspace = true, default-features = true }}
solana-pubkey = {{ workspace = true, default-features = true, features = ["curve25519"] }}
thiserror = {{ workspace = true, default-features = true }}
"#
	);
	fs::write(&cargo_toml_path, cargo_toml).map_err(|source| {
		RenderError::WriteFile {
			path: cargo_toml_path.clone(),
			source,
		}
	})?;

	Ok(())
}

pub(crate) fn write_files(base: &Path, files: BTreeMap<PathBuf, String>) -> Result<()> {
	for (relative_path, content) in files {
		let file_path = base.join(relative_path);
		if let Some(parent) = file_path.parent() {
			fs::create_dir_all(parent).map_err(|source| {
				RenderError::WriteFile {
					path: parent.to_path_buf(),
					source,
				}
			})?;
		}
		fs::write(&file_path, content).map_err(|source| {
			RenderError::WriteFile {
				path: file_path.clone(),
				source,
			}
		})?;
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;
	use std::time::SystemTime;
	use std::time::UNIX_EPOCH;

	use super::*;

	#[test]
	fn scaffold_writes_formatted_cargo_toml() {
		let unique_id = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap_or_default()
			.as_nanos();
		let crate_dir =
			PathBuf::from(std::env::temp_dir()).join(format!("pina-scaffold-{unique_id}"));

		ensure_crate_scaffold(&crate_dir, "DemoProgram").expect("should write scaffold");
		let cargo_toml = fs::read_to_string(crate_dir.join("Cargo.toml"))
			.unwrap_or_else(|err| panic!("failed to read generated Cargo.toml: {err}"));

		assert!(
			cargo_toml.contains(
				"[dependencies]\nbytemuck = { workspace = true, default-features = true }"
			)
		);
		assert!(!cargo_toml.contains("workspace = true ,"));
		assert!(!cargo_toml.contains("\n\t[dependencies]"));

		fs::remove_dir_all(&crate_dir).expect("cleanup test scaffold dir");
	}
}
