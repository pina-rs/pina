//! Event migration facts extracted from a checked-in migration manifest.
//!
//! Generated clients enforce the event version envelope from the IDL alone.
//! Projecting a stale log into the current shape additionally needs the
//! historical schemas, which the Codama IDL intentionally omits: event history
//! lives in `migrations/manifest.json`. Client generation runs next to that
//! manifest, so these facts turn each checked-in adjacent transition into the
//! byte moves a generated projection must apply. Manual transitions carry no
//! derivable byte mapping, so the generated clients fail closed for the
//! versions that would need one.

use std::collections::BTreeMap;
use std::path::Path;

use pina_abi::ContractHistory;
use pina_abi::ContractKind;
use pina_abi::DataSchema;
use pina_abi::MANIFEST_PATH;
use pina_abi::TransitionMode;

/// One adjacent event projection, payload-relative to the version envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EventProjectionStep {
	/// Source schema version.
	pub(crate) from: u32,
	/// Destination schema version, always `from + 1`.
	pub(crate) to: u32,
	/// Whether the checked-in transition is an automatic byte mapping.
	pub(crate) automatic: bool,
	/// Exact source payload size, excluding discriminator and version.
	pub(crate) source_payload_size: usize,
	/// Exact destination payload size, excluding discriminator and version.
	pub(crate) destination_payload_size: usize,
	/// Field byte moves from the source payload into the destination payload.
	pub(crate) moves: Vec<EventFieldMove>,
}

/// One field's bytes moving between adjacent payload layouts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EventFieldMove {
	pub(crate) source_offset: usize,
	pub(crate) destination_offset: usize,
	pub(crate) size: usize,
}

/// Projection facts for one migratable event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EventClientHistory {
	/// Rust struct ident recorded in the manifest, for diagnostics.
	pub(crate) rust_name: String,
	/// Discriminator bytes as stored at offset zero.
	pub(crate) discriminator: Vec<u8>,
	/// Current schema version this project's manifest publishes.
	pub(crate) current_version: u32,
	/// Every adjacent transition in source order.
	pub(crate) steps: Vec<EventProjectionStep>,
}

impl EventClientHistory {
	/// Projection key for an IDL event: discriminator width and value.
	///
	/// The IDL renders the discriminator as a numeric constant at offset zero,
	/// so the width and value identify the contract without depending on naming
	/// conventions.
	pub(crate) fn key(&self) -> EventProjectionKey {
		EventProjectionKey {
			width: self.discriminator.len(),
			value: le_value(&self.discriminator),
		}
	}
}

/// Identity of one migratable event across IDL and manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct EventProjectionKey {
	/// Discriminator width in bytes.
	pub(crate) width: usize,
	/// Discriminator value interpreted little-endian.
	pub(crate) value: u64,
}

/// Projection facts for every migratable event in one program.
#[derive(Clone, Debug, Default)]
pub(crate) struct EventClientHistoryIndex {
	histories: BTreeMap<EventProjectionKey, EventClientHistory>,
}

impl EventClientHistoryIndex {
	/// Look up one event by its IDL discriminator facts.
	pub(crate) fn get(&self, width: usize, value: u64) -> Option<&EventClientHistory> {
		self.histories.get(&EventProjectionKey { width, value })
	}

	/// The same facts in the Rust renderer's checkout-independent shape.
	pub(crate) fn renderer_histories(&self) -> Vec<pina_codama_renderer::EventMigrationHistory> {
		self.histories
			.values()
			.map(|history| {
				pina_codama_renderer::EventMigrationHistory {
					rust_name: history.rust_name.clone(),
					discriminator: history.discriminator.clone(),
					current_version: history.current_version,
					steps: history
						.steps
						.iter()
						.map(|step| {
							pina_codama_renderer::EventProjectionStep {
								from: step.from,
								to: step.to,
								automatic: step.automatic,
								source_payload_size: step.source_payload_size,
								destination_payload_size: step.destination_payload_size,
								moves: step
									.moves
									.iter()
									.map(|movement| {
										pina_codama_renderer::EventFieldMove {
											source_offset: movement.source_offset,
											destination_offset: movement.destination_offset,
											size: movement.size,
										}
									})
									.collect(),
							}
						})
						.collect(),
				}
			})
			.collect()
	}

	#[cfg(test)]
	pub(crate) fn insert_for_test(&mut self, history: EventClientHistory) {
		self.histories.insert(history.key(), history);
	}
}

/// Read every migratable event history from `<program>/migrations/manifest.json`.
///
/// Programs without a manifest return an empty index: their events either are
/// not migration-aware or have no checked-in history, and generated clients
/// still enforce the current envelope from the IDL.
///
/// # Errors
///
/// Returns a message when the manifest cannot be read, decoded, or validated.
pub(crate) fn read_histories(program_dir: &Path) -> Result<EventClientHistoryIndex, String> {
	let path = program_dir.join(MANIFEST_PATH);
	if !path.exists() {
		return Ok(EventClientHistoryIndex::default());
	}
	let bytes = std::fs::read(&path).map_err(|source| {
		format!(
			"could not read event migration manifest {}: {source}",
			path.display()
		)
	})?;
	let manifest = pina_abi::decode_manifest(&bytes)?;
	manifest.validate()?;

	let mut histories = BTreeMap::new();
	for history in manifest
		.contracts
		.values()
		.filter(|history| history.identity.kind == ContractKind::Event)
	{
		let facts = EventClientHistory::try_from(history)?;
		histories.insert(facts.key(), facts);
	}

	Ok(EventClientHistoryIndex { histories })
}

impl EventClientHistory {
	fn try_from(history: &ContractHistory) -> Result<Self, String> {
		let current = history
			.current()
			.ok_or_else(|| format!("event contract `{}` has no versions", history.rust_name))?;
		let mut steps = Vec::new();
		for index in 1..history.versions.len() {
			let source = &history.versions[index - 1];
			let destination = &history.versions[index];
			let transition = destination.transition.as_ref().ok_or_else(|| {
				format!(
					"event contract `{}` version {} is missing its adjacent transition",
					history.rust_name, destination.version,
				)
			})?;
			let automatic = transition.mode == TransitionMode::Automatic;
			let moves = if automatic {
				field_moves(&source.schema, &destination.schema).ok_or_else(|| {
					format!(
						"event contract `{}` automatic transition v{} to v{} has no derivable \
						 fixed-layout byte mapping",
						history.rust_name, transition.from, transition.to,
					)
				})?
			} else {
				Vec::new()
			};
			steps.push(EventProjectionStep {
				from: transition.from,
				to: transition.to,
				automatic,
				source_payload_size: source.schema.fixed_payload_size().ok_or_else(|| {
					format!(
						"event contract `{}` version {} is not a fixed layout",
						history.rust_name, source.version,
					)
				})?,
				destination_payload_size: destination.schema.fixed_payload_size().ok_or_else(
					|| {
						format!(
							"event contract `{}` version {} is not a fixed layout",
							history.rust_name, destination.version,
						)
					},
				)?,
				moves,
			});
		}

		let discriminator = decode_hex(&history.identity.discriminator_hex)?;
		if discriminator.len() != usize::from(history.identity.discriminator_bytes) {
			return Err(format!(
				"event contract `{}` has a discriminator that does not match its width",
				history.rust_name,
			));
		}

		Ok(Self {
			rust_name: history.rust_name.clone(),
			discriminator,
			current_version: current.version,
			steps,
		})
	}
}

/// Reproduce the byte moves `pina migrations make` writes for an automatic
/// transition: destination fields keep their name and Rust type, source bytes
/// move to the destination offset, and fields without a match stay zero.
fn field_moves(source: &DataSchema, destination: &DataSchema) -> Option<Vec<EventFieldMove>> {
	let source_offsets = source.fixed_field_offsets()?;
	let destination_offsets = destination.fixed_field_offsets()?;
	let source_types = source
		.fields
		.iter()
		.map(|field| (field.name.as_str(), field.rust_type.as_str()))
		.collect::<BTreeMap<_, _>>();

	let mut moves = Vec::new();
	for field in &destination.fields {
		if source_types.get(field.name.as_str()) != Some(&field.rust_type.as_str()) {
			continue;
		}
		let (source_offset, source_size) = source_offsets.get(&field.name)?;
		let (destination_offset, destination_size) = destination_offsets.get(&field.name)?;
		// A field's byte width is a property of its Rust type; automatic
		// transitions only match identical types, so the sizes agree.
		if source_size != destination_size {
			return None;
		}
		moves.push(EventFieldMove {
			source_offset: *source_offset,
			destination_offset: *destination_offset,
			size: *source_size,
		});
	}

	Some(moves)
}

fn le_value(bytes: &[u8]) -> u64 {
	let mut padded = [0_u8; 8];
	let width = bytes.len().min(padded.len());
	padded[..width].copy_from_slice(&bytes[..width]);
	u64::from_le_bytes(padded)
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
	if !value.len().is_multiple_of(2) {
		return Err("discriminator hex must contain complete bytes".to_owned());
	}
	let mut bytes = Vec::with_capacity(value.len() / 2);
	for pair in value.as_bytes().as_chunks::<2>().0 {
		let text = std::str::from_utf8(pair).map_err(|_| "discriminator hex is not ASCII")?;
		let byte = u8::from_str_radix(text, 16)
			.map_err(|_| format!("discriminator hex `{text}` is not a byte"))?;
		bytes.push(byte);
	}
	Ok(bytes)
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	use pina_abi::ContractHistory;
	use pina_abi::ContractIdentity;
	use pina_abi::ContractKind;
	use pina_abi::DataCodec;
	use pina_abi::DataSchema;
	use pina_abi::FieldSchema;
	use pina_abi::FixedFieldLayout;
	use pina_abi::LayoutKind;
	use pina_abi::PhysicalLayout;
	use pina_abi::SchemaVersion;
	use pina_abi::Transition;
	use pina_abi::TransitionMode;

	use super::*;

	fn schema(fields: &[(&str, &str)]) -> DataSchema {
		DataSchema::try_new(
			LayoutKind::Fixed,
			fields
				.iter()
				.map(|(name, rust_type)| {
					FieldSchema {
						name: (*name).to_owned(),
						rust_type: (*rust_type).to_owned(),
					}
				})
				.collect(),
		)
		.unwrap_or_else(|error| panic!("schema: {error}"))
	}

	fn example_program_dir() -> PathBuf {
		PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/migrations_program")
	}

	/// A raw fixed schema whose physical descriptor is provided verbatim, so
	/// deliberately inconsistent layouts can be exercised.
	fn raw_schema(fields: &[(&str, &str)], sizes: &[u64]) -> DataSchema {
		DataSchema {
			layout: LayoutKind::Fixed,
			fields: fields
				.iter()
				.map(|(name, rust_type)| {
					FieldSchema {
						name: (*name).to_owned(),
						rust_type: (*rust_type).to_owned(),
					}
				})
				.collect(),
			codec: DataCodec::PinaPodV2,
			physical: PhysicalLayout::Fixed {
				size: sizes.iter().sum(),
				fields: sizes
					.iter()
					.enumerate()
					.map(|(index, size)| {
						FixedFieldLayout {
							name: fields[index].0.to_owned(),
							offset: u64::try_from(index).unwrap_or(u64::MAX),
							size: *size,
						}
					})
					.collect(),
			},
		}
	}

	fn schema_version(
		version: u32,
		schema: DataSchema,
		transition: Option<TransitionMode>,
	) -> SchemaVersion {
		let transition = transition.map(|mode| {
			Transition {
				from: version - 1,
				to: version,
				mode,
				renames: Vec::new(),
				source_schema_sha256: String::new(),
				destination_schema_sha256: String::new(),
				source_process_sha256: None,
				destination_process_sha256: None,
				process: None,
				implementation_sha256: None,
			}
		});
		SchemaVersion {
			version,
			schema_sha256: schema.sha256(),
			schema,
			process: None,
			process_sha256: None,
			transition,
		}
	}

	fn event_history(versions: Vec<SchemaVersion>) -> ContractHistory {
		ContractHistory {
			identity: ContractIdentity {
				kind: ContractKind::Event,
				discriminator_bytes: 1,
				discriminator_hex: "04".to_owned(),
			},
			rust_name: "ValueChangedEvent".to_owned(),
			versions,
		}
	}

	#[test]
	fn renderer_histories_convert_every_projection_fact() {
		let index = read_histories(&example_program_dir())
			.unwrap_or_else(|error| panic!("manifest: {error}"));
		let converted = index.renderer_histories();

		assert_eq!(converted.len(), 1);
		let history = &converted[0];
		assert_eq!(history.rust_name, "ValueChangedEvent");
		assert_eq!(history.discriminator, [4]);
		assert_eq!(history.current_version, 1);
		assert_eq!(history.steps.len(), 1);
		let step = &history.steps[0];
		assert_eq!((step.from, step.to), (0, 1));
		assert!(step.automatic);
		assert_eq!(step.source_payload_size, 8);
		assert_eq!(step.destination_payload_size, 10);
		assert_eq!(step.moves.len(), 1);
		let movement = &step.moves[0];
		assert_eq!(
			(
				movement.source_offset,
				movement.destination_offset,
				movement.size
			),
			(0, 0, 8),
		);
	}

	#[test]
	fn read_histories_reports_unreadable_manifests() {
		let temporary = tempfile::tempdir().expect("temp dir");
		let program = temporary.path().join("program");
		let manifest = program.join("migrations/manifest.json");
		std::fs::create_dir_all(&manifest).expect("manifest path as directory");

		let error = read_histories(&program).expect_err("a directory is not a manifest");
		assert!(
			error.contains("could not read event migration manifest"),
			"{error}"
		);
		assert!(error.contains("manifest.json"), "{error}");
	}

	#[test]
	fn history_parsing_rejects_missing_versions_and_transitions() {
		let empty = event_history(Vec::new());
		let error = EventClientHistory::try_from(&empty).expect_err("empty histories fail");
		assert!(error.contains("has no versions"), "{error}");

		let no_transition = event_history(vec![
			schema_version(0, schema(&[("value", "u64")]), None),
			schema_version(1, schema(&[("value", "u64")]), None),
		]);
		let error =
			EventClientHistory::try_from(&no_transition).expect_err("v1 needs a transition");
		assert!(error.contains("missing its adjacent transition"), "{error}");
	}

	#[test]
	fn history_parsing_rejects_undecodable_projection_plans() {
		let compact = DataSchema::try_new(
			LayoutKind::Compact,
			vec![FieldSchema {
				name: "code".to_owned(),
				rust_type: "String<5>".to_owned(),
			}],
		)
		.unwrap_or_else(|error| panic!("compact schema: {error}"));

		// Automatic transitions need a derivable fixed-layout byte mapping.
		let automatic_compact = event_history(vec![
			schema_version(0, compact.clone(), None),
			schema_version(
				1,
				schema(&[("value", "u64"), ("memo", "u16")]),
				Some(TransitionMode::Automatic),
			),
		]);
		let error = EventClientHistory::try_from(&automatic_compact)
			.expect_err("compact automatic transitions have no mapping");
		assert!(
			error.contains("has no derivable fixed-layout byte mapping"),
			"{error}"
		);

		// Manual transitions still need exact fixed payload sizes on both ends.
		let manual_compact_source = event_history(vec![
			schema_version(0, compact, None),
			schema_version(1, schema(&[("value", "u64")]), Some(TransitionMode::Manual)),
		]);
		let error = EventClientHistory::try_from(&manual_compact_source)
			.expect_err("compact sources have no exact payload size");
		assert!(error.contains("version 0 is not a fixed layout"), "{error}");

		let manual_compact_destination = event_history(vec![
			schema_version(0, schema(&[("value", "u64")]), None),
			schema_version(
				1,
				DataSchema::try_new(
					LayoutKind::Compact,
					vec![FieldSchema {
						name: "code".to_owned(),
						rust_type: "String<5>".to_owned(),
					}],
				)
				.unwrap_or_else(|error| panic!("compact schema: {error}")),
				Some(TransitionMode::Manual),
			),
		]);
		let error = EventClientHistory::try_from(&manual_compact_destination)
			.expect_err("compact destinations have no exact payload size");
		assert!(error.contains("version 1 is not a fixed layout"), "{error}");
	}

	#[test]
	fn history_parsing_rejects_discriminator_width_mismatches() {
		let mut history = event_history(vec![schema_version(0, schema(&[("value", "u64")]), None)]);
		history.identity.discriminator_bytes = 2;

		let error =
			EventClientHistory::try_from(&history).expect_err("a short hex discriminator fails");
		assert!(error.contains("does not match its width"), "{error}");
	}

	#[test]
	fn field_moves_rejects_inconsistent_physical_layouts() {
		// The declared Rust type matches, so only the physical sizes disagree.
		let source = raw_schema(&[("value", "u64")], &[4]);
		let destination = raw_schema(&[("value", "u64")], &[8]);
		assert!(field_moves(&source, &destination).is_none());

		// A physical descriptor that omits a declared field has no offset.
		let missing = raw_schema(&[("first", "u32"), ("second", "u16")], &[4]);
		let declared = schema(&[("first", "u32"), ("second", "u16")]);
		assert!(field_moves(&missing, &declared).is_none());
		assert!(field_moves(&declared, &missing).is_none());
	}

	#[test]
	fn automatic_transitions_turn_into_payload_field_moves() {
		let source = schema(&[("value", "u64")]);
		let destination = schema(&[("value", "u64"), ("memo", "u16")]);

		let moves = field_moves(&source, &destination)
			.unwrap_or_else(|| panic!("v0 to v1 must have a byte mapping"));
		assert_eq!(
			moves,
			[EventFieldMove {
				source_offset: 0,
				destination_offset: 0,
				size: 8,
			}],
			"the appended field stays zero and the shared field keeps its offset",
		);
	}

	#[test]
	fn removed_and_reordered_fields_keep_the_destination_mapping() {
		let source = schema(&[("first", "u32"), ("second", "u16"), ("gone", "u8")]);
		let destination = schema(&[("gone", "u8"), ("second", "u16"), ("first", "u32")]);

		let moves = field_moves(&source, &destination).unwrap_or_else(|| panic!("mapping"));
		let mut keyed = moves
			.iter()
			.map(|step| (step.destination_offset, step.source_offset, step.size))
			.collect::<Vec<_>>();
		keyed.sort_unstable();
		assert_eq!(keyed, [(0, 6, 1), (1, 4, 2), (3, 0, 4)]);
	}

	#[test]
	fn variable_layouts_have_no_derivable_mapping() {
		let source = DataSchema::try_new(
			LayoutKind::Compact,
			vec![FieldSchema {
				name: "code".to_owned(),
				rust_type: "String<5>".to_owned(),
			}],
		)
		.unwrap_or_else(|error| panic!("schema: {error}"));
		let destination = schema(&[("code", "String<5>")]);

		assert!(field_moves(&source, &destination).is_none());
	}

	#[test]
	fn missing_manifests_produce_an_empty_index() {
		let index = read_histories(&PathBuf::from("/definitely/missing/program"))
			.unwrap_or_else(|error| panic!("empty index: {error}"));
		assert!(index.get(1, 4).is_none());
	}

	#[test]
	fn discriminator_bytes_key_the_history() {
		let history = EventClientHistory {
			rust_name: "ValueChangedEvent".to_owned(),
			discriminator: vec![4],
			current_version: 1,
			steps: Vec::new(),
		};
		assert_eq!(history.key(), EventProjectionKey { width: 1, value: 4 });
		assert_eq!(le_value(&[4, 0]), 4);
	}

	#[test]
	fn reads_history_from_the_checked_in_example_manifest() {
		let program_dir =
			PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/migrations_program");
		let index = read_histories(&program_dir).unwrap_or_else(|error| panic!("{error}"));

		let history = index
			.get(1, 4)
			.unwrap_or_else(|| panic!("valueChangedEvent history"));
		assert_eq!(history.rust_name, "ValueChangedEvent");
		assert_eq!(history.current_version, 1);
		assert_eq!(history.discriminator, [4]);
		assert_eq!(history.steps.len(), 1);
		let step = &history.steps[0];
		assert!(step.automatic);
		assert_eq!(step.source_payload_size, 8);
		assert_eq!(step.destination_payload_size, 10);
		assert_eq!(
			step.moves,
			[EventFieldMove {
				source_offset: 0,
				destination_offset: 0,
				size: 8,
			}],
		);
	}

	#[test]
	fn manual_transitions_are_recorded_without_byte_moves() {
		let index = read_manifest_history(manual_event_history());
		let history = index.get(1, 4).unwrap_or_else(|| panic!("manual history"));
		assert_eq!(history.steps.len(), 1);
		assert!(!history.steps[0].automatic);
		assert!(history.steps[0].moves.is_empty());
	}

	#[test]
	fn malformed_manifests_fail_with_a_message() {
		let temporary = tempfile::tempdir().expect("temp dir");
		let directory = temporary.path().join("example");
		let manifest_dir = directory.join("migrations");
		std::fs::create_dir_all(&manifest_dir).expect("manifest dir");
		std::fs::write(manifest_dir.join("manifest.json"), b"{ not json").expect("write");
		let error = read_histories(&directory).expect_err("invalid JSON must fail");
		assert!(error.contains("invalid migration manifest JSON"), "{error}");

		let mut manifest = manual_event_history();
		let key = manifest.contracts.keys().next().cloned().expect("key");
		let history = manifest.contracts.get_mut(&key).expect("history");
		history.versions[1].transition = None;
		std::fs::write(
			manifest_dir.join("manifest.json"),
			serde_json::to_vec(&manifest).expect("serialize"),
		)
		.expect("write");
		let error = read_histories(&directory).expect_err("a missing transition must fail");
		assert!(error.contains("invalid adjacent transition"), "{error}");

		// `pina_abi` validates content hashes before the facts are extracted.
		let mut manifest = manual_event_history();
		let key = manifest.contracts.keys().next().cloned().expect("key");
		let history = manifest.contracts.get_mut(&key).expect("history");
		history.versions[1].schema_sha256 = "00".repeat(32);
		std::fs::write(
			manifest_dir.join("manifest.json"),
			serde_json::to_vec(&manifest).expect("serialize"),
		)
		.expect("write");
		let error = read_histories(&directory).expect_err("hash drift must fail");
		assert!(error.contains("schema hash does not match"), "{error}");
	}

	/// One event history whose only transition changes a field type, which
	/// `pina migrations make` records as manual.
	fn manual_event_history() -> pina_abi::MigrationManifest {
		use pina_abi::ContractHistory;
		use pina_abi::ContractIdentity;
		use pina_abi::MigrationVersionType;
		use pina_abi::SchemaVersion;
		use pina_abi::Transition;
		use pina_abi::TransitionMode;

		let source = schema(&[("value", "u64")]);
		let destination = schema(&[("value", "u32")]);
		let transition = Transition {
			from: 0,
			to: 1,
			mode: TransitionMode::Manual,
			renames: Vec::new(),
			source_schema_sha256: source.sha256(),
			destination_schema_sha256: destination.sha256(),
			source_process_sha256: None,
			destination_process_sha256: None,
			process: None,
			implementation_sha256: None,
		};
		let identity = ContractIdentity::try_new(ContractKind::Event, 1, 4)
			.unwrap_or_else(|error| panic!("identity: {error}"));
		let key = identity.key();
		let history = ContractHistory {
			identity,
			rust_name: "ValueChangedEvent".to_owned(),
			versions: vec![
				SchemaVersion {
					version: 0,
					schema_sha256: source.sha256(),
					schema: source.clone(),
					process: None,
					process_sha256: None,
					transition: None,
				},
				SchemaVersion {
					version: 1,
					schema_sha256: destination.sha256(),
					schema: destination,
					process: None,
					process_sha256: None,
					transition: Some(transition),
				},
			],
		};
		let mut manifest = pina_abi::MigrationManifest::new(
			"GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS".to_owned(),
			MigrationVersionType::U8,
		);
		manifest.contracts.insert(key, history);
		manifest
	}

	fn read_manifest_history(manifest: pina_abi::MigrationManifest) -> EventClientHistoryIndex {
		let temporary = tempfile::tempdir().expect("temp dir");
		let directory = temporary.path().join("example");
		let manifest_dir = directory.join("migrations");
		std::fs::create_dir_all(&manifest_dir).expect("manifest dir");
		std::fs::write(
			manifest_dir.join("manifest.json"),
			serde_json::to_vec(&manifest).expect("serialize"),
		)
		.expect("write");
		let index = read_histories(&directory).unwrap_or_else(|error| panic!("{error}"));
		// The caller reads the borrowed index before the temporary is dropped.
		let owned = index.clone();
		owned
	}

	#[test]
	fn hex_discriminators_decode_or_fail() {
		assert_eq!(
			decode_hex("04ff").unwrap_or_else(|error| panic!("hex: {error}")),
			[4, 255],
		);
		assert!(decode_hex("0").is_err());
		assert!(decode_hex("zz").is_err());
	}
}
