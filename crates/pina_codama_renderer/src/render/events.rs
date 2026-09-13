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

	let envelope = event_migration_envelope(event, discriminator.as_ref());

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
	lines.push(format!(
		"\t\tif event.migration_version < {version_constant} {{"
	));
	lines.push(format!(
		"\t\t\treturn Err({error_enum}::Stale {{ stored: event.migration_version }});"
	));
	lines.push("\t\t}".to_string());
	lines.push(format!(
		"\t\tif event.migration_version > {version_constant} {{"
	));
	lines.push(format!(
		"\t\t\treturn Err({error_enum}::Future {{ stored: event.migration_version }});"
	));
	lines.push("\t\t}".to_string());
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

	lines.push("\t#[test]".to_string());
	lines.push("\tfn future_versions_fail_closed() {".to_string());
	lines.push(format!(
		"\t\tlet future: {version_ty} = {next};\n\t\tlet error = \
		 {event_name}::project_from_bytes(&record(future, &[]))\n\t\t\t.err()\n\t\t\t.expect(\"a 		 \
		 future version must fail\");\n\t\tassert_eq!(error, {projection_error}::Unknown {{ \
		 stored: u32::from(future) }});",
		next = envelope.version.saturating_add(1),
	));
	lines.push("\t}".to_string());
	lines.push(String::new());
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
	event: &EventNode,
	discriminator: Option<&DiscriminatorInfo>,
) -> Option<EventEnvelope> {
	let TypeNode::Struct(data_type) = event.data.as_ref() else {
		return None;
	};
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
