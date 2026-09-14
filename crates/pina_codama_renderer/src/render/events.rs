//! Rust client rendering for event log records.
//!
//! Events are immutable transaction-log records, so generated event modules
//! are decode-only. Migration-aware events additionally expose the version
//! envelope, a direction-aware decode, and — when the checked-in manifest is
//! available at generation time — a projection that mirrors the runtime's
//! `normalize_event_data`.

use std::fmt::Write as _;

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
use crate::EventMigrationHistory;
use crate::error::RenderError;
use crate::error::Result;

/// The `[discriminator][migrationVersion]` envelope of one event.
struct EventEnvelope {
	discriminator_value: u64,
	discriminator_bytes: usize,
	version: u64,
	version_ty: String,
	version_bytes: usize,
}

impl EventEnvelope {
	fn discriminator_literal(&self) -> String {
		let encoded = self.discriminator_value.to_le_bytes();
		let bytes = encoded[..self.discriminator_bytes]
			.iter()
			.map(u8::to_string)
			.collect::<Vec<_>>()
			.join(", ");
		format!("[{bytes}]")
	}
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
pub(crate) fn render_event_page(
	event: &EventNode,
	history: Option<&EventMigrationHistory>,
) -> Result<String> {
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
	for doc_line in render_docs(&event.docs, 0) {
		field_lines.push(doc_line);
	}
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

	let mut lines = Vec::new();
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
		history,
	));

	Ok(lines.join("\n"))
}

fn render_decoders(
	event_name: &str,
	zc_name: &str,
	discriminator: Option<&DiscriminatorInfo>,
	omitted_constants: &[OmittedConstantInfo],
	envelope: Option<&EventEnvelope>,
	history: Option<&EventMigrationHistory>,
) -> Vec<String> {
	let mut lines = Vec::new();
	lines.push(format!("impl {event_name} {{"));
	lines.push("\t/// Exact size of the current event representation.".to_string());
	lines.push(format!(
		"\tpub const LEN: usize = core::mem::size_of::<{zc_name}>();"
	));
	lines.push(String::new());
	lines.push(
		"\t/// Read one current-version event record from transaction-log bytes.".to_string(),
	);
	lines.push("\t///".to_string());
	lines.push(
		"\t/// Logs carry the record base64-encoded after `Program data: `; pass the decoded \
		 bytes here. Historical and future records are rejected; `try_from_bytes` tells them \
		 apart, and `project_from_bytes` projects historical records when this client ships their \
		 transitions."
			.to_string(),
	);
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
		lines.extend(render_version_error(
			event_name,
			envelope,
			history.is_some(),
		));
		lines.push(String::new());
		lines.extend(render_try_from_bytes(
			event_name,
			zc_name,
			discriminator,
			envelope,
		));
		if let Some(history) = history {
			lines.push(String::new());
			lines.extend(render_projection(event_name, zc_name, envelope, history));
		}
	}

	lines
}

/// The direction-aware version error shared by `try_from_bytes` and the
/// projection entry point.
fn render_version_error(
	event_name: &str,
	envelope: &EventEnvelope,
	has_history: bool,
) -> Vec<String> {
	let error_enum = format!("{event_name}VersionError");
	let version = envelope.version;
	let version_ty = &envelope.version_ty;
	let stale_hint = if has_history {
		"the log predates this client; project it with the checked-in event history or decode it \
		 with a client generated from the schema that wrote it"
	} else {
		"the log predates this client and this client ships no event history; decode it with a \
		 client generated from the schema that wrote it"
	};
	let future_hint = "the log was written by a newer program; upgrade this client";

	let mut lines = Vec::new();
	lines.push(format!(
		"/// Why `{event_name}::try_from_bytes` rejected event bytes."
	));
	lines.push("#[derive(Clone, Copy, Debug, PartialEq, Eq)]".to_string());
	lines.push(format!("pub enum {error_enum} {{"));
	lines.push("\t/// The bytes do not decode as this event's layout at all.".to_string());
	lines.push("\tInvalidData,".to_string());
	lines.push(
		"\t/// The envelope names this event but the stored version predates this client."
			.to_string(),
	);
	lines.push(format!("\tStale {{ stored: {version_ty} }},"));
	lines.push(
		"\t/// The envelope names this event but the stored version is newer than this client's \
		 schema: upgrade this client."
			.to_string(),
	);
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

	// The envelope facts must stay in sync with the decoded constant.
	let _ = envelope;
	lines
}

/// The projection API for one event whose checked-in history is available.
fn render_projection(
	event_name: &str,
	zc_name: &str,
	envelope: &EventEnvelope,
	history: &EventMigrationHistory,
) -> Vec<String> {
	let error_enum = format!("{event_name}VersionError");
	let projection_error = format!("{event_name}ProjectionError");
	let projected = format!("Projected{event_name}");
	let steps_constant = format!("{}_PROJECTION_STEPS", event_name.to_shouty_snake_case());
	let version_ty = &envelope.version_ty;
	let version_literal = envelope.version;
	let version_constant = format!("{}_MIGRATION_VERSION", event_name.to_shouty_snake_case());
	let discriminator_constant = format!("{}_DISCRIMINATOR", event_name.to_shouty_snake_case());
	let header_size = envelope.discriminator_bytes + envelope.version_bytes;
	let version_end = header_size;
	let version_start = envelope.discriminator_bytes;

	let mut steps = String::new();
	for step in &history.steps {
		let moves = step
			.moves
			.iter()
			.map(|movement| {
				format!(
					"({}, {}, {})",
					movement.source_offset, movement.destination_offset, movement.size,
				)
			})
			.collect::<Vec<_>>()
			.join(", ");
		let _ = writeln!(
			steps,
			"\t({}, {}, {}, {}, {}, &[{}]),",
			step.from,
			step.to,
			step.automatic,
			step.source_payload_size,
			step.destination_payload_size,
			moves,
		);
	}

	let mut lines = Vec::new();
	lines.push(format!(
		"/// Why `{event_name}::project_from_bytes` could not produce current bytes."
	));
	lines.push("#[derive(Clone, Copy, Debug, PartialEq, Eq)]".to_string());
	lines.push(format!("pub enum {projection_error} {{"));
	lines.push("\t/// The bytes do not decode as this event's envelope.".to_string());
	lines.push("\tInvalidData,".to_string());
	lines.push(
		"\t/// The record's payload length does not match the schema for its version.".to_string(),
	);
	lines.push("\tInvalidLength { stored: u32 },".to_string());
	lines.push(
		"\t/// The log names this event but its adjacent transition is manual, so generated \
		 clients cannot project it."
			.to_string(),
	);
	lines.push("\tManual { from: u32, to: u32 },".to_string());
	lines.push(
		"\t/// The log names a version this client ships no checked-in projection for.".to_string(),
	);
	lines.push("\tUnknown { stored: u32 },".to_string());
	lines.push("}".to_string());
	lines.push(String::new());
	lines.push(format!("impl core::fmt::Display for {projection_error} {{"));
	lines.push(
		"\tfn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {".to_string(),
	);
	lines.push("\t\tmatch self {".to_string());
	lines.push(format!(
		"\t\t\tSelf::InvalidData => write!(f, \"invalid {event_name} event data\"),"
	));
	lines.push("\t\t\tSelf::InvalidLength { stored } => write!(".to_string());
	lines.push("\t\t\t\tf,".to_string());
	lines.push(format!(
		"\t\t\t\t\"event migration version mismatch: expected {version_literal}, received \
		 {{stored}} (the log length does not match the v{{stored}} schema)\""
	));
	lines.push("\t\t\t),".to_string());
	lines.push("\t\t\tSelf::Manual { from, to } => write!(".to_string());
	lines.push("\t\t\t\tf,".to_string());
	lines.push(format!(
		"\t\t\t\t\"event migration version mismatch: expected {version_literal}, received \
		 {{from}} (the v{{from}} to v{{to}} transition is manual, so only an on-chain projection \
		 or a client generated from that schema can represent it)\""
	));
	lines.push("\t\t\t),".to_string());
	lines.push("\t\t\tSelf::Unknown { stored } => write!(".to_string());
	lines.push("\t\t\t\tf,".to_string());
	lines.push(format!(
		"\t\t\t\t\"event migration version mismatch: expected {version_literal}, received \
		 {{stored}} (this client has no checked-in projection for it)\""
	));
	lines.push("\t\t\t),".to_string());
	lines.push("\t\t}".to_string());
	lines.push("\t}".to_string());
	lines.push("}".to_string());
	lines.push(String::new());
	lines.push(format!(
		"impl std::error::Error for {projection_error} {{}}"
	));
	lines.push(String::new());
	lines.push(
		"/// Current-shape event bytes paired with the version that actually wrote them."
			.to_string(),
	);
	lines.push("#[derive(Clone, Debug, PartialEq, Eq)]".to_string());
	lines.push(format!("pub struct {projected} {{"));
	lines.push("\tbytes: Vec<u8>,".to_string());
	lines.push(format!("\tsource_version: {version_ty},"));
	lines.push("\twas_migrated: bool,".to_string());
	lines.push("}".to_string());
	lines.push(String::new());
	lines.push(format!("impl {projected} {{"));
	lines.push("\t/// The exact current-version event bytes.".to_string());
	lines.push("\t#[must_use]".to_string());
	lines.push("\tpub fn bytes(&self) -> &[u8] {".to_string());
	lines.push("\t\t&self.bytes".to_string());
	lines.push("\t}".to_string());
	lines.push(String::new());
	lines.push(
		"\t/// The version carried by the immutable log record, matching the runtime's \
		 `CurrentEventData::source_version`."
			.to_string(),
	);
	lines.push("\t#[must_use]".to_string());
	lines.push(format!(
		"\tpub const fn source_version(&self) -> {version_ty} {{"
	));
	lines.push("\t\tself.source_version".to_string());
	lines.push("\t}".to_string());
	lines.push(String::new());
	lines.push("\t/// Whether a historical projection ran.".to_string());
	lines.push("\t#[must_use]".to_string());
	lines.push("\tpub const fn was_migrated(&self) -> bool {".to_string());
	lines.push("\t\tself.was_migrated".to_string());
	lines.push("\t}".to_string());
	lines.push(String::new());
	lines.push("\t/// Decode the projected bytes as the current event shape.".to_string());
	lines.push(format!(
		"\tpub fn data(&self) -> Result<&{zc_name}, {error_enum}> {{"
	));
	lines.push(format!("\t\t{event_name}::try_from_bytes(&self.bytes)"));
	lines.push("\t}".to_string());
	lines.push("}".to_string());
	lines.push(String::new());
	lines.push(
		"/// Adjacent projections from the checked-in migration manifest: `(from, to, automatic, \
		 source payload size, destination payload size, moves)`."
			.to_string(),
	);
	lines.push("#[allow(clippy::type_complexity)]".to_owned());
	lines.push(format!(
		"const {steps_constant}: &[(u32, u32, bool, usize, usize, &[(usize, usize, usize)])] = &["
	));
	lines.push(steps.trim_end().to_string());
	lines.push("];".to_string());
	lines.push(String::new());
	lines.push(format!("impl {event_name} {{"));
	lines.push(
		"\t/// Project current or historical event bytes into the current shape, mirroring the \
		 runtime's `normalize_event_data`."
			.to_string(),
	);
	lines.push("\t///".to_string());
	lines.push(
		"\t/// Unknown, future, non-exact historical lengths, and manual transitions fail closed. \
		 The returned bytes always carry the current version and are decoded by \
		 [`Self::try_from_bytes`]."
			.to_string(),
	);
	lines.push("\tpub fn project_from_bytes(".to_string());
	lines.push("\t\tdata: &[u8],".to_string());
	lines.push(format!("\t) -> Result<{projected}, {projection_error}> {{"));
	lines.push(format!("\t\tif data.len() < {header_size} {{"));
	lines.push(format!(
		"\t\t\treturn Err({projection_error}::InvalidData);"
	));
	lines.push("\t\t}".to_string());
	lines.push(format!(
		"\t\tif data[..{}] != {} {{",
		envelope.discriminator_bytes,
		envelope.discriminator_literal()
	));
	lines.push(format!(
		"\t\t\treturn Err({projection_error}::InvalidData);"
	));
	lines.push("\t\t}".to_string());
	lines.push(format!(
		"\t\tlet mut version = \
		 {version_ty}::from_le_bytes(\n\t\t\tdata[{version_start}..{version_end}]\n\t\t\t\t.\
		 try_into()\n\t\t\t\t.map_err(|_| {projection_error}::InvalidData)?,\n\t\t);"
	));
	lines.push(format!("\t\tlet expected = {version_constant};"));
	lines.push(format!(
		"\t\tif version > expected {{\n\t\t\treturn Err({projection_error}::Unknown {{ stored: \
		 u32::from(version) }});\n\t\t}}"
	));
	lines.push(format!(
		"\t\tlet source_version = version;\n\t\tlet mut payload = data[{header_size}..].to_vec();"
	));
	lines.push("\t\twhile version != expected {".to_string());
	lines.push(format!(
		"\t\t\tlet Some((from, to, automatic, source_size, destination_size, moves)) = \
		 {steps_constant}\n\t\t\t\t.iter()\n\t\t\t\t.find(|(from, ..)| *from == \
		 u32::from(version))\n\t\t\telse {{\n\t\t\t\treturn Err({projection_error}::Unknown {{ \
		 stored: u32::from(version) }});\n\t\t\t}};"
	));
	lines.push(format!(
		"\t\t\tif !*automatic {{\n\t\t\t\treturn Err({projection_error}::Manual {{ from: *from, \
		 to: *to }});\n\t\t\t}}"
	));
	lines.push(format!(
		"\t\t\tif payload.len() != *source_size {{\n\t\t\t\treturn \
		 Err({projection_error}::InvalidLength {{ stored: u32::from(version) }});\n\t\t\t}}"
	));
	lines.push("\t\t\tlet mut destination = vec![0_u8; *destination_size];".to_string());
	lines.push(
		concat!(
			"\t\t\tfor (source_offset, destination_offset, size) in *moves {\n",
			"\t\t\t\tdestination[*destination_offset..*destination_offset + *size]\n",
			"\t\t\t\t\t.copy_from_slice(&payload[*source_offset..*source_offset + *size]);\n",
			"\t\t\t}"
		)
		.to_string(),
	);
	lines.push("\t\t\tpayload = destination;".to_string());
	lines.push(format!(
		"\t\t\tversion = {version_ty}::try_from(*to)\n\t\t\t\t.map_err(|_| \
		 {projection_error}::Unknown {{ stored: *to }})?;"
	));
	lines.push("\t\t}".to_string());
	lines.push(format!(
		"\t\tlet mut bytes = Vec::with_capacity({header_size} + payload.len());"
	));
	lines.push(format!(
		"\t\tbytes.extend_from_slice(&{discriminator_constant}.to_le_bytes()[..{}]);",
		envelope.discriminator_bytes
	));
	let version_suffix = format!("{version_literal}{version_ty}");
	lines.push(format!(
		"\t\tbytes.extend_from_slice(&{version_suffix}.to_le_bytes()[..{}]);",
		envelope.version_bytes,
	));
	lines.push("\t\tbytes.extend_from_slice(&payload);".to_string());
	lines.push(format!(
		"\t\tOk({projected} {{\n\t\t\tbytes,\n\t\t\tsource_version,\n\t\t\twas_migrated: \
		 source_version != expected,\n\t\t}})"
	));
	lines.push("\t}".to_string());
	lines.push("}".to_string());
	lines.push(String::new());
	lines.extend(render_projection_tests(
		event_name, envelope, history, &projected,
	));
	lines
}

fn render_projection_tests(
	event_name: &str,
	envelope: &EventEnvelope,
	history: &EventMigrationHistory,
	projected: &str,
) -> Vec<String> {
	let version_ty = &envelope.version_ty;
	let header_size = envelope.discriminator_bytes + envelope.version_bytes;
	let version_start = envelope.discriminator_bytes;
	let projection_error = format!("{event_name}ProjectionError");
	let first_step = history.steps.first();
	// A saturating "next" version equals the current one when the current
	// version is the largest its width can represent, so the generated
	// future test would decode the current envelope and fail.
	let future_possible = envelope.version != version_type_max(envelope.version_bytes);
	let mut lines = Vec::new();

	lines.push("#[cfg(test)]".to_string());
	lines.push(format!("mod {}_projection_tests {{", snake(event_name)));
	lines.push("\tuse super::*;".to_string());
	lines.push(String::new());
	lines.push(format!(
		"\tfn record(version: {version_ty}, payload: &[u8]) -> Vec<u8> {{"
	));
	lines.push(format!(
		"\t\tlet mut data = vec![0_u8; {header_size} + payload.len()];"
	));
	lines.push(format!(
		"\t\tdata[..{}].copy_from_slice(&{});",
		envelope.discriminator_bytes,
		envelope.discriminator_literal()
	));
	lines.push(format!(
		"\t\tdata[{version_start}..{header_size}]\n\t\t\t.copy_from_slice(&version.to_le_bytes()[.\
		 .{}]);",
		envelope.version_bytes,
	));
	lines.push(format!(
		"\t\tdata[{header_size}..].copy_from_slice(payload);"
	));
	lines.push("\t\tdata".to_string());
	lines.push("\t}".to_string());
	lines.push(String::new());

	if let Some(step) = first_step {
		lines.push("\t#[test]".to_string());
		lines.push("\tfn historical_bytes_project_to_the_current_shape() {".to_string());
		lines.push(format!(
			"\t\tlet projected = {event_name}::project_from_bytes(&record(0, &[1_u8; \
			 {}]))\n\t\t\t.unwrap_or_else(|error| panic!(\"project: {{error}}\"));",
			step.source_payload_size,
		));
		lines.push(format!("\t\tlet _: &{projected} = &projected;"));
		lines.push("\t\tassert!(projected.was_migrated());".to_string());
		lines.push("\t\tassert_eq!(projected.source_version(), 0);".to_string());
		lines.push("\t\tassert!(projected.data().is_ok());".to_string());
		lines.push("\t}".to_string());
		lines.push(String::new());
	}

	if future_possible {
		lines.push("\t#[test]".to_string());
		lines.push("\tfn future_versions_fail_closed() {".to_string());
		lines.push(format!(
			"\t\tlet future: {version_ty} = {next};\n\t\tlet error = \
			 {event_name}::project_from_bytes(&record(future, \
			 &[]))\n\t\t\t.err()\n\t\t\t.expect(\"a future version must \
			 fail\");\n\t\tassert_eq!(error, {projection_error}::Unknown {{ stored: \
			 u32::from(future) }});",
			next = envelope.version.saturating_add(1),
		));
		lines.push("\t}".to_string());
		lines.push(String::new());
	} else {
		// No higher version exists, so the current maximum must decode as
		// the current schema, matching `try_from_bytes`.
		lines.push("\t#[test]".to_string());
		lines.push("\tfn maximal_version_decodes_as_current() {".to_string());
		lines.push(format!(
			"\t\tlet projected = {event_name}::project_from_bytes(&record({version} as \
			 {version_ty}, &[]))\n\t\t\t.unwrap_or_else(|error| panic!(\"project: {{error}}\"));",
			version = envelope.version,
		));
		lines.push("\t\tassert!(!projected.was_migrated());".to_string());
		lines.push(format!(
			"\t\tassert_eq!(projected.source_version(), {version});",
			version = envelope.version,
		));
		lines.push("\t}".to_string());
		lines.push(String::new());
	}
	lines.push("\t#[test]".to_string());
	lines.push("\tfn wrong_lengths_and_discriminators_fail_closed() {".to_string());
	if let Some(step) = first_step {
		lines.push(format!(
			"\t\tlet short = {event_name}::project_from_bytes(&record(0, &[0_u8; \
			 {}]))\n\t\t\t.err()\n\t\t\t.expect(\"a non-exact historical length must \
			 fail\");\n\t\tassert_eq!(\n\t\t\tshort,\n\t\t\t{projection_error}::InvalidLength {{ \
			 stored: 0 }},\n\t\t);",
			step.source_payload_size.saturating_sub(1).max(1),
		));
	}
	lines.push(format!(
		"\t\tlet mut foreign = record({}, &[]);\n\t\tforeign[0] = \
		 foreign[0].wrapping_add(1);\n\t\tassert_eq!(\n\t\t\t{event_name}::project_from_bytes(&\
		 foreign).err(),\n\t\t\tSome({projection_error}::InvalidData),\n\t\t);",
		envelope.version
	));
	lines.push("\t}".to_string());
	lines.push("}".to_string());
	let _ = version_ty;
	lines
}

/// Discriminator bytes of one event as stored at offset zero.
pub(crate) fn event_discriminator_bytes(event: &EventNode) -> Option<Vec<u8>> {
	let (value, width) = event.discriminators.iter().find_map(|discriminator| {
		let codama_nodes::DiscriminatorNode::Constant(constant) = discriminator else {
			return None;
		};
		let ValueNode::Number(number) = constant.constant.value.as_ref() else {
			return None;
		};
		let TypeNode::Number(number_type) = constant.constant.r#type.as_ref() else {
			return None;
		};
		let width = match number_type.format {
			NumberFormat::U8 => 1,
			NumberFormat::U16 => 2,
			NumberFormat::U32 => 4,
			NumberFormat::U64 => 8,
			_ => return None,
		};
		let Number::UnsignedInteger(value) = number.number else {
			return None;
		};
		Some((value, width))
	})?;
	Some(value.to_le_bytes()[..width].to_vec())
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
	let (discriminator_value, discriminator_bytes) = number_facts(field_type, default_value)?;
	let Some(("migrationVersion", field_type, default_value)) = fields.next() else {
		return None;
	};
	let (version, version_bytes) = number_facts(field_type, default_value)?;

	Some(EventEnvelope {
		discriminator_value,
		discriminator_bytes,
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
	use crate::EventFieldMove;
	use crate::EventProjectionStep;
	use crate::render_program_to_files_with_histories;

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
	fn renders_event_modules_with_envelope_and_projection() {
		let root = load_fixture_root("migrations_program");
		let histories = vec![EventMigrationHistory {
			rust_name: "ValueChangedEvent".to_owned(),
			discriminator: vec![4],
			current_version: 1,
			steps: vec![EventProjectionStep {
				from: 0,
				to: 1,
				automatic: true,
				source_payload_size: 8,
				destination_payload_size: 10,
				moves: vec![EventFieldMove {
					source_offset: 0,
					destination_offset: 0,
					size: 8,
				}],
			}],
		}];
		let files = render_program_to_files_with_histories(&root, &histories)
			.unwrap_or_else(|error| panic!("event render: {error}"));

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
			"pub fn project_from_bytes(",
			"pub struct ProjectedValueChangedEvent",
			"pub const fn source_version(&self) -> u8",
			"pub const fn was_migrated(&self) -> bool",
			"const VALUE_CHANGED_EVENT_PROJECTION_STEPS",
			"\t(0, 1, true, 8, 10, &[(0, 0, 8)]),",
			"event migration version mismatch: expected 1",
		] {
			assert!(page.contains(expected), "missing `{expected}` in:\n{page}");
		}
	}

	#[test]
	fn renders_event_modules_without_history_as_current_only_decoders() {
		let root = load_fixture_root("events_program");
		let files = render_program_to_files_with_histories(&root, &[])
			.unwrap_or_else(|error| panic!("event render: {error}"));

		let page = files
			.get(Path::new("events/my_event.rs"))
			.unwrap_or_else(|| panic!("event module must exist"));
		assert!(page.contains("pub struct MyEvent {"));
		assert!(page.contains("pub discriminator: u8,"));
		assert!(page.contains("pub fn from_bytes(data: &[u8])"));
		assert!(!page.contains("pub fn project_from_bytes("));
		assert!(!page.contains("MIGRATION_VERSION"));
	}

	#[test]
	fn event_discriminator_bytes_require_a_constant_node() {
		let mut root = load_fixture_root("events_program");
		for event in &mut root.program.events {
			event.discriminators.clear();
		}
		let files = render_program_to_files_with_histories(&root, &[])
			.unwrap_or_else(|error| panic!("event render: {error}"));
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

	fn envelope_history(automatic: bool) -> EventMigrationHistory {
		EventMigrationHistory {
			rust_name: "ValueChangedEvent".to_owned(),
			discriminator: vec![4],
			current_version: 1,
			steps: vec![EventProjectionStep {
				from: 0,
				to: 1,
				automatic,
				source_payload_size: 8,
				destination_payload_size: 10,
				moves: vec![EventFieldMove {
					source_offset: 0,
					destination_offset: 0,
					size: 8,
				}],
			}],
		}
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
			let page = render_event_page(&event, Some(&envelope_history(true)))
				.unwrap_or_else(|error| panic!("event render: {error}"));

			assert!(
				page.contains(&format!("{ty}::from_le_bytes")),
				"missing {ty} read in:\n{page}"
			);
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
			render_event_page(&event, None).unwrap_or_else(|error| panic!("event render: {error}"));

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
			render_event_page(&event, None).unwrap_or_else(|error| panic!("event render: {error}"));

		assert!(
			!page.contains("> VALUE_CHANGED_MIGRATION_VERSION"),
			"a maximal version must not emit a future arm:\n{page}"
		);
		assert!(page.contains("< VALUE_CHANGED_MIGRATION_VERSION"), "{page}");
	}

	#[test]
	fn maximal_projection_versions_pin_the_current_decode() {
		let event = envelope_event(
			"valueChanged",
			U8,
			Number::UnsignedInteger(u64::from(u8::MAX)),
		);
		let page = render_event_page(&event, Some(&envelope_history(true)))
			.unwrap_or_else(|error| panic!("event render: {error}"));

		// A saturating "next" version would equal the current one, so the
		// future test must be replaced by a current-decode assertion.
		assert!(
			!page.contains("future_versions_fail_closed"),
			"a maximal version has no future envelope to test:\n{page}"
		);
		assert!(
			page.contains("fn maximal_version_decodes_as_current()"),
			"{page}"
		);
		assert!(
			page.contains("assert!(!projected.was_migrated());"),
			"{page}"
		);
		assert!(page.contains("record(255 as u8, &[])"), "{page}");
	}

	#[test]
	fn renders_event_docs_on_the_struct_and_its_fields() {
		let mut event = envelope_event("valueChanged", U8, Number::UnsignedInteger(1));
		event.docs = vec!["Tracks value changes.".to_owned()].into();
		if let TypeNode::Struct(data) = event.data.as_mut() {
			data.fields[1].docs = vec!["Schema version.".to_owned()].into();
		}
		let page =
			render_event_page(&event, None).unwrap_or_else(|error| panic!("event render: {error}"));

		assert!(page.contains("/// Tracks value changes."), "{page}");
		assert!(page.contains("/// Schema version."), "{page}");
	}

	#[test]
	fn render_event_page_rejects_non_struct_events() {
		let event = EventNode::new("badEvent", BytesTypeNode::new());
		let error =
			render_event_page(&event, None).expect_err("non-struct events have no fixed layout");
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
			render_event_page(&event, None).unwrap_or_else(|error| panic!("event render: {error}"));
		assert!(parsed.contains("MIGRATION_VERSION"), "{parsed}");

		for event in &mut events {
			drop_omitted_strategy(event);
			let label = format!("event `{}`", event.name.as_ref());
			let page = render_event_page(event, Some(&envelope_history(true)))
				.unwrap_or_else(|error| panic!("event render: {error}"));
			assert!(
				!page.contains("MIGRATION_VERSION"),
				"{label} must not render a version envelope:\n{page}",
			);
		}
	}

	#[test]
	fn event_discriminator_bytes_cover_every_supported_width() {
		for (format, number, expected) in [
			(U8, Number::UnsignedInteger(4), vec![4]),
			(
				NumberFormat::U16,
				Number::UnsignedInteger(0x0102),
				vec![2, 1],
			),
			(
				NumberFormat::U32,
				Number::UnsignedInteger(0x0102_0304),
				vec![4, 3, 2, 1],
			),
			(
				NumberFormat::U64,
				Number::UnsignedInteger(0x0102_0304_0506_0708),
				vec![8, 7, 6, 5, 4, 3, 2, 1],
			),
		] {
			let mut event = envelope_event("valueChanged", U8, Number::UnsignedInteger(1));
			event.discriminators = vec![event_constant_discriminator(format, number)];
			assert_eq!(event_discriminator_bytes(&event), Some(expected));
		}
	}

	#[test]
	fn event_discriminator_bytes_reject_invalid_nodes() {
		let mut events = Vec::new();

		let mut event = envelope_event("valueChanged", U8, Number::UnsignedInteger(1));
		event.discriminators = vec![DiscriminatorNode::Size(SizeDiscriminatorNode::new(4))];
		events.push(event);

		let mut event = envelope_event("valueChanged", U8, Number::UnsignedInteger(1));
		event.discriminators = vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
			ConstantValueNode::new(
				NumberTypeNode::le(U8),
				ValueNode::String(StringValueNode::new("4")),
			),
			0,
		))];
		events.push(event);

		let mut event = envelope_event("valueChanged", U8, Number::UnsignedInteger(1));
		event.discriminators = vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
			ConstantValueNode::new(
				StringTypeNode::utf8(),
				ValueNode::Number(NumberValueNode::new(4_u8)),
			),
			0,
		))];
		events.push(event);

		let mut event = envelope_event("valueChanged", U8, Number::UnsignedInteger(1));
		event.discriminators = vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
			ConstantValueNode::new(
				NumberTypeNode::le(NumberFormat::F32),
				ValueNode::Number(NumberValueNode::new(4_u8)),
			),
			0,
		))];
		events.push(event);

		let mut event = envelope_event("valueChanged", U8, Number::UnsignedInteger(1));
		event.discriminators = vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
			ConstantValueNode::new(
				NumberTypeNode::le(U8),
				ValueNode::Number(NumberValueNode::new(-4_i8)),
			),
			0,
		))];
		events.push(event);

		for event in &events {
			let label = format!("event `{}`", event.name.as_ref());
			assert_eq!(
				event_discriminator_bytes(event),
				None,
				"{label} must have no usable discriminator",
			);
		}
	}

	#[test]
	fn event_projection_renders_manual_and_empty_histories() {
		let event = envelope_event("valueChanged", U8, Number::UnsignedInteger(1));

		let manual = envelope_history(false);
		let page = render_event_page(&event, Some(&manual))
			.unwrap_or_else(|error| panic!("event render: {error}"));
		assert!(
			page.contains("(0, 1, false, 8, 10, &[(0, 0, 8)]),"),
			"{page}"
		);

		let empty = EventMigrationHistory {
			steps: Vec::new(),
			..envelope_history(true)
		};
		let page = render_event_page(&event, Some(&empty))
			.unwrap_or_else(|error| panic!("event render: {error}"));
		assert!(
			!page.contains("historical_bytes_project_to_the_current_shape"),
			"{page}"
		);
		assert!(page.contains("future_versions_fail_closed"), "{page}");
	}
}
