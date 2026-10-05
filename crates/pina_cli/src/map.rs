//! A self-contained HTML map of a program's instructions and account locks.
//!
//! `pina map` renders the [`crate::locks`] analysis, together with each
//! instruction's docs, arguments, and account constraints, into one HTML file
//! with inline CSS and script and no network access. The data travels as JSON
//! in a `<script type="application/json">` block, escaped so that no string
//! from the program's source can close the block early, and the page renders
//! every string through `textContent`.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use serde::Serialize;

use crate::ir::FieldIr;
use crate::ir::InstructionIr;
use crate::ir::ProgramIr;
use crate::locks::InstructionLocks;
use crate::locks::LockReport;
use crate::locks::LocksError;

const MAP_TEMPLATE: &str = include_str!("../templates/map.html");
const TITLE_PLACEHOLDER: &str = "@@TITLE@@";
const DATA_PLACEHOLDER: &str = "@@DATA@@";

/// The `pina locks` document plus what the map's detail panels show.
///
/// Serialized, it is the locks schema-version-1 document with two more
/// top-level fields, `instructionDetails` and `accountTypes`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ProgramMap {
	#[serde(flatten)]
	pub locks: LockReport,
	/// Every instruction in declaration order.
	pub instruction_details: Vec<InstructionDetail>,
	/// Every `#[account]` type, with the PDA it declares.
	pub account_types: Vec<AccountTypeDetail>,
}

/// An instruction's docs, arguments, and accounts in slot order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct InstructionDetail {
	pub name: String,
	pub docs: Vec<String>,
	pub arguments: Vec<FieldDetail>,
	pub accounts: Vec<SlotDetail>,
}

/// One account slot of an instruction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SlotDetail {
	pub slot: String,
	/// The [`crate::locks::AccountNode::id`] the slot locks.
	pub node: String,
	pub writable: bool,
	pub signer: bool,
	pub optional: bool,
	/// The PDA the slot's account belongs to.
	pub pda: Option<String>,
	/// Declarative `#[pina(...)]` constraints on the slot.
	pub constraints: Vec<String>,
	pub docs: Vec<String>,
}

/// A named, typed field of an instruction payload or account.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct FieldDetail {
	pub name: String,
	#[serde(rename = "type")]
	pub rust_type: String,
	pub docs: Vec<String>,
}

/// An `#[account]` type and the PDA its `#[pda]` attribute declares.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AccountTypeDetail {
	pub name: String,
	pub pda: Option<String>,
	pub docs: Vec<String>,
	pub fields: Vec<FieldDetail>,
}

/// A project's map and where `pina map` writes it by default.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ProjectMap {
	pub map: ProgramMap,
	/// `<target>/pina/map.html` under the project's Cargo target directory.
	pub default_output: PathBuf,
}

/// Errors produced while writing a map.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MapError {
	#[error("Could not serialize the program map: {0}")]
	Json(#[from] serde_json::Error),

	#[error("Could not write the program map to {path}: {source}")]
	Write {
		path: PathBuf,
		source: std::io::Error,
	},
}

/// Discover the project at or above `start` and build its map.
///
/// # Errors
///
/// Returns the same errors as [`crate::locks::analyze_project`].
pub fn map_project(start: &Path) -> Result<ProjectMap, LocksError> {
	let loaded = crate::locks::load_project(start)?;

	Ok(ProjectMap {
		map: build_map(&loaded.ir, &loaded.allow)?,
		default_output: loaded.project.target_dir.join("pina").join("map.html"),
	})
}

/// Build the map of `ir`, honoring the `allow` list like
/// [`crate::locks::analyze`].
///
/// # Errors
///
/// Returns the same errors as [`crate::locks::analyze`].
pub fn build_map(ir: &ProgramIr, allow: &[String]) -> Result<ProgramMap, LocksError> {
	let locks = crate::locks::analyze(ir, allow)?;
	let instruction_details = ir
		.instructions
		.iter()
		.zip(&locks.instructions)
		.map(|(instruction, locked)| instruction_detail(instruction, locked))
		.collect();
	let account_types = ir
		.accounts
		.iter()
		.map(|account| {
			AccountTypeDetail {
				name: account.name.clone(),
				pda: account.pda_name.clone(),
				docs: account.visible_docs(),
				fields: account.fields.iter().map(field_detail).collect(),
			}
		})
		.collect();

	Ok(ProgramMap {
		locks,
		instruction_details,
		account_types,
	})
}

fn instruction_detail(instruction: &InstructionIr, locked: &InstructionLocks) -> InstructionDetail {
	let nodes = locked
		.writes
		.iter()
		.chain(&locked.reads)
		.map(|lock| (lock.slot.as_str(), lock.node.as_str()))
		.collect::<HashMap<_, _>>();

	InstructionDetail {
		name: instruction.name.clone(),
		docs: instruction.visible_docs(),
		arguments: instruction.arguments.iter().map(field_detail).collect(),
		accounts: instruction
			.accounts
			.iter()
			.map(|account| {
				SlotDetail {
					slot: account.name.clone(),
					node: nodes
						.get(account.name.as_str())
						.copied()
						.unwrap_or_default()
						.to_owned(),
					writable: account.is_writable,
					signer: account.is_signer,
					optional: account.is_optional,
					pda: account.pda_name.clone(),
					constraints: account.constraints.clone(),
					docs: account.docs.clone(),
				}
			})
			.collect(),
	}
}

fn field_detail(field: &FieldIr) -> FieldDetail {
	FieldDetail {
		name: field.name.clone(),
		rust_type: field.rust_type.clone(),
		docs: field.docs.clone(),
	}
}

/// Render `map` as one self-contained HTML document.
///
/// The output is deterministic: the same map always renders the same bytes.
///
/// # Errors
///
/// Returns an error when the map cannot be serialized.
pub fn render_html(map: &ProgramMap) -> Result<String, serde_json::Error> {
	let json = serde_json::to_string(map)?;

	Ok(MAP_TEMPLATE
		.replacen(TITLE_PLACEHOLDER, &escape_html(&map.locks.program), 1)
		.replacen(DATA_PLACEHOLDER, &escape_script_json(&json), 1))
}

/// Render `map` and write it to `path`, creating missing parent directories.
///
/// # Errors
///
/// Returns an error when the map cannot be serialized or the file cannot be
/// written.
pub fn write_html(map: &ProgramMap, path: &Path) -> Result<(), MapError> {
	let html = render_html(map)?;
	// `create_dir_all` accepts the empty parent of a bare file name.
	let parent = path.parent().unwrap_or_else(|| Path::new(""));

	fs::create_dir_all(parent).map_err(|source| {
		MapError::Write {
			path: parent.to_path_buf(),
			source,
		}
	})?;
	fs::write(path, html).map_err(|source| {
		MapError::Write {
			path: path.to_path_buf(),
			source,
		}
	})
}

/// Escape JSON for an HTML `<script>` element.
///
/// `<`, `>`, and `&` only occur inside JSON strings, where their `\u` escapes
/// decode to the same text, so no string can spell `</script>` or open a
/// comment. U+2028 and U+2029 are escaped because older JavaScript parsers
/// treat them as line terminators.
fn escape_script_json(json: &str) -> String {
	let mut escaped = String::with_capacity(json.len());

	for character in json.chars() {
		match character {
			'<' => escaped.push_str("\\u003c"),
			'>' => escaped.push_str("\\u003e"),
			'&' => escaped.push_str("\\u0026"),
			'\u{2028}' => escaped.push_str("\\u2028"),
			'\u{2029}' => escaped.push_str("\\u2029"),
			other => escaped.push(other),
		}
	}

	escaped
}

/// Escape text for HTML element content and attribute values.
fn escape_html(text: &str) -> String {
	let mut escaped = String::with_capacity(text.len());

	for character in text.chars() {
		match character {
			'&' => escaped.push_str("&amp;"),
			'<' => escaped.push_str("&lt;"),
			'>' => escaped.push_str("&gt;"),
			'"' => escaped.push_str("&quot;"),
			'\'' => escaped.push_str("&#39;"),
			other => escaped.push(other),
		}
	}

	escaped
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ir::AccountIr;
	use crate::ir::DiscriminatorIr;
	use crate::ir::InstructionAccountIr;
	use crate::ir::PdaIr;
	use crate::ir::PdaSeedIr;

	const HOSTILE: &str = "</script><script>alert(1)</script><!-- & \u{2028}\u{2029} 'q' \"q\"";

	fn discriminator() -> DiscriminatorIr {
		DiscriminatorIr {
			value: 1,
			repr_size: 1,
		}
	}

	fn field(name: &str, rust_type: &str, docs: &[&str]) -> FieldIr {
		FieldIr {
			name: name.to_owned(),
			rust_type: rust_type.to_owned(),
			docs: docs.iter().map(|doc| (*doc).to_owned()).collect(),
		}
	}

	fn account(name: &str, is_writable: bool, pda_name: Option<&str>) -> InstructionAccountIr {
		InstructionAccountIr {
			name: name.to_owned(),
			is_writable,
			is_signer: !is_writable,
			is_optional: false,
			default_value: None,
			is_pda: pda_name.is_some(),
			pda_name: pda_name.map(str::to_owned),
			constraints: vec!["owner=ID".to_owned()],
			docs: vec![HOSTILE.to_owned()],
		}
	}

	fn program() -> ProgramIr {
		ProgramIr {
			name: "map<&>program".to_owned(),
			public_key: "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS".to_owned(),
			pinapod_enums: Vec::new(),
			accounts: vec![AccountIr {
				name: "ConfigState".to_owned(),
				fields: vec![field("bump", "u8", &[])],
				discriminator: discriminator(),
				docs: vec![
					HOSTILE.to_owned(),
					crate::ir::COMPACT_ACCOUNT_DOC_MARKER.to_owned(),
				],
				pda_name: Some("config".to_owned()),
			}],
			instructions: vec![InstructionIr {
				name: "update".to_owned(),
				rust_name: "UpdateInstruction".to_owned(),
				accounts: vec![
					account("authority", false, None),
					account("config", true, Some("config")),
				],
				arguments: vec![field("value", "u64", &["The new value."])],
				discriminator: discriminator(),
				docs: vec![
					"Update the config.".to_owned(),
					crate::ir::MIGRATABLE_DOC_MARKER.to_owned(),
				],
			}],
			events: Vec::new(),
			errors: Vec::new(),
			pdas: vec![PdaIr {
				name: "config".to_owned(),
				seeds: vec![PdaSeedIr::Constant {
					value: b"config".to_vec(),
				}],
			}],
		}
	}

	fn map() -> ProgramMap {
		let map = build_map(&program(), &[]);

		map.unwrap_or_else(|error| panic!("map failed: {error}"))
	}

	/// The JSON text of the page's data block.
	fn data_block(html: &str) -> &str {
		let start = html.find("id=\"pina-map-data\">").map(|index| index + 19);
		let start = start.unwrap_or_else(|| panic!("the page has a data block"));
		let length = html[start..].find("</script>");

		&html[start..start + length.unwrap_or_else(|| panic!("the data block is closed"))]
	}

	#[test]
	fn builds_details_without_internal_doc_markers() {
		let map = map();
		let detail = &map.instruction_details[0];

		assert_eq!(detail.docs, ["Update the config."]);
		assert_eq!(detail.arguments[0].rust_type, "u64");
		assert_eq!(detail.accounts[0].node, "caller:update.authority");
		assert!(detail.accounts[0].signer);
		assert_eq!(detail.accounts[1].node, "pda:config");
		assert_eq!(detail.accounts[1].pda.as_deref(), Some("config"));
		assert_eq!(detail.accounts[1].constraints, ["owner=ID"]);
		assert_eq!(map.account_types[0].docs, [HOSTILE]);
		assert_eq!(map.account_types[0].pda.as_deref(), Some("config"));
		assert_eq!(map.account_types[0].fields[0].name, "bump");
		assert_eq!(map.locks.hotspots[0].name, "config");
	}

	#[test]
	fn json_extends_the_locks_document() {
		let json = serde_json::to_value(map());
		let json = json.unwrap_or_else(|error| panic!("map must serialize: {error}"));

		assert_eq!(json["schemaVersion"], 1);
		assert_eq!(json["hotspots"][0]["node"], "pda:config");
		assert_eq!(
			json["instructionDetails"][0]["accounts"][1]["writable"],
			true
		);
		assert_eq!(json["instructionDetails"][0]["arguments"][0]["type"], "u64");
		assert_eq!(json["accountTypes"][0]["name"], "ConfigState");
	}

	#[test]
	fn renders_a_self_contained_page_that_no_string_can_escape() {
		let map = map();
		let html = render_html(&map).unwrap_or_else(|error| panic!("render failed: {error}"));

		assert!(!html.contains(TITLE_PLACEHOLDER));
		assert!(!html.contains(DATA_PLACEHOLDER));
		assert!(html.contains("<title>map&lt;&amp;&gt;program · interlocking chart</title>"));
		assert_eq!(
			html.matches("</script>").count(),
			2,
			"only the template closes scripts"
		);
		assert!(!html.contains("<script>alert(1)"));
		assert!(!html.contains('\u{2028}') && !html.contains('\u{2029}'));
		assert!(!html.contains("http://") && !html.contains("https://"));

		let block = data_block(&html);
		assert!(block.contains("\\u003c/script\\u003e"), "{block}");
		assert!(block.contains("\\u2028\\u2029"), "{block}");

		let decoded = serde_json::from_str::<serde_json::Value>(block);
		let decoded = decoded.unwrap_or_else(|error| panic!("data block must stay JSON: {error}"));
		let expected = serde_json::to_value(&map);
		assert_eq!(
			decoded,
			expected.unwrap_or_else(|error| panic!("map must serialize: {error}"))
		);
	}

	#[test]
	fn rendering_is_deterministic() {
		let first = render_html(&map()).unwrap_or_else(|error| panic!("render failed: {error}"));
		let second = render_html(&map()).unwrap_or_else(|error| panic!("render failed: {error}"));

		assert_eq!(first, second);
	}

	#[test]
	fn escapes_every_html_special_character() {
		assert_eq!(
			escape_html("<a href=\"x\">'&'</a>"),
			"&lt;a href=&quot;x&quot;&gt;&#39;&amp;&#39;&lt;/a&gt;"
		);
		assert_eq!(
			escape_script_json("<>&\u{2028}\u{2029}é"),
			"\\u003c\\u003e\\u0026\\u2028\\u2029é"
		);
	}

	#[test]
	fn writes_the_page_and_reports_unwritable_paths() {
		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let map = map();
		let path = temp.path().join("nested/dir/map.html");

		write_html(&map, &path).unwrap_or_else(|error| panic!("write failed: {error}"));
		let written =
			fs::read_to_string(&path).unwrap_or_else(|error| panic!("read failed: {error}"));
		assert!(written.contains("id=\"pina-map-data\""));

		let blocker = temp.path().join("blocker");
		fs::write(&blocker, "").unwrap_or_else(|error| panic!("write failed: {error}"));
		let error = write_html(&map, &blocker.join("map.html"))
			.expect_err("a file cannot be a parent directory");
		assert!(
			matches!(&error, MapError::Write { path, .. } if *path == blocker),
			"{error}"
		);

		let error = write_html(&map, temp.path()).expect_err("a directory cannot be overwritten");
		assert!(
			error
				.to_string()
				.contains("Could not write the program map"),
			"{error}"
		);
	}
}
