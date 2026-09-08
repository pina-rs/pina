//! Keeps the embedded CLI catalog (`crates/pina_cli/lints.json`) in sync with
//! the lints registered by this crate.
//!
//! The comparison runs against the catalog printed by the bundled
//! `pina_lint_driver` (`PINA_LINT_LIST=1`), so the check exercises the same
//! binary the CLI lint runs use instead of linking this crate a second time.
//!
//! The test runs inside the repository workspace; when the crate is used
//! outside the repository (for example from a crates.io checkout) the sibling
//! file is absent and the test is skipped.

use std::path::PathBuf;
use std::process::Command;

use serde::Deserialize;

/// The sysroot of the toolchain pinned by the workspace's `rust-toolchain.toml`.
///
/// The driver links against that toolchain's `rustc_driver` library, and the
/// spawned process must be able to load it regardless of the calling
/// environment.
fn pinned_sysroot() -> PathBuf {
	let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	let workspace_root = manifest
		.ancestors()
		.nth(2)
		.expect("crate manifest must have a workspace root");

	let output = Command::new("rustc")
		.arg("--print")
		.arg("sysroot")
		.current_dir(workspace_root)
		.output()
		.expect("could not resolve the pinned toolchain sysroot");
	assert!(
		output.status.success(),
		"rustc --print sysroot failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);

	let sysroot = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
	assert!(
		sysroot.is_dir(),
		"sysroot is missing: {}",
		sysroot.display()
	);
	sysroot
}

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

/// Parse the lint catalog printed by the driver.
///
/// Every catalog line is `\t<name>    <level>    <description>`; lines outside
/// the table (headers, blank lines) do not parse as name/level pairs and are
/// skipped.
fn parse_driver_catalog(output: &str) -> Vec<(String, String)> {
	output
		.lines()
		.filter_map(|line| {
			let mut fields = line.split_whitespace();
			let name = fields.next()?.to_owned();
			let level = fields.next()?.to_owned();
			fields.next()?;
			Some((name, level))
		})
		.collect()
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

	let sysroot = pinned_sysroot();
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina_lint_driver"));
	command
		.arg("rustc")
		.env("PINA_LINT_LIST", "1")
		.env("DYLD_LIBRARY_PATH", sysroot.join("lib"))
		.env("LD_LIBRARY_PATH", sysroot.join("lib"));
	let output = command
		.output()
		.expect("could not run the bundled lint driver");
	assert!(
		output.status.success(),
		"the lint driver must print its catalog: {}",
		String::from_utf8_lossy(&output.stderr)
	);

	let registered = parse_driver_catalog(&String::from_utf8_lossy(&output.stdout));
	let names = registered
		.iter()
		.map(|(name, _)| name.clone())
		.collect::<Vec<_>>();
	assert_eq!(
		names,
		catalog
			.lints
			.iter()
			.map(|entry| entry.name.clone())
			.collect::<Vec<_>>(),
		"the CLI catalog must list exactly the lints registered by this crate, in catalog order"
	);

	for ((name, level), entry) in registered.iter().zip(catalog.lints.iter()) {
		assert_eq!(
			name, &entry.name,
			"catalog entry order must match the registered lint order"
		);
		assert_eq!(
			level, &entry.level,
			"catalog level for `{name}` must match its default level"
		);
	}
}
