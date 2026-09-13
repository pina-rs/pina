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
const SCAFFOLD: &str =
	"fn main() {\n\tprintln!(\"cargo:rerun-if-changed=migrations/manifest.json\");\n}\n";

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
fn emits_rerun(source: &str) -> bool {
	source.lines().any(|line| line.contains(RERUN_LITERAL))
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
}
