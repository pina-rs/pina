//! Module resolution for multi-file Pina programs.
//!
//! Follows `mod` declarations from `src/lib.rs` to discover all source files
//! in a program crate, parsing each into a `syn::File`.

use std::path::Path;
use std::path::PathBuf;

use rayon::prelude::*;

use crate::error::IdlError;

/// A resolved source file with its parsed AST.
#[derive(Debug)]
pub struct ResolvedFile {
	/// Path to the source file (relative or absolute).
	pub path: PathBuf,
	/// Parsed AST.
	pub file: syn::File,
}

#[derive(Debug)]
struct PendingModule {
	path: PathBuf,
	child_base_dir: PathBuf,
}

/// Resolve all source files in a crate starting from `lib.rs`.
///
/// Reads sibling modules for each discovery depth in parallel, then parses
/// them deterministically to discover nested `mod` declarations. Each file is
/// read once, and I/O or parse failures are surfaced as `IdlError`s.
pub fn resolve_crate(src_dir: &Path, lib_path: &Path) -> Result<Vec<ResolvedFile>, IdlError> {
	let source = std::fs::read_to_string(lib_path).map_err(|e| IdlError::io(lib_path, e))?;
	let file = syn::parse_file(&source).map_err(|e| IdlError::parse(lib_path, &e))?;

	let mut seen = vec![lib_path.to_path_buf()];
	let mut files = vec![ResolvedFile {
		path: lib_path.to_path_buf(),
		file,
	}];
	let mut pending = discover_module_paths(src_dir, &files[0].file, &mut seen);

	while !pending.is_empty() {
		let read_modules: Vec<(PendingModule, String)> = pending
			.into_par_iter()
			.map(|module| {
				let source = std::fs::read_to_string(&module.path)
					.map_err(|e| IdlError::io(&module.path, e))?;
				Ok((module, source))
			})
			.collect::<Result<_, IdlError>>()?;

		let mut next_pending = Vec::new();

		for (module, source) in read_modules {
			let file = syn::parse_file(&source).map_err(|e| IdlError::parse(&module.path, &e))?;
			next_pending.extend(discover_module_paths(
				&module.child_base_dir,
				&file,
				&mut seen,
			));
			files.push(ResolvedFile {
				path: module.path,
				file,
			});
		}

		pending = next_pending;
	}

	Ok(files)
}

/// Discover file-based `mod` declarations in a parsed file.
fn discover_module_paths(
	base_dir: &Path,
	file: &syn::File,
	seen: &mut Vec<PathBuf>,
) -> Vec<PendingModule> {
	let mut modules = Vec::new();

	for item in &file.items {
		let syn::Item::Mod(item_mod) = item else {
			continue;
		};

		// Inline modules are already in the parent file's AST.
		if item_mod.content.is_some() {
			continue;
		}

		let mod_name = item_mod.ident.to_string();
		let candidates = [
			base_dir.join(format!("{mod_name}.rs")),
			base_dir.join(&mod_name).join("mod.rs"),
		];

		let Some(mod_path) = candidates.iter().find(|p| p.is_file()) else {
			// Missing module files are skipped to preserve support for cfg-gated or
			// externally-provided modules.
			continue;
		};

		if seen.contains(mod_path) {
			continue;
		}

		seen.push(mod_path.clone());

		let child_base_dir = if mod_path.file_name().is_some_and(|n| n == "mod.rs") {
			mod_path.parent().unwrap_or(base_dir).to_path_buf()
		} else {
			base_dir.join(&mod_name)
		};

		modules.push(PendingModule {
			path: mod_path.clone(),
			child_base_dir,
		});
	}

	modules
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
