//! Keeps the embedded CLI catalog (`crates/pina_cli/lints.json`) in sync with
//! the lints registered by this crate.
//!
//! The test runs inside the repository workspace; when the crate is used
//! outside the repository (for example from a crates.io checkout) the sibling
//! file is absent and the test is skipped.

// Reading `pina_lints` items links against the compiler's `rustc_driver`
// dylib, exactly like the crate itself.
#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct CatalogEntry {
	name: String,
	level: String,
}

#[derive(Debug, Deserialize)]
struct CatalogFile {
	#[serde(rename = "schemaVersion")]
	_schema_version: u8,
	lints: Vec<CatalogEntry>,
}

#[test]
fn embedded_cli_catalog_matches_the_registered_lints() {
	let catalog_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("..")
		.join("pina_cli")
		.join("lints.json");
	let Ok(source) = std::fs::read_to_string(&catalog_path) else {
		// Not running inside the repository workspace.
		return;
	};

	let catalog: CatalogFile = serde_json::from_str(&source)
		.unwrap_or_else(|error| panic!("lints.json must be valid JSON: {error}"));

	let names = catalog
		.lints
		.iter()
		.map(|entry| entry.name.clone())
		.collect::<Vec<_>>();
	assert_eq!(
		names,
		pina_lints::LINT_NAMES,
		"the CLI catalog must list exactly the lints registered by this crate, in catalog order"
	);

	for (entry, lint) in catalog.lints.iter().zip(pina_lints::catalog()) {
		assert_eq!(
			entry.name, lint.name,
			"catalog entry order must match the registered lint order"
		);
		assert_eq!(
			entry.level, lint.default_level,
			"catalog level for `{}` must match its default level",
			entry.name
		);
	}
}
