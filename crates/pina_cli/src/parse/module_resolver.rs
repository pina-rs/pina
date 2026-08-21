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
	let mut pending = discover_module_paths(src_dir, &files[0].file, &mut seen)?;

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
			)?);
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
) -> Result<Vec<PendingModule>, IdlError> {
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
		let explicit_path = module_path_attribute(item_mod)?;
		let candidates = explicit_path.as_ref().map_or_else(
			|| {
				vec![
					base_dir.join(format!("{mod_name}.rs")),
					base_dir.join(&mod_name).join("mod.rs"),
				]
			},
			|path| vec![base_dir.join(path)],
		);

		let existing_candidates = candidates
			.iter()
			.filter(|path| path.is_file())
			.collect::<Vec<_>>();
		let Some(mod_path) = existing_candidates.first().copied() else {
			if is_cfg_gated(item_mod) {
				continue;
			}

			let missing_path = candidates
				.first()
				.cloned()
				.unwrap_or_else(|| base_dir.join(format!("{mod_name}.rs")));
			return Err(IdlError::io(
				missing_path,
				std::io::Error::new(
					std::io::ErrorKind::NotFound,
					format!("module `{mod_name}` has no source file"),
				),
			));
		};
		if explicit_path.is_none() && existing_candidates.len() > 1 {
			return Err(IdlError::Other(format!(
				"Module `{mod_name}` is ambiguous; both {} and {} exist",
				existing_candidates[0].display(),
				existing_candidates[1].display(),
			)));
		}

		if seen.contains(mod_path) {
			continue;
		}

		seen.push(mod_path.clone());

		let child_base_dir = if mod_path.file_name().is_some_and(|name| name == "mod.rs") {
			mod_path.parent().unwrap_or(base_dir).to_path_buf()
		} else {
			mod_path.with_extension("")
		};

		modules.push(PendingModule {
			path: mod_path.clone(),
			child_base_dir,
		});
	}

	Ok(modules)
}

fn is_cfg_gated(item_mod: &syn::ItemMod) -> bool {
	item_mod
		.attrs
		.iter()
		.any(|attr| attr.path().is_ident("cfg"))
}

fn module_path_attribute(item_mod: &syn::ItemMod) -> Result<Option<PathBuf>, IdlError> {
	let Some(attr) = item_mod
		.attrs
		.iter()
		.find(|attr| attr.path().is_ident("path"))
	else {
		return Ok(None);
	};

	let syn::Meta::NameValue(name_value) = &attr.meta else {
		return Err(IdlError::Other(format!(
			"Module `{}` has an invalid `#[path]` attribute",
			item_mod.ident
		)));
	};
	let syn::Expr::Lit(syn::ExprLit {
		lit: syn::Lit::Str(path),
		..
	}) = &name_value.value
	else {
		return Err(IdlError::Other(format!(
			"Module `{}` has a non-string `#[path]` attribute",
			item_mod.ident
		)));
	};

	Ok(Some(PathBuf::from(path.value())))
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
	fn rejects_ambiguous_implicit_module_files() {
		let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
		let src = dir.path().join("src");
		let state_dir = src.join("state");
		fs::create_dir_all(&state_dir).unwrap_or_else(|e| panic!("mkdir: {e}"));
		fs::write(src.join("lib.rs"), "mod state;").unwrap_or_else(|e| panic!("write: {e}"));
		fs::write(src.join("state.rs"), "pub struct FlatState;")
			.unwrap_or_else(|e| panic!("write: {e}"));
		fs::write(state_dir.join("mod.rs"), "pub struct NestedState;")
			.unwrap_or_else(|e| panic!("write: {e}"));

		let error = resolve_crate(&src, &src.join("lib.rs"))
			.expect_err("ambiguous implicit module files must fail");
		let message = error.to_string();
		assert!(message.contains("Module `state` is ambiguous"));
		assert!(message.contains("state.rs"));
		assert!(message.contains("state/mod.rs"));
	}

	#[test]
	fn rejects_missing_unconditional_modules() {
		let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
		let src = dir.path().join("src");
		fs::create_dir_all(&src).unwrap_or_else(|e| panic!("mkdir: {e}"));
		// References a module that doesn't exist on disk.
		fs::write(src.join("lib.rs"), "mod nonexistent;\npub fn hello() {}")
			.unwrap_or_else(|e| panic!("write: {e}"));

		let error = resolve_crate(&src, &src.join("lib.rs"))
			.expect_err("missing unconditional modules must fail");
		assert!(matches!(error, IdlError::Io { .. }));
		assert!(error.to_string().contains("nonexistent.rs"));
	}

	#[test]
	fn skips_missing_cfg_gated_modules() {
		let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
		let src = dir.path().join("src");
		fs::create_dir_all(&src).unwrap_or_else(|e| panic!("mkdir: {e}"));
		fs::write(
			src.join("lib.rs"),
			"#[cfg(feature = \"client\")]\nmod client;\npub fn hello() {}",
		)
		.unwrap_or_else(|e| panic!("write: {e}"));

		let files =
			resolve_crate(&src, &src.join("lib.rs")).unwrap_or_else(|e| panic!("resolve: {e}"));
		assert_eq!(files.len(), 1);
	}

	#[test]
	fn resolves_explicit_path_module() {
		let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
		let src = dir.path().join("src");
		let generated = src.join("generated");
		fs::create_dir_all(&generated).unwrap_or_else(|e| panic!("mkdir: {e}"));
		fs::write(
			src.join("lib.rs"),
			"#[path = \"generated/state.rs\"]\nmod state;",
		)
		.unwrap_or_else(|e| panic!("write: {e}"));
		fs::write(generated.join("state.rs"), "pub struct State;")
			.unwrap_or_else(|e| panic!("write: {e}"));

		let files =
			resolve_crate(&src, &src.join("lib.rs")).unwrap_or_else(|e| panic!("resolve: {e}"));
		assert_eq!(files.len(), 2);
		assert!(
			files
				.iter()
				.any(|file| file.path.ends_with("generated/state.rs"))
		);
	}

	#[test]
	fn rejects_malformed_path_attribute() {
		let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
		let src = dir.path().join("src");
		fs::create_dir_all(&src).unwrap_or_else(|e| panic!("mkdir: {e}"));
		fs::write(src.join("lib.rs"), "#[path(\"state.rs\")] mod state;")
			.unwrap_or_else(|e| panic!("write: {e}"));

		let error = resolve_crate(&src, &src.join("lib.rs"))
			.expect_err("malformed path attributes must fail");
		assert!(error.to_string().contains("invalid `#[path]`"));
	}

	#[test]
	fn rejects_non_string_path_attribute() {
		let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
		let src = dir.path().join("src");
		fs::create_dir_all(&src).unwrap_or_else(|e| panic!("mkdir: {e}"));
		fs::write(src.join("lib.rs"), "#[path = 7] mod state;")
			.unwrap_or_else(|e| panic!("write: {e}"));

		let error = resolve_crate(&src, &src.join("lib.rs"))
			.expect_err("non-string path attributes must fail");
		assert!(error.to_string().contains("non-string `#[path]`"));
	}

	#[test]
	fn rejects_malformed_path_attribute_in_child_module() {
		let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
		let src = dir.path().join("src");
		fs::create_dir_all(&src).unwrap_or_else(|e| panic!("mkdir: {e}"));
		fs::write(src.join("lib.rs"), "mod state;").unwrap_or_else(|e| panic!("write: {e}"));
		fs::write(src.join("state.rs"), "#[path(\"nested.rs\")] mod nested;")
			.unwrap_or_else(|e| panic!("write: {e}"));

		let error = resolve_crate(&src, &src.join("lib.rs"))
			.expect_err("malformed nested path attributes must fail");
		assert!(error.to_string().contains("invalid `#[path]`"));
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
