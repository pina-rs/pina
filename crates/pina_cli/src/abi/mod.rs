//! The `pina abi schema` command: print the JSON Schema of an ABI document.
//!
//! The schema is generated from the same types that read and write the
//! document, so it cannot drift from the code enforcing it. A consumer in any
//! language can validate a checked-in `migrations/manifest.json` or
//! `migrations/publications.json` against the printed schema instead of
//! trusting its bytes.

use pina_abi::ABI_OLDEST_SUPPORTED;
use pina_abi::ABI_VERSION;
use pina_abi::AbiDocument;
use pina_abi::parse_document_version;
use pina_abi::render_document_schema;

/// Resolve a requested document kind from the `--document` value.
fn document_kind(value: &str) -> Result<AbiDocument, String> {
	AbiDocument::parse(value).ok_or_else(|| {
		format!("unknown ABI document `{value}`; expected `manifest` or `publications`")
	})
}

/// Render the schema for one document at the requested version.
///
/// Only the version this build writes can be described: an older version's
/// fields are not representable by the current types, and printing today's
/// shape under an older version would be a lie about what that release wrote.
/// The error names the supported version and the frozen fixture that records
/// the historical shape.
pub fn render_schema(document: &str, version: Option<&str>) -> Result<String, String> {
	let kind = document_kind(document)?;
	if let Some(requested) = version {
		let parsed = parse_document_version(requested)?;
		let current = parse_document_version(ABI_VERSION)?;
		if parsed != current {
			return Err(format!(
				"ABI version {requested} cannot be printed by this build, which writes \
				 {ABI_VERSION}; version {requested} predates it and its frozen record is checked \
				 in under the `v{requested}` fixture directory"
			));
		}
	}
	render_document_schema(kind)
}

/// Run the command, writing the schema to stdout.
///
/// Returns the process exit code.
pub fn run_abi_schema(document: &str, version: Option<&str>) -> i32 {
	match render_schema(document, version) {
		Ok(schema) => {
			print!("{schema}");
			0
		}
		Err(reason) => {
			eprintln!("Error {reason}");
			eprintln!(
				"Available: `--document manifest|publications`, `--version {ABI_VERSION}` (oldest \
				 supported: {ABI_OLDEST_SUPPORTED})."
			);
			1
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn schema_renders_for_both_documents_at_the_current_version() {
		for document in ["manifest", "publications"] {
			let rendered =
				render_schema(document, None).unwrap_or_else(|error| panic!("{document}: {error}"));
			assert!(rendered.ends_with('\n'), "output must be redirect-safe");
			// The schema names the release that wrote it and the permanent URL
			// where it is published, so a consumer can cite a stable address.
			assert!(
				rendered.contains(&format!(
					"https://pina-rs.github.io/pina/abi/schemas/{ABI_VERSION}/{document}.schema.json"
				)),
				"the schema must carry its published URL"
			);
			let explicit = render_schema(document, Some(ABI_VERSION))
				.unwrap_or_else(|error| panic!("{document} at {ABI_VERSION}: {error}"));
			assert_eq!(rendered, explicit);
		}
	}

	#[test]
	fn schema_rejects_unknown_documents_and_versions() {
		let error = render_schema("other", None).unwrap_err();
		assert!(error.contains("unknown ABI document"));

		let error = render_schema("manifest", Some("0.1")).unwrap_err();
		assert!(error.contains("cannot be printed"), "got: {error}");

		let error = render_schema("manifest", Some("not-a-version")).unwrap_err();
		assert!(error.contains("invalid ABI version"));
	}

	#[test]
	fn schema_command_exit_codes_are_stable() {
		assert_eq!(run_abi_schema("manifest", None), 0);
		assert_eq!(run_abi_schema("other", None), 1);
	}
}
