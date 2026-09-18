//! Build-script scaffolding that keeps auto-policy flips fresh.
//!
//! Proc macros do not re-expand when `pina.toml` or the manifest changes, so a
//! program with an auto policy needs a build script that emits
//! `cargo:rerun-if-changed=migrations/manifest.json`. The CLI scaffolds a
//! missing script, verifies an existing one, and never rewrites a hand-written
//! script that is missing the directive: it reports the exact line instead.

use std::path::Path;
use std::path::PathBuf;

use serde::Serialize;

use super::MigrationError;
use super::storage::ensure_safe_path;
use super::storage::write_atomic;

/// Relative path of a program build script.
pub const BUILD_SCRIPT_PATH: &str = "build.rs";

/// Cargo directive that re-expands macros when the manifest changes.
pub const RERUN_DIRECTIVE: &str = "cargo:rerun-if-changed=migrations/manifest.json";

/// The directive as it must appear inside a printed string.
const RERUN_LITERAL: &str = "\"cargo:rerun-if-changed=migrations/manifest.json\"";

/// Canonical build script scaffold written when none exists.
///
/// The crate-level doc comment matters: workspaces that lint with
/// `-D warnings` reject a build script without one, so the scaffold must be
/// warning-clean in the programs it lands in.
const SCAFFOLD: &str = "//! Re-expand Pina macros when the migration manifest changes.\n\nfn \
                        main() {\n\tprintln!(\"cargo:rerun-if-changed=migrations/manifest.json\");\
                        \n}\n";

/// What the current build script says about migration freshness.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "action")]
pub enum BuildScriptStatus {
	/// A missing build script was scaffolded.
	Created {
		/// Path of the written build script.
		path: PathBuf,
	},
	/// The existing build script already emits the directive.
	Verified {
		/// Path of the verified build script.
		path: PathBuf,
	},
	/// An existing build script must be edited by hand.
	Manual {
		/// Path of the existing build script.
		path: PathBuf,
		/// Exact line the developer must add.
		directive: &'static str,
	},
}

/// Scaffold a missing build script or verify an existing one.
///
/// The operation is idempotent: a script that already emits the directive is
/// reported as [`BuildScriptStatus::Verified`] and a hand-written script
/// without it is reported as [`BuildScriptStatus::Manual`] without being
/// touched.
pub(super) fn ensure_build_script(program_dir: &Path) -> Result<BuildScriptStatus, MigrationError> {
	let path = program_dir.join(BUILD_SCRIPT_PATH);
	ensure_safe_path(&path)?;

	match std::fs::read_to_string(&path) {
		Ok(source) => {
			Ok(if emits_rerun(&source) {
				BuildScriptStatus::Verified { path }
			} else {
				BuildScriptStatus::Manual {
					path,
					directive: RERUN_DIRECTIVE,
				}
			})
		}
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
			write_atomic(&path, SCAFFOLD.as_bytes())?;
			Ok(BuildScriptStatus::Created { path })
		}
		Err(source) => Err(MigrationError::Read { path, source }),
	}
}

/// Require that the program build script emits the manifest rerun directive.
pub(super) fn verify_build_script(program_dir: &Path) -> Result<(), MigrationError> {
	let path = program_dir.join(BUILD_SCRIPT_PATH);
	let missing = match std::fs::read_to_string(&path) {
		Ok(source) => !emits_rerun(&source),
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
		Err(source) => return Err(MigrationError::Read { path, source }),
	};
	if !missing {
		return Ok(());
	}

	Err(MigrationError::BuildScriptRerunMissing {
		path,
		directive: RERUN_DIRECTIVE,
	})
}

/// Whether a build script prints the manifest rerun directive.
///
/// Deliberately conservative: the directive must sit inside a single-line
/// `println!`/`print!` call and outside a line comment, which is the only
/// spelling that unambiguously reaches cargo. Spellings that split the call
/// across lines, build the string at runtime, or route it through another
/// macro are reported as manual work, so verification never accepts a
/// directive cargo cannot see.
fn emits_rerun(source: &str) -> bool {
	source.lines().any(|line| {
		// Anything after `//` on the line cannot emit the directive.
		let code = line.split_once("//").map_or(line, |(code, _comment)| code);
		code.contains(RERUN_LITERAL) && (code.contains("println!(") || code.contains("print!("))
	})
}

#[cfg(test)]
mod tests {
	use tempfile::TempDir;

	use super::*;

	/// Canonicalize so path-safety checks do not reject the macOS temp root.
	fn program_dir() -> (TempDir, PathBuf) {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
		let root = std::fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonical temp dir: {error}"));
		(temp, root)
	}

	#[test]
	fn scaffolds_a_missing_build_script_once() {
		let (_temp, root) = program_dir();
		let created = ensure_build_script(&root)
			.unwrap_or_else(|error| panic!("scaffold build script: {error}"));

		assert!(matches!(created, BuildScriptStatus::Created { .. }));
		let source = std::fs::read_to_string(root.join(BUILD_SCRIPT_PATH))
			.unwrap_or_else(|error| panic!("read scaffold: {error}"));
		assert_eq!(source, SCAFFOLD);

		// A second run verifies the scaffold instead of rewriting it.
		let verified = ensure_build_script(&root)
			.unwrap_or_else(|error| panic!("verify build script: {error}"));
		assert!(matches!(verified, BuildScriptStatus::Verified { .. }));
		verify_build_script(&root).unwrap_or_else(|error| panic!("verify: {error}"));
	}

	#[test]
	fn reports_the_required_directive_without_clobbering_handwritten_scripts() {
		let (_temp, root) = program_dir();
		let path = root.join(BUILD_SCRIPT_PATH);
		let handwritten = "fn main() {\n\tprintln!(\"cargo:rustc-cfg=custom\");\n}\n";
		std::fs::write(&path, handwritten)
			.unwrap_or_else(|error| panic!("write build script: {error}"));

		let status = ensure_build_script(&root)
			.unwrap_or_else(|error| panic!("inspect build script: {error}"));
		let BuildScriptStatus::Manual { directive, .. } = status else {
			panic!("a hand-written script without the directive must be manual");
		};
		assert_eq!(directive, RERUN_DIRECTIVE);
		assert_eq!(
			std::fs::read_to_string(&path)
				.unwrap_or_else(|error| panic!("read build script: {error}")),
			handwritten,
		);

		let error = verify_build_script(&root).expect_err("missing rerun directive must fail");
		assert!(matches!(
			error,
			MigrationError::BuildScriptRerunMissing { .. }
		));
	}

	#[test]
	fn accepting_a_comment_spelling_would_be_stale() {
		let (_temp, root) = program_dir();
		std::fs::write(
			root.join(BUILD_SCRIPT_PATH),
			"fn main() {\n\t// cargo:rerun-if-changed=migrations/manifest.json\n}\n",
		)
		.unwrap_or_else(|error| panic!("write build script: {error}"));

		assert!(matches!(
			ensure_build_script(&root)
				.unwrap_or_else(|error| panic!("inspect build script: {error}")),
			BuildScriptStatus::Manual { .. }
		));
	}

	#[test]
	fn dead_string_and_commented_invocations_do_not_verify() {
		let (_temp, root) = program_dir();
		let path = root.join(BUILD_SCRIPT_PATH);
		std::fs::write(
			&path,
			"fn main() {\n\tlet _ = \"cargo:rerun-if-changed=migrations/manifest.json\";\n\t// \
			 println!(\"cargo:rerun-if-changed=migrations/manifest.json\");\n}\n",
		)
		.unwrap_or_else(|error| panic!("write build script: {error}"));

		// A dead string never reaches cargo, and a commented-out `println!` is
		// equally inert, so both must stay manual instead of verifying.
		assert!(matches!(
			ensure_build_script(&root)
				.unwrap_or_else(|error| panic!("inspect build script: {error}")),
			BuildScriptStatus::Manual { .. }
		));
		assert!(matches!(
			verify_build_script(&root),
			Err(MigrationError::BuildScriptRerunMissing { .. })
		));
		assert_eq!(
			std::fs::read_to_string(&path)
				.unwrap_or_else(|error| panic!("read build script: {error}")),
			"fn main() {\n\tlet _ = \"cargo:rerun-if-changed=migrations/manifest.json\";\n\t// \
			 println!(\"cargo:rerun-if-changed=migrations/manifest.json\");\n}\n",
		);
	}

	#[test]
	fn multiline_rerun_spellings_are_reported_as_manual() {
		let (_temp, root) = program_dir();
		let handwritten = r#"fn main() {
	println!(
		"cargo:rerun-if-changed=migrations/manifest.json"
	);
}
"#;
		std::fs::write(root.join(BUILD_SCRIPT_PATH), handwritten)
			.unwrap_or_else(|error| panic!("write build script: {error}"));

		// The heuristic only recognizes single-line invocations; a multi-line
		// spelling is reported as manual so the exact line is still required.
		let status = ensure_build_script(&root)
			.unwrap_or_else(|error| panic!("inspect build script: {error}"));
		let BuildScriptStatus::Manual { directive, .. } = status else {
			panic!("a multi-line spelling must not verify");
		};
		assert_eq!(directive, RERUN_DIRECTIVE);
		assert_eq!(
			std::fs::read_to_string(root.join(BUILD_SCRIPT_PATH))
				.unwrap_or_else(|error| panic!("read build script: {error}")),
			handwritten,
		);
	}
}
