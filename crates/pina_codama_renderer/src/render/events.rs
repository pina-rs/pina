//! Rust client rendering for event log records.
//!
//! Events are immutable transaction-log records, so generated event modules
//! are decode-only. Migration-aware events additionally expose the version
//! envelope and a direction-aware decode. Each historical version of an event
//! is its own IDL event node, so older records decode with that node's module.

use codama_nodes::DefaultValueStrategy;
use codama_nodes::EventNode;
use codama_nodes::HasKind as _;
use codama_nodes::Number;
use codama_nodes::NumberFormat;
use codama_nodes::StructTypeNode;
use codama_nodes::TypeNode;
use codama_nodes::ValueNode;
use heck::ToShoutySnakeCase as _;

use super::discriminator::DiscriminatorInfo;
use super::discriminator::OmittedConstantInfo;
use super::discriminator::render_constant_discriminator;
use super::discriminator::render_omitted_value_constant;
use super::helpers::pascal;
use super::helpers::render_docs;
use super::helpers::snake;
use super::helpers::version_type_max;
use super::types::render_type_for_pod;
use crate::error::RenderError;
use crate::error::Result;

/// The `[discriminator][migrationVersion]` envelope of one event.
struct EventEnvelope {
	version: u64,
	version_ty: String,
	version_bytes: usize,
}

pub(crate) fn render_events_mod(events: &[EventNode]) -> String {
	let mut lines = Vec::new();

	for event in events {
		lines.push(format!("pub(crate) mod r#{};", snake(event.name.as_ref())));
	}

	lines.push(String::new());

	for event in events {
		lines.push(format!(
			"pub use self::r#{}::*;",
			snake(event.name.as_ref())
		));
	}

	lines.join("\n")
}

/// Render one event module: the wire struct plus its decode API.
pub(crate) fn render_event_page(event: &EventNode) -> Result<String> {
	let event_name = pascal(event.name.as_ref());
	let zc_name = format!("{event_name}Zc");
	let context = format!("event `{event_name}`");
	let discriminator =
		render_constant_discriminator(event.name.as_ref(), &event.discriminators, &context)?;
	let TypeNode::Struct(data_type) = event.data.as_ref() else {
		return Err(RenderError::UnsupportedType {
			context,
			kind: event.data.kind(),
			reason: "events must be fixed-size structs".to_string(),
		});
	};

	let omitted_constants = data_type
		.fields
		.iter()
		.filter(|field| {
			field.name.as_ref() != "discriminator"
				&& matches!(
					field.default_value_strategy,
					Some(DefaultValueStrategy::Omitted)
				)
		})
		.map(|field| {
			render_omitted_value_constant(
				event.name.as_ref(),
				field.name.as_ref(),
				&field.r#type,
				field.default_value.as_ref().as_ref(),
				&context,
			)
		})
		.collect::<Result<Vec<_>>>()?;

	let envelope = event_migration_envelope(data_type, discriminator.as_ref());

	let mut field_lines = Vec::new();
	if let Some(discriminator) = &discriminator {
		field_lines.push(format!("\tpub discriminator: {},", discriminator.ty));
	}
	for field in &data_type.fields {
		if discriminator.is_some() && field.name.as_ref() == "discriminator" {
			continue;
		}
		let field_name = snake(field.name.as_ref());
		let field_context = format!("{event_name}.{field_name}");
		let field_type = render_type_for_pod(&field.r#type, &field_context)?;
		for doc_line in render_docs(&field.docs, 1) {
			field_lines.push(doc_line);
		}
		field_lines.push(format!("\tpub {field_name}: {field_type},"));
	}

	let mut lines = render_docs(&event.docs, 0);
	lines.push("#[allow(clippy::len_without_is_empty)]".to_string());
	lines.push("#[derive(pina::PinaPod)]".to_string());
	lines.push("#[pinapod(crate = pina::pinapod, no_inherent)]".to_string());
	lines.push(format!("pub struct {event_name} {{"));
	lines.extend(field_lines);
	lines.push("}".to_string());
	lines.push(String::new());

	if let Some(discriminator) = &discriminator {
		lines.push(format!(
			"pub const {}: {} = {};",
			discriminator.name, discriminator.ty, discriminator.value
		));
		lines.push(String::new());
	}
	for constant in &omitted_constants {
		lines.push(format!(
			"pub const {}: {} = {};",
			constant.name, constant.ty, constant.value
		));
	}
	if !omitted_constants.is_empty() {
		lines.push(String::new());
	}

	lines.extend(render_decoders(
		&event_name,
		&zc_name,
		discriminator.as_ref(),
		&omitted_constants,
		envelope.as_ref(),
	));

	Ok(lines.join("\n"))
}

fn render_decoders(
	event_name: &str,
	zc_name: &str,
	discriminator: Option<&DiscriminatorInfo>,
	omitted_constants: &[OmittedConstantInfo],
	envelope: Option<&EventEnvelope>,
) -> Vec<String> {
	let mut lines = Vec::new();
	lines.push(format!("impl {event_name} {{"));
	lines.push("\t/// Exact size of this event's representation.".to_string());
	lines.push(format!(
		"\tpub const LEN: usize = core::mem::size_of::<{zc_name}>();"
	));
	lines.push(String::new());
	lines.push("\t/// Read one record of this event from transaction-log bytes.".to_string());
	lines.push("\t///".to_string());
	lines.push(
		"\t/// Logs carry the record base64-encoded after `Program data: `; pass the decoded \
		 bytes here."
			.to_string(),
	);
	if envelope.is_some() {
		lines.push("\t///".to_string());
		lines.push(
			"\t/// Records of another version are rejected; `try_from_bytes` tells them apart, \
			 and the event generated for that version decodes them."
				.to_string(),
		);
	}
	lines.push(format!(
		"\tpub fn from_bytes(data: &[u8]) -> Result<&{zc_name}, \
		 solana_program_error::ProgramError> {{"
	));
	lines.push(
		"\t\tlet event = <Self as pina::PinaPodFixed>::read_exact(data)\n\t\t\t.map_err(|_| \
		 solana_program_error::ProgramError::InvalidArgument)?;"
			.to_string(),
	);
	if let Some(discriminator) = discriminator {
		lines.push(format!(
			"\t\tif event.discriminator != {} {{",
			discriminator.name
		));
		lines.push(
			"\t\t\treturn Err(solana_program_error::ProgramError::InvalidArgument);".to_string(),
		);
		lines.push("\t\t}".to_string());
	}
	for constant in omitted_constants {
		lines.push(format!(
			"\t\tif event.{} != {} {{",
			constant.field, constant.name
		));
		lines.push(
			"\t\t\treturn Err(solana_program_error::ProgramError::InvalidArgument);".to_string(),
		);
		lines.push("\t\t}".to_string());
	}
	lines.push("\t\tOk(event)".to_string());
	lines.push("\t}".to_string());
	lines.push("}".to_string());

	if let Some(envelope) = envelope {
		lines.push(String::new());
		lines.extend(render_version_error(event_name, envelope));
		lines.push(String::new());
		lines.extend(render_try_from_bytes(
			event_name,
			zc_name,
			discriminator,
			envelope,
		));
	}

	lines
}

/// The direction-aware version error returned by `try_from_bytes`.
///
/// Versions are relative to this event node: a historical `<Event>V<n>` node
/// calls the current layout "later", and the current node calls a historical
/// one "earlier". Either way, another generated event decodes it.
fn render_version_error(event_name: &str, envelope: &EventEnvelope) -> Vec<String> {
	let error_enum = format!("{event_name}VersionError");
	let version = envelope.version;
	let version_ty = &envelope.version_ty;
	let stale_hint = "an earlier version emitted it; decode it with the event generated for that \
	                  version";
	let future_hint = "a later version emitted it; decode it with the event generated for that \
	                   version, or regenerate this client";

	let mut lines = Vec::new();
	lines.push(format!(
		"/// Why `{event_name}::try_from_bytes` rejected event bytes."
	));
	lines.push("#[derive(Clone, Copy, Debug, PartialEq, Eq)]".to_string());
	lines.push(format!("pub enum {error_enum} {{"));
	lines.push("\t/// The bytes do not decode as this event's layout at all.".to_string());
	lines.push("\tInvalidData,".to_string());
	lines
		.push("\t/// The envelope names this event but an earlier version emitted it.".to_string());
	lines.push(format!("\tStale {{ stored: {version_ty} }},"));
	lines.push("\t/// The envelope names this event but a later version emitted it.".to_string());
	lines.push(format!("\tFuture {{ stored: {version_ty} }},"));
	lines.push("}".to_string());
	lines.push(String::new());
	lines.push(format!("impl core::fmt::Display for {error_enum} {{"));
	lines.push(
		"\tfn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {".to_string(),
	);
	lines.push("\t\tmatch self {".to_string());
	lines.push(format!(
		"\t\t\tSelf::InvalidData => write!(f, \"invalid {event_name} event data\"),"
	));
	lines.push("\t\t\tSelf::Stale { stored } => write!(".to_string());
	lines.push("\t\t\t\tf,".to_string());
	lines.push(format!(
		"\t\t\t\t\"event migration version mismatch: expected {version}, received {{stored}} \
		 ({stale_hint})\""
	));
	lines.push("\t\t\t),".to_string());
	lines.push("\t\t\tSelf::Future { stored } => write!(".to_string());
	lines.push("\t\t\t\tf,".to_string());
	lines.push(format!(
		"\t\t\t\t\"event migration version mismatch: expected {version}, received {{stored}} \
		 ({future_hint})\""
	));
	lines.push("\t\t\t),".to_string());
	lines.push("\t\t}".to_string());
	lines.push("\t}".to_string());
	lines.push("}".to_string());
	lines.push(String::new());
	lines.push(format!("impl std::error::Error for {error_enum} {{}}"));
	lines
}

fn render_try_from_bytes(
	event_name: &str,
	zc_name: &str,
	discriminator: Option<&DiscriminatorInfo>,
	envelope: &EventEnvelope,
) -> Vec<String> {
	let error_enum = format!("{event_name}VersionError");
	let version_constant = format!("{}_MIGRATION_VERSION", event_name.to_shouty_snake_case());
	// Stored versions are unsigned, so a stored version can never compare
	// below 0 or above its type maximum. Emitting those impossible arms
	// trips the deny-by-default `clippy::absurd_extreme_comparisons` in
	// the generated crate, so only reachable arms are emitted.
	let stale_possible = envelope.version != 0;
	let future_possible = envelope.version != version_type_max(envelope.version_bytes);

	let mut lines = Vec::new();
	lines.push(format!("impl {event_name} {{"));
	lines.push(
		"\t/// Decode one event record and tell stale logs apart from future ones. The failure \
		 message mirrors the generated JavaScript decoder."
			.to_string(),
	);
	lines.push("\tpub fn try_from_bytes(".to_string());
	lines.push("\t\tdata: &[u8],".to_string());
	lines.push(format!("\t) -> Result<&{zc_name}, {error_enum}> {{"));
	lines.push(format!(
		"\t\tlet event = <Self as pina::PinaPodFixed>::read_exact(data)\n\t\t\t.map_err(|_| \
		 {error_enum}::InvalidData)?;"
	));
	if let Some(discriminator) = discriminator {
		lines.push(format!(
			"\t\tif event.discriminator != {} {{",
			discriminator.name
		));
		lines.push(format!("\t\t\treturn Err({error_enum}::InvalidData);"));
		lines.push("\t\t}".to_string());
	}
	if stale_possible {
		lines.push(format!(
			"\t\tif event.migration_version < {version_constant} {{"
		));
		lines.push(format!(
			"\t\t\treturn Err({error_enum}::Stale {{ stored: event.migration_version }});"
		));
		lines.push("\t\t}".to_string());
	}
	if future_possible {
		lines.push(format!(
			"\t\tif event.migration_version > {version_constant} {{"
		));
		lines.push(format!(
			"\t\t\treturn Err({error_enum}::Future {{ stored: event.migration_version }});"
		));
		lines.push("\t\t}".to_string());
	}
	lines.push("\t\tOk(event)".to_string());
	lines.push("\t}".to_string());
	lines.push("}".to_string());
	lines
}

fn event_migration_envelope(
	data_type: &StructTypeNode,
	discriminator: Option<&DiscriminatorInfo>,
) -> Option<EventEnvelope> {
	let mut fields = data_type.fields.iter().filter_map(|field| {
		let default_value = field.default_value.as_ref().as_ref()?;
		let kind = match field.name.as_ref() {
			"discriminator" => "discriminator",
			"migrationVersion" => "migrationVersion",
			_ => return None,
		};
		Some((kind, field.r#type.as_ref(), default_value))
	});
	let number_facts = |field_type: &TypeNode, default_value: &ValueNode| -> Option<(u64, usize)> {
		let ValueNode::Number(number_value) = default_value else {
			return None;
		};
		let TypeNode::Number(number_type) = field_type else {
			return None;
		};
		let width = match number_type.format {
			NumberFormat::U8 => 1,
			NumberFormat::U16 => 2,
			NumberFormat::U32 => 4,
			NumberFormat::U64 => 8,
			_ => return None,
		};
		let Number::UnsignedInteger(value) = number_value.number else {
			return None;
		};
		Some((value, width))
	};

	// The envelope only exists when the discriminator node is present, so a
	// missing discriminator constant makes this event non-migratable.
	discriminator?;
	let Some(("discriminator", field_type, default_value)) = fields.next() else {
		return None;
	};
	number_facts(field_type, default_value)?;
	let Some(("migrationVersion", field_type, default_value)) = fields.next() else {
		return None;
	};
	let (version, version_bytes) = number_facts(field_type, default_value)?;

	Some(EventEnvelope {
		version,
		version_ty: match version_bytes {
			1 => "u8".to_owned(),
			2 => "u16".to_owned(),
			4 => "u32".to_owned(),
			_ => "u64".to_owned(),
		},
		version_bytes,
	})
}

#[cfg(test)]
mod tests {
	use std::path::Path;

	use codama_nodes::BytesTypeNode;
	use codama_nodes::ConstantDiscriminatorNode;
	use codama_nodes::ConstantValueNode;
	use codama_nodes::DiscriminatorNode;
	use codama_nodes::NumberTypeNode;
	use codama_nodes::NumberValueNode;
	use codama_nodes::PublicKeyTypeNode;
	use codama_nodes::RootNode;
	use codama_nodes::SizeDiscriminatorNode;
	use codama_nodes::StringTypeNode;
	use codama_nodes::StringValueNode;
	use codama_nodes::StructFieldTypeNode;
	use codama_nodes::U8;

	use super::*;
	use crate::render_program_to_files;

	fn load_fixture_root(name: &str) -> RootNode {
		let path = Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("../..")
			.join("codama/idls")
			.join(format!("{name}.json"));
		let source = std::fs::read_to_string(&path)
			.unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
		serde_json::from_str(&source)
			.unwrap_or_else(|error| panic!("decode {}: {error}", path.display()))
	}

	#[test]
	fn renders_event_modules_with_their_envelope() {
		let root = load_fixture_root("migrations_program");
		let files =
			render_program_to_files(&root).unwrap_or_else(|error| panic!("event render: {error}"));

		let root_mod = files
			.get(Path::new("mod.rs"))
			.unwrap_or_else(|| panic!("root module must exist"));
		assert!(root_mod.contains("pub mod events;"));

		let module = files
			.get(Path::new("events/mod.rs"))
			.unwrap_or_else(|| panic!("events module barrel must exist"));
		assert!(module.contains("pub use self::r#value_changed_event::*;"));

		let page = files
			.get(Path::new("events/value_changed_event.rs"))
			.unwrap_or_else(|| panic!("event module must exist"));
		for expected in [
			"pub struct ValueChangedEvent {",
			"pub discriminator: u8,",
			"pub migration_version: u8,",
			"pub value: u64,",
			"pub memo: u16,",
			"pub const VALUE_CHANGED_EVENT_DISCRIMINATOR: u8 = 4u8;",
			"pub const VALUE_CHANGED_EVENT_MIGRATION_VERSION: u8 = 1u8;",
			"pub fn from_bytes(data: &[u8])",
			"pub fn try_from_bytes(",
			"pub enum ValueChangedEventVersionError",
			"event migration version mismatch: expected 1",
			"decode it with the event generated for that version",
		] {
			assert!(page.contains(expected), "missing `{expected}` in:\n{page}");
		}
		assert!(!page.contains("project_from_bytes"), "{page}");
	}

	/// A historical version is its own event node, decoded by its own module.
	#[test]
	fn renders_a_historical_event_version_as_its_own_module() {
		let event = envelope_event("valueChangedV0", U8, Number::UnsignedInteger(0));
		let page =
			render_event_page(&event).unwrap_or_else(|error| panic!("event render: {error}"));

		assert!(page.contains("pub struct ValueChangedV0 {"), "{page}");
		assert!(
			page.contains("pub const VALUE_CHANGED_V0_MIGRATION_VERSION: u8 = 0u8;"),
			"{page}"
		);
		assert!(
			page.contains("Future { stored: event.migration_version }"),
			"{page}"
		);
		assert!(page.contains("a later version emitted it"), "{page}");
	}

	#[test]
	fn renders_event_modules_without_history_as_current_only_decoders() {
		let root = load_fixture_root("events_program");
		let files =
			render_program_to_files(&root).unwrap_or_else(|error| panic!("event render: {error}"));

		let page = files
			.get(Path::new("events/my_event.rs"))
			.unwrap_or_else(|| panic!("event module must exist"));
		assert!(page.contains("pub struct MyEvent {"));
		assert!(page.contains("pub discriminator: u8,"));
		// The program envelopes events, so the current-only decoder still
		// carries the migration version constant and validates it.
		assert!(page.contains("pub migration_version: u8,"));
		assert!(page.contains("pub const MY_EVENT_MIGRATION_VERSION: u8 = 0u8;"));
		assert!(page.contains("pub fn from_bytes(data: &[u8])"));
		assert!(!page.contains("pub fn project_from_bytes("));
		assert!(!page.contains("PROJECTION_STEPS"));
	}

	#[test]
	fn event_discriminator_bytes_require_a_constant_node() {
		let mut root = load_fixture_root("events_program");
		for event in &mut root.program.events {
			event.discriminators.clear();
		}
		let files =
			render_program_to_files(&root).unwrap_or_else(|error| panic!("event render: {error}"));
		let page = files
			.get(Path::new("events/my_event.rs"))
			.unwrap_or_else(|| panic!("event module must exist"));
		assert!(!page.contains("MY_EVENT_DISCRIMINATOR"));
		assert!(!page.contains("pub fn try_from_bytes("));
	}

	fn event_number_field(name: &str, format: NumberFormat, number: Number) -> StructFieldTypeNode {
		let mut field =
			StructFieldTypeNode::new(name, TypeNode::Number(NumberTypeNode::le(format)));
		field.default_value = Box::new(Some(ValueNode::Number(NumberValueNode { number })));
		field.default_value_strategy = Some(DefaultValueStrategy::Omitted);
		field
	}

	fn event_constant_discriminator(format: NumberFormat, number: Number) -> DiscriminatorNode {
		DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
			ConstantValueNode::new(NumberTypeNode::le(format), NumberValueNode { number }),
			0,
		))
	}

	fn envelope_event(name: &str, version_format: NumberFormat, version: Number) -> EventNode {
		let data = StructTypeNode::new(vec![
			event_number_field("discriminator", U8, Number::UnsignedInteger(4)),
			event_number_field("migrationVersion", version_format, version),
			event_number_field("value", NumberFormat::U64, Number::UnsignedInteger(0)),
		]);
		let mut event = EventNode::new(name, data);
		event.discriminators = vec![event_constant_discriminator(U8, Number::UnsignedInteger(4))];
		event
	}

	fn drop_omitted_strategy(event: &mut EventNode) {
		if let TypeNode::Struct(data) = event.data.as_mut() {
			for field in &mut data.fields {
				field.default_value_strategy = None;
			}
		}
	}

	#[test]
	fn dropping_omitted_strategy_leaves_non_struct_events_alone() {
		let mut event = EventNode::new("bytesEvent", BytesTypeNode::new());
		drop_omitted_strategy(&mut event);
		assert!(matches!(event.data.as_ref(), TypeNode::Bytes(_)));
	}

	#[test]
	fn renders_event_pages_with_every_envelope_width() {
		let cases = [
			(U8, Number::UnsignedInteger(1), "u8", "1u8"),
			(NumberFormat::U16, Number::UnsignedInteger(2), "u16", "2u16"),
			(NumberFormat::U32, Number::UnsignedInteger(4), "u32", "4u32"),
			(NumberFormat::U64, Number::UnsignedInteger(8), "u64", "8u64"),
		];
		for (format, version, ty, literal) in cases {
			let event = envelope_event("valueChanged", format, version);
			let page =
				render_event_page(&event).unwrap_or_else(|error| panic!("event render: {error}"));

			assert!(
				page.contains(&format!(
					"_MIGRATION_VERSION: pina::Pod{}",
					ty.to_uppercase()
				)) || page.contains(&format!("_MIGRATION_VERSION: {ty} = {literal};")),
				"missing version constant for {ty} in:\n{page}"
			);
		}
	}

	#[test]
	fn version_zero_events_omit_the_impossible_stale_arm() {
		let event = envelope_event("valueChanged", U8, Number::UnsignedInteger(0));
		let page =
			render_event_page(&event).unwrap_or_else(|error| panic!("event render: {error}"));

		// A stored unsigned version is never below 0, and emitting the
		// impossible comparison trips the deny-by-default
		// `clippy::absurd_extreme_comparisons` in the generated crate.
		assert!(
			!page.contains("< VALUE_CHANGED_MIGRATION_VERSION"),
			"version 0 must not emit a stale arm:\n{page}"
		);
		assert!(page.contains("> VALUE_CHANGED_MIGRATION_VERSION"), "{page}");
	}

	#[test]
	fn maximal_events_omit_the_impossible_future_arm() {
		let event = envelope_event(
			"valueChanged",
			U8,
			Number::UnsignedInteger(u64::from(u8::MAX)),
		);
		let page =
			render_event_page(&event).unwrap_or_else(|error| panic!("event render: {error}"));

		assert!(
			!page.contains("> VALUE_CHANGED_MIGRATION_VERSION"),
			"a maximal version must not emit a future arm:\n{page}"
		);
		assert!(page.contains("< VALUE_CHANGED_MIGRATION_VERSION"), "{page}");
	}

	#[test]
	fn renders_event_docs_on_the_struct_and_its_fields() {
		let mut version = event_number_field("migrationVersion", U8, Number::UnsignedInteger(1));
		version.docs = vec!["Schema version.".to_owned()].into();
		let data = StructTypeNode::new(vec![
			event_number_field("discriminator", U8, Number::UnsignedInteger(4)),
			version,
			event_number_field("value", NumberFormat::U64, Number::UnsignedInteger(0)),
		]);
		let mut event = EventNode::new("valueChanged", data);
		event.discriminators = vec![event_constant_discriminator(U8, Number::UnsignedInteger(4))];
		event.docs = vec!["Tracks value changes.".to_owned()].into();
		let page = render_event_page(&event);
		let page = page.unwrap_or_else(|error| panic!("event render: {error}"));

		// Event docs describe the struct, not its first field.
		assert!(
			page.contains("/// Tracks value changes.\n#[allow(clippy::len_without_is_empty)]"),
			"{page}"
		);
		assert!(page.contains("/// Schema version."), "{page}");
		assert!(
			page.contains("Records of another version are rejected"),
			"{page}"
		);
	}

	#[test]
	fn events_without_an_envelope_document_a_single_layout() {
		let mut event = envelope_event("valueChanged", U8, Number::UnsignedInteger(1));
		if let TypeNode::Struct(data) = event.data.as_mut() {
			data.fields.remove(1);
		}
		let page = render_event_page(&event);
		let page = page.unwrap_or_else(|error| panic!("event render: {error}"));

		assert!(page.contains("/// Read one record of this event"), "{page}");
		assert!(!page.contains("Records of another version"), "{page}");
		assert!(!page.contains("migration_version"), "{page}");
	}

	#[test]
	fn render_event_page_rejects_non_struct_events() {
		let event = EventNode::new("badEvent", BytesTypeNode::new());
		let error = render_event_page(&event).expect_err("non-struct events have no fixed layout");
		assert!(matches!(error, RenderError::UnsupportedType { .. }));
	}

	#[test]
	fn event_envelopes_require_numeric_unsigned_defaults() {
		let mut events = Vec::new();

		// Discriminator field with a non-numeric type and a non-number default.
		let mut event = envelope_event("stringType", U8, Number::UnsignedInteger(1));
		if let TypeNode::Struct(data) = event.data.as_mut() {
			data.fields[0].r#type = Box::new(TypeNode::PublicKey(PublicKeyTypeNode::new()));
		}
		events.push(event);

		let mut event = envelope_event("stringDefault", U8, Number::UnsignedInteger(1));
		if let TypeNode::Struct(data) = event.data.as_mut() {
			data.fields[0].default_value =
				Box::new(Some(ValueNode::String(StringValueNode::new("4"))));
		}
		events.push(event);

		let mut event = envelope_event("migrationType", U8, Number::UnsignedInteger(1));
		if let TypeNode::Struct(data) = event.data.as_mut() {
			data.fields[1].r#type = Box::new(TypeNode::PublicKey(PublicKeyTypeNode::new()));
		}
		events.push(event);

		let mut event = envelope_event("migrationDefault", U8, Number::UnsignedInteger(1));
		if let TypeNode::Struct(data) = event.data.as_mut() {
			data.fields[1].default_value =
				Box::new(Some(ValueNode::String(StringValueNode::new("1"))));
		}
		events.push(event);

		// A 128-bit version or a signed default is not a Pina migration version.
		let mut event = envelope_event(
			"wideVersion",
			NumberFormat::U128,
			Number::UnsignedInteger(1),
		);
		if let TypeNode::Struct(data) = event.data.as_mut() {
			data.fields[1].r#type =
				Box::new(TypeNode::Number(NumberTypeNode::le(NumberFormat::U128)));
		}
		events.push(event);

		let mut event = envelope_event("signedVersion", U8, Number::UnsignedInteger(1));
		if let TypeNode::Struct(data) = event.data.as_mut() {
			data.fields[1].default_value =
				Box::new(Some(ValueNode::Number(NumberValueNode::new(-1_i8))));
		}
		events.push(event);

		// Envelope-shaped fields without a discriminator constant.
		let mut event = envelope_event("missingDiscriminator", U8, Number::UnsignedInteger(1));
		event.discriminators.clear();
		events.push(event);

		// A discriminator field with no migration version field.
		let mut event = envelope_event("missingVersion", U8, Number::UnsignedInteger(1));
		if let TypeNode::Struct(data) = event.data.as_mut() {
			data.fields.truncate(1);
		}
		events.push(event);

		// The version field appearing before the discriminator is not an envelope.
		let mut event = envelope_event("reorderedVersion", U8, Number::UnsignedInteger(1));
		if let TypeNode::Struct(data) = event.data.as_mut() {
			data.fields.swap(0, 1);
		}
		events.push(event);

		// Unrelated defaulted fields are skipped while scanning the envelope.
		let mut event = envelope_event("unrelatedDefault", U8, Number::UnsignedInteger(1));
		if let TypeNode::Struct(data) = event.data.as_mut() {
			let mut note = event_number_field("note", U8, Number::UnsignedInteger(7));
			note.default_value_strategy = None;
			data.fields.insert(1, note);
		}
		// The envelope still parses; the skipped field only exercises the scan.
		let parsed =
			render_event_page(&event).unwrap_or_else(|error| panic!("event render: {error}"));
		assert!(parsed.contains("MIGRATION_VERSION"), "{parsed}");

		for event in &mut events {
			drop_omitted_strategy(event);
			let label = format!("event `{}`", event.name.as_ref());
			let page =
				render_event_page(event).unwrap_or_else(|error| panic!("event render: {error}"));
			assert!(
				!page.contains("MIGRATION_VERSION"),
				"{label} must not render a version envelope:\n{page}",
			);
		}
	}
}
