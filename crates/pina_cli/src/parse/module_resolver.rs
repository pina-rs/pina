//! Module resolution for multi-file Pina programs.
//!
//! Follows `mod` declarations from `src/lib.rs` to discover all source files
//! in a program crate, parsing each into a `syn::File`.

use std::path::Path;
use std::path::PathBuf;

use crate::error::IdlError;

/// A resolved source file with its parsed AST.
#[derive(Debug)]
pub struct ResolvedFile {
	/// Path to the source file (relative or absolute).
	pub path: PathBuf,
	/// Parsed AST.
	pub file: syn::File,
}

/// Resolve all source files in a crate starting from `lib.rs`.
///
/// Parses `lib.rs`, then follows `mod` declarations to find and parse
/// additional source files. Only inline modules (with a body) and file-based
/// modules (without a body) are resolved. Conditional compilation attributes
/// are ignored — all `mod` declarations are followed.
pub fn resolve_crate(src_dir: &Path, lib_path: &Path) -> Result<Vec<ResolvedFile>, IdlError> {
	let source = std::fs::read_to_string(lib_path).map_err(|e| IdlError::io(lib_path, e))?;
	let file = syn::parse_file(&source).map_err(|e| IdlError::parse(lib_path, &e))?;

	let mut files = vec![ResolvedFile {
		path: lib_path.to_path_buf(),
		file: file.clone(),
	}];

	// Resolve child modules declared in lib.rs.
	resolve_modules(src_dir, &file, &mut files)?;

	Ok(files)
}

/// Recursively resolve `mod` declarations in a parsed file.
fn resolve_modules(
	base_dir: &Path,
	file: &syn::File,
	files: &mut Vec<ResolvedFile>,
) -> Result<(), IdlError> {
	for item in &file.items {
		let syn::Item::Mod(item_mod) = item else {
			continue;
		};

		// Skip inline modules (they're already in the parent file's AST).
		if item_mod.content.is_some() {
			continue;
		}

		let mod_name = item_mod.ident.to_string();

		// Try mod_name.rs first, then mod_name/mod.rs.
		let candidates = [
			base_dir.join(format!("{mod_name}.rs")),
			base_dir.join(&mod_name).join("mod.rs"),
		];

		let mod_path = candidates.iter().find(|p| p.is_file());

		let Some(mod_path) = mod_path else {
			// Module file not found — skip silently (could be a cfg-gated
			// or external module).
			continue;
		};

		let source = std::fs::read_to_string(mod_path).map_err(|e| IdlError::io(mod_path, e))?;
		let mod_file = syn::parse_file(&source).map_err(|e| IdlError::parse(mod_path, &e))?;

		files.push(ResolvedFile {
			path: mod_path.clone(),
			file: mod_file.clone(),
		});

		// Recurse into the resolved module's directory for nested modules.
		let child_dir = if mod_path.file_name().is_some_and(|n| n == "mod.rs") {
			mod_path.parent().unwrap_or(base_dir).to_path_buf()
		} else {
			// For foo.rs, child modules would be in foo/ directory.
			base_dir.join(&mod_name)
		};

		if !child_dir.is_dir() {
			continue;
		}

		resolve_modules(&child_dir, &mod_file, files)?;
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use std::fs;

	use super::*;

	#[test]
	fn resolves_single_file_crate() {
		let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
		let src = dir.path().join("src");
		fs::create_dir_all(&src).unwrap_or_else(|e| panic!("mkdir: {e}"));
		fs::write(src.join("lib.rs"), "pub fn hello() {}").unwrap_or_else(|e| panic!("write: {e}"));

		let files =
			resolve_crate(&src, &src.join("lib.rs")).unwrap_or_else(|e| panic!("resolve: {e}"));
		assert_eq!(files.len(), 1);
	}

	#[test]
	fn resolves_child_module_file() {
		let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
		let src = dir.path().join("src");
		fs::create_dir_all(&src).unwrap_or_else(|e| panic!("mkdir: {e}"));
		fs::write(src.join("lib.rs"), "mod state;\npub fn hello() {}")
			.unwrap_or_else(|e| panic!("write: {e}"));
		fs::write(src.join("state.rs"), "pub struct MyState {}")
			.unwrap_or_else(|e| panic!("write: {e}"));

		let files =
			resolve_crate(&src, &src.join("lib.rs")).unwrap_or_else(|e| panic!("resolve: {e}"));
		assert_eq!(files.len(), 2);
		assert!(files.iter().any(|f| f.path.ends_with("state.rs")));
	}

	#[test]
	fn resolves_mod_rs_style() {
		let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
		let src = dir.path().join("src");
		let instructions_dir = src.join("instructions");
		fs::create_dir_all(&instructions_dir).unwrap_or_else(|e| panic!("mkdir: {e}"));
		fs::write(src.join("lib.rs"), "mod instructions;\npub fn hello() {}")
			.unwrap_or_else(|e| panic!("write: {e}"));
		fs::write(
			instructions_dir.join("mod.rs"),
			"pub struct MyInstruction {}",
		)
		.unwrap_or_else(|e| panic!("write: {e}"));

		let files =
			resolve_crate(&src, &src.join("lib.rs")).unwrap_or_else(|e| panic!("resolve: {e}"));
		assert_eq!(files.len(), 2);
		assert!(files.iter().any(|f| f.path.ends_with("mod.rs")));
	}

	#[test]
	fn skips_missing_modules_gracefully() {
		let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
		let src = dir.path().join("src");
		fs::create_dir_all(&src).unwrap_or_else(|e| panic!("mkdir: {e}"));
		// References a module that doesn't exist on disk.
		fs::write(src.join("lib.rs"), "mod nonexistent;\npub fn hello() {}")
			.unwrap_or_else(|e| panic!("write: {e}"));

		let files =
			resolve_crate(&src, &src.join("lib.rs")).unwrap_or_else(|e| panic!("resolve: {e}"));
		// Should still resolve lib.rs, just skip the missing module.
		assert_eq!(files.len(), 1);
	}

	#[test]
	fn skips_inline_modules() {
		let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
		let src = dir.path().join("src");
		fs::create_dir_all(&src).unwrap_or_else(|e| panic!("mkdir: {e}"));
		fs::write(
			src.join("lib.rs"),
			"mod inline { pub fn foo() {} }\npub fn hello() {}",
		)
		.unwrap_or_else(|e| panic!("write: {e}"));

		let files =
			resolve_crate(&src, &src.join("lib.rs")).unwrap_or_else(|e| panic!("resolve: {e}"));
		assert_eq!(files.len(), 1); // Only lib.rs, inline module is part of it.
	}
}
