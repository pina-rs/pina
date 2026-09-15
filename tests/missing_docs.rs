//! Proves the workspace docstring-coverage gate rejects what it claims to.
//!
//! The gate is `missing_docs = "warn"` in the root `[workspace.lints.rust]`
//! table, escalated to a hard failure by `lint:clippy` running with
//! `-D warnings`. These tests compile representative fixtures directly with
//! the pinned toolchain under `-D missing_docs` and assert both directions:
//! an undocumented public item fails with the expected message, and the same
//! item with a doc comment passes.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Fixture with an undocumented public item: must be rejected.
const UNDOCUMENTED: &str =
	"pub struct Unpublished {\n\tpub field: u8,\n}\n\npub fn undocumented() -> u8 {\n\t0\n}\n";

/// The same surface with a crate doc and item docs: must compile.
const DOCUMENTED: &str = "//! A documented fixture.\n\n/// A documented structure.\npub struct \
                          Published {\n\t/// A documented field.\n\tpub field: u8,\n}\n\n/// A \
                          documented function.\npub fn documented() -> u8 {\n\t0\n}\n";

/// Fixture asserting the crate-level opt-out the excluded crates rely on.
const ALLOWED: &str = "#![allow(missing_docs)]\n\npub struct Unpublished {\n\tpub field: \
                       u8,\n}\n\npub fn undocumented() -> u8 {\n\t0\n}\n";

/// Compile `source` as a library with `-D missing_docs` and return whether the
/// compilation succeeded.
fn compiles(source: &str, name: &str, scratch: &PathBuf) -> bool {
	let root = scratch.join(name);
	fs::create_dir_all(&root).expect("could not create fixture directory");

	let source_path = root.join(format!("{name}.rs"));
	fs::write(&source_path, source).expect("could not write fixture");

	let output = Command::new("rustc")
		.arg("--edition=2024")
		.arg("--crate-type=lib")
		.arg("--crate-name")
		.arg(name.replace('-', "_"))
		.arg("-D")
		.arg("missing_docs")
		.arg("--emit=metadata")
		.arg("--out-dir")
		.arg(root.join("out"))
		.arg(&source_path)
		.output()
		.expect("could not run rustc");

	output.status.success()
}

#[test]
fn missing_docs_gate_rejects_undocumented_public_items() {
	let scratch = std::env::temp_dir().join("pina-missing-docs-gate");

	assert!(
		!compiles(UNDOCUMENTED, "undocumented", &scratch),
		"an undocumented public item must fail under -D missing_docs"
	);

	assert!(
		compiles(DOCUMENTED, "documented", &scratch),
		"a documented public item must compile under -D missing_docs"
	);
}

#[test]
fn missing_docs_gate_honours_the_crate_level_opt_out() {
	let scratch = std::env::temp_dir().join("pina-missing-docs-gate-allowed");

	assert!(
		compiles(ALLOWED, "allowed", &scratch),
		"the crate-level `#![allow(missing_docs)]` used by the excluded crates must suppress the \
		 gate"
	);
}
