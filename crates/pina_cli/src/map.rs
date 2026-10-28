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
use std::io;
use std::io::Write;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

#[cfg(not(windows))]
use cap_fs_ext::DirExt;
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use cap_std::fs::File;
use cap_std::fs::OpenOptions;
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
	Write { path: PathBuf, source: io::Error },
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

/// Render `map` and atomically replace `path`, creating missing directories.
/// Symbolic links and reparse points in the output path are refused.
///
/// # Errors
///
/// Returns an error when serialization, safe path traversal, or publication fails.
pub fn write_html(map: &ProgramMap, path: &Path) -> Result<(), MapError> {
	let html = render_html(map)?;
	let publish = || -> io::Result<()> {
		if crate::path_security::has_untrusted_link_component(path)? {
			return Err(io::Error::other(
				"output path contains a symbolic link or reparse point",
			));
		}

		let name = path.file_name().ok_or_else(|| {
			io::Error::new(io::ErrorKind::InvalidInput, "output path must name a file")
		})?;
		let parent = open_map_parent(path)?;

		publish_html(&parent, name, html.as_bytes())
	};

	publish().map_err(|source| {
		MapError::Write {
			path: path.to_path_buf(),
			source,
		}
	})
}

fn open_map_parent(path: &Path) -> io::Result<Dir> {
	let absolute = std::path::absolute(path)?;
	let parent = absolute.parent().ok_or_else(|| {
		io::Error::new(
			io::ErrorKind::InvalidInput,
			"output path must have a parent",
		)
	})?;
	let root: PathBuf = parent
		.components()
		.take_while(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
		.collect();
	let mut directory = Dir::open_ambient_dir(&root, ambient_authority())?;
	let mut first = true;

	// Pin each component before looking up the next; checking a path and then
	// opening it normally would let a swapped ancestor redirect publication.
	for component in parent.components() {
		match component {
			Component::Prefix(_) | Component::RootDir | Component::CurDir => continue,
			Component::ParentDir => directory = directory.open_parent_dir(ambient_authority())?,
			Component::Normal(name) => {
				// Only root can replace these system aliases (e.g. macOS /tmp).
				// Everything else, including a missing entry, takes the no-follow path.
				if first
					&& let Ok(metadata) = fs::symlink_metadata(root.join(name))
					&& crate::path_security::is_link_like(&metadata)
					&& !crate::path_security::is_untrusted_link(&metadata, true)
				{
					directory = Dir::open_ambient_dir(root.join(name), ambient_authority())?;
					first = false;
					continue;
				}

				directory = match open_map_directory(&directory, name) {
					Ok(child) => child,
					Err(error) if error.kind() == io::ErrorKind::NotFound => {
						directory.create_dir(name)?;

						open_map_directory(&directory, name)?
					}
					Err(error) => return Err(error),
				};
			}
		}

		first = false;
	}

	Ok(directory)
}

/// Open one component without following a link, including Windows junctions.
fn open_map_directory(parent: &Dir, name: &std::ffi::OsStr) -> io::Result<Dir> {
	#[cfg(not(windows))]
	{
		parent.open_dir_nofollow(name)
	}

	#[cfg(windows)]
	{
		use cap_std::fs::OpenOptionsExt;

		const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
		const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
		const FILE_SHARE_READ_WRITE: u32 = 0x3;

		// Inspect the reparse point itself, never the directory it redirects to.
		// Excluding FILE_SHARE_DELETE also prevents replacing a held directory.
		let file = parent
			.open_with(
				name,
				OpenOptions::new()
					.read(true)
					.share_mode(FILE_SHARE_READ_WRITE)
					.custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT),
			)?
			.into_std();
		let metadata = file.metadata()?;

		if crate::path_security::is_link_like(&metadata) {
			return Err(io::Error::other("output parent is a reparse point"));
		}

		if !metadata.is_dir() {
			return Err(io::Error::from(io::ErrorKind::NotADirectory));
		}

		Ok(Dir::from_std_file(file))
	}
}

/// A named staging file whose creation and cleanup stay relative to its parent.
struct MapTempFile<'a> {
	parent: &'a Dir,
	name: String,
	file: File,
	published: bool,
}

impl<'a> MapTempFile<'a> {
	fn new(parent: &'a Dir) -> io::Result<Self> {
		let mut nonce = [0_u8; 16];
		getrandom::fill(&mut nonce).map_err(|error| io::Error::other(error.to_string()))?;
		let name = format!(".pina-map-{:032x}.tmp", u128::from_le_bytes(nonce));
		// create_new refuses any existing entry, including a planted symlink.
		// Keeping a name from creation avoids O_TMPFILE's procfs publication path.
		let file = parent.open_with(&name, OpenOptions::new().write(true).create_new(true))?;

		Ok(Self {
			parent,
			name,
			file,
			published: false,
		})
	}

	fn publish(mut self, name: &std::ffi::OsStr) -> io::Result<()> {
		self.parent.rename(&self.name, self.parent, name)?;
		self.published = true;

		Ok(())
	}
}

impl Drop for MapTempFile<'_> {
	fn drop(&mut self) {
		// Drop cannot report cleanup errors. Never resolve an ambient path while
		// removing a failed stage, or remove a name we relinquished on publication.
		if !self.published {
			let _ = self.parent.remove_file(&self.name);
		}
	}
}

fn publish_html(parent: &Dir, name: &std::ffi::OsStr, html: &[u8]) -> io::Result<()> {
	let permissions = match parent.symlink_metadata(name) {
		Ok(metadata) if !metadata.is_file() => {
			return Err(io::Error::other(
				"output must be a regular file, not a link or directory",
			));
		}
		Ok(metadata) => Some(metadata.permissions()),
		Err(error) if error.kind() == io::ErrorKind::NotFound => None,
		Err(error) => return Err(error),
	};

	let mut staged = MapTempFile::new(parent)?;

	if let Some(permissions) = permissions {
		staged.file.set_permissions(permissions)?;
	}

	staged.file.write_all(html)?;
	staged.file.sync_all()?;
	// Rename replaces the directory entry, so even a newly planted symlink or
	// hard link cannot cause the old inode's contents to be truncated.
	staged.publish(name)
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
			docs: Vec::new(),
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
		write_html(&map, &path).unwrap_or_else(|error| panic!("replacement failed: {error}"));
		assert_eq!(fs::read_to_string(&path).unwrap(), written);
		assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);

		let blocker = temp.path().join("blocker");
		fs::write(&blocker, "").unwrap_or_else(|error| panic!("write failed: {error}"));
		let error = write_html(&map, &blocker.join("map.html"))
			.expect_err("a file cannot be a parent directory");
		assert!(
			matches!(&error, MapError::Write { path, .. } if *path == blocker.join("map.html")),
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

	#[cfg(unix)]
	#[test]
	fn refuses_linked_outputs_without_modifying_their_targets() {
		use std::os::unix::fs::symlink;

		let temp = tempfile::tempdir().unwrap();
		let victim = temp.path().join("keypair.json");
		fs::write(&victim, b"keypair sentinel").unwrap();
		let link = temp.path().join("map.html");
		symlink(&victim, &link).unwrap();

		let error = write_html(&map(), &link).unwrap_err();
		assert!(error.to_string().contains("symbolic link or reparse point"));
		assert_eq!(fs::read(&victim).unwrap(), b"keypair sentinel");
		assert!(fs::symlink_metadata(&link).unwrap().is_symlink());

		let missing = temp.path().join("missing");
		let dangling = temp.path().join("dangling.html");
		symlink(&missing, &dangling).unwrap();
		assert!(write_html(&map(), &dangling).is_err());
		assert!(!missing.exists());

		let directory_link = temp.path().join("linked");
		symlink(temp.path(), &directory_link).unwrap();
		assert!(write_html(&map(), &directory_link.join("nested/map.html")).is_err());
		assert!(!temp.path().join("nested").exists());
		assert!(open_map_parent(&directory_link.join("map.html")).is_err());
	}

	#[test]
	fn stages_named_files_and_cleans_up_after_failed_publication() {
		let temp = tempfile::tempdir().unwrap();
		let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
		let mut staged = MapTempFile::new(&parent).unwrap();
		staged.file.write_all(b"staged report").unwrap();
		// The file is already named, so publication never needs /proc/self/fd.
		assert_eq!(parent.read(&staged.name).unwrap(), b"staged report");
		parent.create_dir("blocked.html").unwrap();
		let name = staged.name.clone();
		assert!(
			staged
				.publish(std::ffi::OsStr::new("blocked.html"))
				.is_err()
		);

		assert!(!parent.try_exists(name).unwrap());
		assert!(parent.is_dir("blocked.html"));
		assert_eq!(parent.entries().unwrap().count(), 1);
	}

	#[cfg(windows)]
	#[test]
	fn refuses_a_junction_planted_after_path_inspection() {
		use std::process::Command;

		let temp = tempfile::tempdir().unwrap();
		let root = fs::canonicalize(temp.path()).unwrap();
		let parent = Dir::open_ambient_dir(&root, ambient_authority()).unwrap();
		let victim = root.join("victim");
		let link = root.join("linked");
		let output = link.join("nested/map.html");
		fs::create_dir(&victim).unwrap();
		fs::write(victim.join("map.html"), b"sentinel").unwrap();
		assert!(!crate::path_security::has_untrusted_link_component(&output).unwrap());

		let junction = Command::new("cmd.exe")
			.args(["/C", "mklink", "/J"])
			.arg(temp.path().join("linked"))
			.arg(temp.path().join("victim"))
			.output()
			.unwrap();
		assert!(junction.status.success(), "{junction:?}");

		// Bypass write_html's earlier path inspection to exercise traversal's
		// independent protection against a component swapped in that window.
		assert!(open_map_directory(&parent, std::ffi::OsStr::new("linked")).is_err());
		assert!(open_map_parent(&output).is_err());
		assert!(write_html(&map(), &output).is_err());
		assert!(!victim.join("nested").exists());
		assert_eq!(fs::read(victim.join("map.html")).unwrap(), b"sentinel");

		fs::write(root.join("file"), b"not a directory").unwrap();
		assert!(open_map_directory(&parent, std::ffi::OsStr::new("file")).is_err());
	}

	#[test]
	fn atomic_replacement_preserves_hard_linked_files() {
		let temp = tempfile::tempdir().unwrap();
		let victim = temp.path().join("keypair.json");
		let output = temp.path().join("map.html");
		fs::write(&victim, b"keypair sentinel").unwrap();
		fs::hard_link(&victim, &output).unwrap();

		write_html(&map(), &output).unwrap();

		assert_eq!(fs::read(&victim).unwrap(), b"keypair sentinel");
		assert!(
			fs::read_to_string(output)
				.unwrap()
				.starts_with("<!doctype html>")
		);
	}

	#[cfg(unix)]
	#[test]
	fn replacing_a_private_report_preserves_its_permissions() {
		use std::os::unix::fs::PermissionsExt;

		let temp = tempfile::tempdir().unwrap();
		let output = temp.path().join("map.html");
		fs::write(&output, b"previous report").unwrap();
		fs::set_permissions(&output, fs::Permissions::from_mode(0o600)).unwrap();

		write_html(&map(), &output).unwrap();

		assert_eq!(
			fs::metadata(&output).unwrap().permissions().mode() & 0o777,
			0o600
		);
	}

	#[cfg(unix)]
	#[test]
	fn pins_the_parent_directory_before_publication() {
		use std::os::unix::fs::symlink;

		let temp = tempfile::tempdir().unwrap();
		let output = temp.path().join("output");
		fs::create_dir(&output).unwrap();
		let parent = open_map_parent(&output.join("map.html")).unwrap();
		let original = temp.path().join("original");
		fs::rename(&output, &original).unwrap();
		let victim = temp.path().join("victim");
		fs::create_dir(&victim).unwrap();
		fs::write(victim.join("map.html"), b"sentinel").unwrap();
		symlink(&victim, &output).unwrap();

		publish_html(&parent, std::ffi::OsStr::new("map.html"), b"safe report").unwrap();

		assert_eq!(fs::read(victim.join("map.html")).unwrap(), b"sentinel");
		assert_eq!(fs::read(original.join("map.html")).unwrap(), b"safe report");

		let late = original.join("late.html");
		symlink(victim.join("map.html"), &late).unwrap();
		assert!(publish_html(&parent, std::ffi::OsStr::new("late.html"), b"report").is_err());
		assert_eq!(fs::read(victim.join("map.html")).unwrap(), b"sentinel");
	}

	#[test]
	fn rejects_invalid_names_and_directory_outputs() {
		let temp = tempfile::tempdir().unwrap();
		let directory = temp.path().join("map.html");
		fs::create_dir(&directory).unwrap();
		let parent = open_map_parent(&directory).unwrap();

		assert!(write_html(&map(), Path::new(".")).is_err());
		assert!(open_map_parent(Path::new("/")).is_err());
		assert!(publish_html(&parent, std::ffi::OsStr::new("invalid\0name"), b"report").is_err());
		assert!(publish_html(&parent, std::ffi::OsStr::new("map.html"), b"report").is_err());
		assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);

		// Parent components remain relative to the directory handles, including
		// `..`; a caller's working directory never needs to be changed.
		write_html(&map(), &directory.join("../report.html")).unwrap();
		assert!(temp.path().join("report.html").is_file());
	}

	#[cfg(unix)]
	#[test]
	fn opens_trusted_system_aliases_without_following_project_links() {
		for entry in fs::read_dir("/").unwrap() {
			let path = entry.unwrap().path();
			let metadata = fs::symlink_metadata(&path).unwrap();

			if crate::path_security::is_link_like(&metadata)
				&& !crate::path_security::is_untrusted_link(&metadata, true)
				&& path.is_dir()
			{
				assert!(open_map_parent(&path.join("map.html")).is_ok());
			}
		}
	}
}
