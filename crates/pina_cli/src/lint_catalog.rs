//! Embedded Pina lint catalog.
//!
//! The catalog mirrors `pina_lints::LINT_NAMES` and the default levels
//! declared by each lint. The JSON file is the single source of truth shared
//! with the CLI; `pina_lints` keeps a test asserting that its registered
//! lints match this file.

use std::sync::OnceLock;

use serde::Deserialize;

/// The embedded catalog, kept in sync with `pina_lints`.
const CATALOG_JSON: &str = include_str!("../lints.json");

/// A configured lint level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LintLevel {
	Allow,
	Warn,
	Deny,
}

impl LintLevel {
	/// Return the stable configuration spelling.
	#[must_use]
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Allow => "allow",
			Self::Warn => "warn",
			Self::Deny => "deny",
		}
	}
}

/// One entry of the embedded lint catalog.
#[derive(Debug, Deserialize)]
struct CatalogEntry {
	name: String,
	// The default levels are read by this crate's tests and by the parity test
	// in `pina_lints`, which validates its registered lints against this
	// file.
	#[allow(dead_code)]
	level: LintLevel,
}

#[derive(Debug, Deserialize)]
struct CatalogFile {
	#[serde(rename = "schemaVersion")]
	_schema_version: u8,
	lints: Vec<CatalogEntry>,
}

/// The Pina lint catalog embedded in this CLI release.
#[derive(Debug)]
pub struct LintCatalog {
	entries: Vec<CatalogEntry>,
}

impl LintCatalog {
	/// Return the embedded catalog.
	///
	/// # Panics
	///
	/// Panics when the embedded catalog is malformed; the file is part of the
	/// crate and is validated by tests.
	pub fn global() -> &'static Self {
		static CATALOG: OnceLock<LintCatalog> = OnceLock::new();
		CATALOG.get_or_init(|| {
			let file: CatalogFile = serde_json::from_str(CATALOG_JSON).unwrap_or_else(|error| {
				panic!("embedded lint catalog must be valid JSON: {error}")
			});
			let catalog = LintCatalog {
				entries: file.lints,
			};
			assert!(
				!catalog.entries.is_empty(),
				"embedded lint catalog must not be empty"
			);
			catalog
		})
	}

	/// Return the name of every known lint.
	#[must_use]
	pub fn names(&self) -> Vec<&str> {
		self.entries
			.iter()
			.map(|entry| entry.name.as_str())
			.collect()
	}

	/// Return the comma-separated names of every known lint.
	#[must_use]
	pub fn known_lints(&self) -> String {
		self.names().join(", ")
	}

	/// Return whether `name` is a Pina lint shipped with this CLI.
	#[must_use]
	pub fn contains(&self, name: &str) -> bool {
		self.entries.iter().any(|entry| entry.name == name)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn catalog_has_exactly_one_entry_per_pina_lint() {
		let catalog = LintCatalog::global();
		assert_eq!(catalog.entries.len(), 21);

		let mut names = catalog.names();
		names.sort_unstable();
		let mut deduplicated = names.clone();
		deduplicated.dedup();
		assert_eq!(names, deduplicated, "lint names must be unique");
	}

	#[test]
	fn catalog_defaults_match_the_declared_levels() {
		let catalog = LintCatalog::global();
		let warns = catalog
			.entries
			.iter()
			.filter(|entry| entry.level == LintLevel::Warn)
			.map(|entry| entry.name.as_str())
			.collect::<Vec<_>>();
		assert_eq!(
			warns,
			[
				"deny_heap_allocations_in_onchain_instruction_handlers",
				"require_canonical_instruction_dispatch_for_idl",
				"require_explicit_discriminators_and_seed_namespaces",
				"require_idl_root_to_define_one_program_id",
			]
		);
	}
}
