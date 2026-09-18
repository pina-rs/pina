//! Generated Dart modules for the event log read path.
//!
//! The Dart renderer does not emit event nodes yet, so Pina generates them
//! here from the Codama IDL. Every event gets a typed decoder and a
//! `Program data:` log entry point; migration-aware events additionally carry
//! the version envelope, a historical projection derived from the checked-in
//! migration manifest, and the source-version provenance the runtime exposes
//! through `CurrentEventData::source_version`.

use std::fmt::Write as _;
use std::path::Path;

use codama_nodes::CountNode;
use codama_nodes::EventNode;
use codama_nodes::NestedTypeNodeTrait;
use codama_nodes::NumberFormat;
use codama_nodes::ProgramNode;
use codama_nodes::RootNode;
use codama_nodes::StructTypeNode;
use codama_nodes::TypeNode;
use codama_nodes::ValueNode;
use heck::ToSnakeCase as _;

use crate::client_events::EventClientHistory;
use crate::client_events::EventClientHistoryIndex;
use crate::client_migrations::pascal_case;
use crate::error::CodamaError;

/// Which package imports a generated Dart file needs.
#[derive(Default)]
#[allow(
	clippy::struct_excessive_bools,
	reason = "each flag tracks one independent Dart package import"
)]
struct DartImports {
	addresses: bool,
	core: bool,
	data_structures: bool,
	numbers: bool,
	strings: bool,
	pod_helpers: bool,
}

impl DartImports {
	fn merge_into(&mut self, other: &mut Self) {
		other.addresses |= self.addresses;
		other.core |= self.core;
		other.data_structures |= self.data_structures;
		other.numbers |= self.numbers;
		other.strings |= self.strings;
		other.pod_helpers |= self.pod_helpers;
	}

	fn render(&self) -> String {
		let mut lines = vec!["import 'dart:typed_data';".to_owned()];
		if self.addresses {
			lines.push(
				"import 'package:solana_kit_addresses/solana_kit_addresses.dart';".to_owned(),
			);
		}
		if self.core {
			lines.push(
				"import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';".to_owned(),
			);
		}
		if self.data_structures {
			lines.push(
				"import 'package:solana_kit_codecs_data_structures/\
				 solana_kit_codecs_data_structures.dart';"
					.to_owned(),
			);
		}
		if self.numbers {
			lines.push(
				"import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';"
					.to_owned(),
			);
		}
		if self.strings {
			lines.push(
				"import 'package:solana_kit_codecs_strings/solana_kit_codecs_strings.dart';"
					.to_owned(),
			);
		}
		if self.pod_helpers {
			lines.push("import '../pina_pod_codecs.dart';".to_owned());
		}
		lines.join("\n")
	}
}

/// How one decoded field participates in the envelope.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DartFieldKind {
	Discriminator,
	Version,
	Payload,
}

/// One decoded event field.
struct DartField {
	name: String,
	ty: String,
	decoder: String,
	size: usize,
	kind: DartFieldKind,
}

/// The `[discriminator][migrationVersion]` envelope of one event.
struct DartEnvelope {
	version: u64,
	version_bytes: usize,
}

/// Everything the Dart emitter needs about one event.
struct EventFacts<'a> {
	name: String,
	pascal: String,
	fields: Vec<DartField>,
	imports: DartImports,
	discriminator: Vec<u8>,
	discriminator_value: u64,
	envelope: Option<DartEnvelope>,
	history: Option<&'a EventClientHistory>,
}

impl EventFacts<'_> {
	fn header_size(&self) -> usize {
		self.discriminator.len()
			+ self
				.envelope
				.as_ref()
				.map_or(0, |envelope| envelope.version_bytes)
	}

	fn current_size(&self) -> usize {
		self.fields.iter().map(|field| field.size).sum()
	}
}

/// Emit `events/` modules for one program and report whether the shared
/// `pina_pod_codecs.dart` helper is required.
pub(crate) fn emit_dart_event_modules(
	generated: &Path,
	program: &str,
	histories: &EventClientHistoryIndex,
	root: &RootNode,
) -> Result<bool, CodamaError> {
	if root.program.events.is_empty() {
		return Ok(false);
	}
	let program_pascal = pascal_case(&snake_to_camel(program));
	let mut events = Vec::new();
	for event in &root.program.events {
		let facts = event_facts(event, &root.program, histories).map_err(|message| {
			CodamaError::DartClient {
				path: generated.to_path_buf(),
				source: message.into(),
			}
		})?;
		let Some(facts) = facts else {
			return Err(CodamaError::DartClient {
				path: generated.to_path_buf(),
				source: format!(
					"event `{}` has no constant discriminator, so no log entry point can be \
					 generated for it",
					event.name.as_ref(),
				)
				.into(),
			});
		};
		events.push(facts);
	}

	let events_dir = generated.join("events");
	std::fs::create_dir_all(&events_dir).map_err(|source| {
		CodamaError::DartClient {
			path: events_dir.clone(),
			source: Box::new(source),
		}
	})?;

	let support_path = events_dir.join("event_log.dart");
	std::fs::write(&support_path, support_module(&program_pascal, &events)).map_err(|source| {
		CodamaError::DartClient {
			path: support_path.clone(),
			source: Box::new(source),
		}
	})?;

	let mut needs_pod_helper = false;
	let mut exports = vec!["event_log.dart".to_owned()];
	for facts in &events {
		let snake = facts.name.to_snake_case();
		let path = events_dir.join(format!("{snake}.dart"));
		let source = event_module(&program_pascal, facts);
		needs_pod_helper |= source.contains("getPinaPodBooleanDecoder");
		std::fs::write(&path, source).map_err(|source| {
			CodamaError::DartClient {
				path: path.clone(),
				source: Box::new(source),
			}
		})?;
		exports.push(format!("{snake}.dart"));
	}

	let barrel_path = events_dir.join("events.dart");
	let barrel = events_barrel(&program_pascal, &events, &exports);
	std::fs::write(&barrel_path, barrel).map_err(|source| {
		CodamaError::DartClient {
			path: barrel_path.clone(),
			source: Box::new(source),
		}
	})?;

	register_program_barrel(generated)?;

	Ok(needs_pod_helper)
}

/// Add the event barrel to the renderer's per-program barrel file.
fn register_program_barrel(generated: &Path) -> Result<(), CodamaError> {
	let Some(name) = generated
		.file_name()
		.map(|name| name.to_string_lossy().into_owned())
	else {
		return Ok(());
	};
	let barrel = generated.join(format!("{name}.dart"));
	if !barrel.exists() {
		return Ok(());
	}
	let source = std::fs::read_to_string(&barrel).map_err(|source| {
		CodamaError::DartClient {
			path: barrel.clone(),
			source: Box::new(source),
		}
	})?;
	if source.contains("events/events.dart") {
		return Ok(());
	}
	let patched = format!("{source}export 'events/events.dart';\n");
	std::fs::write(&barrel, patched).map_err(|source| {
		CodamaError::DartClient {
			path: barrel.clone(),
			source: Box::new(source),
		}
	})
}

/// The shared `event_log.dart` support module.
fn support_module(program_pascal: &str, events: &[EventFacts<'_>]) -> String {
	let wide_versions = events.iter().any(|facts| {
		facts
			.envelope
			.as_ref()
			.is_some_and(|env| env.version_bytes > 1)
	});
	let mut module = format!(
		r"// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:convert';
import 'dart:typed_data';

/// One event decoded from this program's transaction logs.
///
/// Every generated event class implements this interface so one log parser can
/// return a list of mixed events.
abstract class {program_pascal}Event {{
  /// Const constructor for generated subclasses.
  const {program_pascal}Event();

  /// The IDL event name this record was decoded as.
  String get name;
}}

/// Decode the base64 payload of one `Program data:` log line.
///
/// Returns `null` when the line is not a program-data record. Malformed base64
/// throws a [FormatException] naming the log line instead of silently dropping
/// the event.
Uint8List? decodeProgramDataLog(String log) {{
  const prefix = 'Program data: ';
  if (!log.startsWith(prefix)) {{
    return null;
  }}
  try {{
    return base64Decode(log.substring(prefix.length));
  }} on FormatException catch (error) {{
    throw FormatException(
      'invalid base64 in `Program data:` log line: ${{error.message}}',
    );
  }}
}}
"
	);
	if wide_versions {
		module.push_str(
			r"
/// Read a little-endian unsigned integer of `width` bytes.
int readLittleEndian(List<int> data, int offset, int width) {
  var value = 0;
  for (var index = 0; index < width; index++) {
    value |= data[offset + index] << (8 * index);
  }
  return value;
}

/// Encode a little-endian unsigned integer of `width` bytes.
Uint8List writeLittleEndian(int value, int width) {
  final bytes = Uint8List(width);
  for (var index = 0; index < width; index++) {
    bytes[index] = (value >> (8 * index)) & 0xff;
  }
  return bytes;
}
",
		);
	}
	module
}

/// The `events.dart` barrel with the program-level log parser.
fn events_barrel(program_pascal: &str, events: &[EventFacts<'_>], exports: &[String]) -> String {
	let mut lines = vec![
		"// Auto-generated. Do not edit.".to_owned(),
		"// ignore_for_file: type=lint".to_owned(),
		String::new(),
	];
	for export in exports {
		lines.push(format!("export '{export}';"));
	}
	lines.push(String::new());
	lines.push("import 'event_log.dart';".to_owned());
	for export in exports.iter().skip(1) {
		lines.push(format!("import '{export}';"));
	}
	lines.push(String::new());
	lines.push(
		"/// Decode every `Program data:` line that names one of this program's events.".to_owned(),
	);
	lines.push("///".to_owned());
	lines.push(
		"/// Unrelated lines and programs are skipped. A log that names an event but".to_owned(),
	);
	lines.push(
		"/// carries an unknown, future, or non-projectable version throws instead of".to_owned(),
	);
	lines.push("/// being silently dropped.".to_owned());
	lines.push(format!(
		"List<{program_pascal}Event> parse{program_pascal}EventsFromLogs(List<String> logs) {{"
	));
	lines.push(format!("  final discovered = <{program_pascal}Event>[];"));
	lines.push("  for (final log in logs) {".to_owned());
	for facts in events {
		let binding = lower_camel(&facts.pascal);
		lines.push(format!(
			"    final {binding} = parse{pascal}EventFromLog(log);",
			pascal = facts.pascal,
		));
		lines.push(format!("    if ({binding} != null) {{"));
		lines.push(format!("      discovered.add({binding});"));
		lines.push("      continue;".to_owned());
		lines.push("    }".to_owned());
	}
	lines.push("  }".to_owned());
	lines.push("  return discovered;".to_owned());
	lines.push("}".to_owned());
	lines.join("\n") + "\n"
}

/// The per-event module.
fn event_module(program_pascal: &str, facts: &EventFacts<'_>) -> String {
	let pascal = &facts.pascal;
	let camel = lower_camel(pascal);
	let class_name = format!("{pascal}Event");
	let imports = &facts.imports;

	let fields = facts
		.fields
		.iter()
		.map(|field| format!("\tfinal {} {};", field.ty, field.name))
		.collect::<Vec<_>>()
		.join("\n");
	let constructor_arguments = facts
		.fields
		.iter()
		.map(|field| format!("\t\trequired this.{},", field.name))
		.collect::<Vec<_>>()
		.join("\n");
	let to_string_fields = facts
		.fields
		.iter()
		.map(|field| {
			let name = &field.name;
			format!("{name}: ${{{name}}}")
		})
		.collect::<Vec<_>>()
		.join(", ");
	let size = facts.current_size();

	let mut lines = Vec::new();
	lines.push("// Auto-generated. Do not edit.".to_owned());
	lines.push("// ignore_for_file: type=lint".to_owned());
	lines.push(String::new());
	lines.push(imports.render());
	lines.push(String::new());
	lines.push("import 'event_log.dart';".to_owned());
	lines.push(String::new());
	lines.push(format!("/// Event record `{pascal}`."));
	if facts.envelope.is_none() {
		lines.push(format!(
			"class {class_name} extends {program_pascal}Event {{"
		));
	} else {
		lines.push(format!("class {class_name} {{"));
	}
	lines.push(format!("\tconst {class_name}({{"));
	lines.push(constructor_arguments);
	lines.push("\t});".to_owned());
	lines.push(String::new());
	lines.push(fields);
	lines.push(String::new());
	if facts.envelope.is_none() {
		lines.push("\t@override".to_owned());
	}
	lines.push(format!("\tString get name => '{}';", facts.name));
	lines.push(String::new());
	lines.push(format!(
		"\tString toString() => '{class_name}({to_string_fields})';"
	));
	lines.push("}".to_owned());
	lines.push(String::new());
	lines.push("/// The discriminator this event is emitted under.".to_owned());
	lines.push(format!(
		"const {camel}EventDiscriminator = {};",
		discriminator_literal(facts),
	));
	lines.push(String::new());
	lines.push("/// The discriminator bytes as stored at offset zero.".to_owned());
	lines.push(format!(
		"const List<int> _{camel}EventDiscriminatorBytes = {};",
		discriminator_bytes_literal(facts),
	));
	if let Some(envelope) = &facts.envelope {
		lines.push(String::new());
		lines.push("/// The version this client was generated from.".to_owned());
		lines.push(format!(
			"const {camel}EventMigrationVersion = {};",
			envelope.version,
		));
	}
	lines.push(String::new());
	lines.push(format!(
		"/// Exact current byte length of a `{pascal}` record, envelope included."
	));
	lines.push(format!("const {camel}EventSize = {size};"));
	lines.push(String::new());
	lines.push(format!("/// Decode one current-version `{pascal}` record."));
	lines.push(format!(
		"{class_name} decode{pascal}Event(Uint8List data) {{"
	));
	lines.push(format!("\tif (data.length != {camel}EventSize) {{"));
	lines.push(format!(
		"\t\tthrow RangeError('expected exactly ${{{camel}EventSize}} bytes, received \
		 ${{data.length}}');"
	));
	lines.push("\t}".to_owned());
	lines.push("\tvar cursor = 0;".to_owned());
	lines.push(decode_field_reads(facts));
	let arguments = facts
		.fields
		.iter()
		.enumerate()
		.map(|(index, field)| format!("{}: v{index}", field.name))
		.collect::<Vec<_>>()
		.join(", ");
	lines.push(format!("\treturn {class_name}({arguments});"));
	lines.push("}".to_owned());
	lines.push(String::new());

	if let (Some(envelope), Some(history)) = (&facts.envelope, facts.history) {
		lines.extend(projection_module(
			program_pascal,
			facts,
			envelope,
			history,
			&class_name,
			&camel,
		));
	}

	lines.push(format!("/// A decoded `{pascal}` log record."));
	lines.push(if facts.envelope.is_some() {
		format!("typedef Decoded{pascal}Event = Normalized{pascal}Event;")
	} else {
		format!("typedef Decoded{pascal}Event = {class_name};")
	});
	lines.push(String::new());
	lines.push(
		"/// Decode a `Program data:` log line, or return null when the line is not".to_owned(),
	);
	lines.push("/// this event.".to_owned());
	let return_type = if facts.envelope.is_some() {
		format!("Normalized{pascal}Event?")
	} else {
		format!("{class_name}?")
	};
	lines.push(format!(
		"{return_type} parse{pascal}EventFromLog(String log) {{"
	));
	lines.push("\tfinal bytes = decodeProgramDataLog(log);".to_owned());
	lines.push(format!(
		"\tif (bytes == null || bytes.length < {}) {{",
		facts.discriminator.len()
	));
	lines.push("\t\treturn null;".to_owned());
	lines.push("\t}".to_owned());
	lines.push(format!(
		"\tfor (var index = 0; index < {}; index++) {{",
		facts.discriminator.len()
	));
	lines.push(format!(
		"\t\tif (bytes[index] != _{camel}EventDiscriminatorBytes[index]) {{"
	));
	lines.push("\t\t\treturn null;".to_owned());
	lines.push("\t\t}".to_owned());
	lines.push("\t}".to_owned());
	if facts.envelope.is_some() {
		lines.push(format!("\treturn normalize{pascal}Event(bytes);"));
	} else {
		lines.push(format!("\treturn decode{pascal}Event(bytes);"));
	}
	lines.push("}".to_owned());

	lines.join("\n") + "\n"
}

/// The generated field reads plus the version guard.
fn decode_field_reads(facts: &EventFacts<'_>) -> String {
	let mut lines = String::new();
	for (index, field) in facts.fields.iter().enumerate() {
		let decoder = &field.decoder;
		let _ = writeln!(
			lines,
			"\tfinal (v{index}, c{index}) = {decoder}.read(data, cursor);"
		);
		let _ = writeln!(lines, "\tcursor = c{index};");
		// A `migrationVersion` field only exists on envelope events.
		if let (DartFieldKind::Version, Some(envelope)) = (&field.kind, facts.envelope.as_ref()) {
			let expected = envelope.version;
			let stale = "the log predates this client; project it through the checked-in event \
			             history or decode it with a client generated from the schema that wrote \
			             it";
			let future = "the log was written by a newer program; upgrade this client";
			let _ = writeln!(lines, "\tif (v{index} != {expected}) {{");
			let _ = writeln!(lines, "\t\tthrow RangeError(");
			let _ = writeln!(lines, "\t\t\tv{index} < {expected}");
			let _ = writeln!(
				lines,
				"\t\t\t\t? 'event migration version mismatch: expected {expected}, received \
				 $v{index} ({stale})'"
			);
			let _ = writeln!(
				lines,
				"\t\t\t\t: 'event migration version mismatch: expected {expected}, received \
				 $v{index} ({future})',"
			);
			let _ = writeln!(lines, "\t\t);");
			let _ = writeln!(lines, "\t}}");
		} else if field.kind == DartFieldKind::Discriminator {
			let _ = writeln!(
				lines,
				"\tif (v{index} != {}) {{",
				discriminator_literal(facts)
			);
			let _ = writeln!(
				lines,
				"\t\tthrow RangeError('the provided bytes do not match the \"{pascal}\" event \
				 discriminator');",
				pascal = facts.pascal,
			);
			let _ = writeln!(lines, "\t}}");
		}
	}
	lines
}

/// The normalization API for one migration-aware event.
fn projection_module(
	program_pascal: &str,
	facts: &EventFacts<'_>,
	envelope: &DartEnvelope,
	history: &EventClientHistory,
	class_name: &str,
	camel: &str,
) -> Vec<String> {
	let pascal = &facts.pascal;
	let current = envelope.version;
	let header_size = facts.header_size();
	let version_offset = facts.discriminator.len();
	let version_end = header_size;
	let wide_version = envelope.version_bytes > 1;
	let mut lines = Vec::new();

	let steps = history
		.steps
		.iter()
		.map(|step| {
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
			format!(
				"\t({}, {}, {}, {}, {}, [{}]),",
				step.from,
				step.to,
				step.automatic,
				step.source_payload_size,
				step.destination_payload_size,
				moves,
			)
		})
		.collect::<Vec<_>>()
		.join("\n");

	lines.push("/// Event bytes projected into the current shape.".to_owned());
	lines.push(format!(
		"class Normalized{pascal}Event extends {program_pascal}Event {{"
	));
	lines.push(format!("\tconst Normalized{pascal}Event({{"));
	lines.push("\t\trequired this.data,".to_owned());
	lines.push("\t\trequired this.sourceVersion,".to_owned());
	lines.push("\t\trequired this.wasMigrated,".to_owned());
	lines.push("\t});".to_owned());
	lines.push(String::new());
	lines.push(format!("\tfinal {class_name} data;"));
	lines.push(String::new());
	lines.push(
		"\t/// The version carried by the immutable log record, matching the runtime's".to_owned(),
	);
	lines.push("\t/// `CurrentEventData::source_version`.".to_owned());
	lines.push("\tfinal int sourceVersion;".to_owned());
	lines.push(String::new());
	lines.push("\t/// Whether a historical projection ran.".to_owned());
	lines.push("\tfinal bool wasMigrated;".to_owned());
	lines.push(String::new());
	lines.push("\t@override".to_owned());
	lines.push(format!("\tString get name => '{}';", facts.name));
	lines.push("}".to_owned());
	lines.push(String::new());
	lines.push(
		"/// Adjacent projections from the checked-in migration manifest: `(from, to,".to_owned(),
	);
	lines.push("/// automatic, source payload size, destination payload size, moves)`.".to_owned());
	lines.push(format!(
		"const List<(int, int, bool, int, int, List<(int, int, int)>)> _{camel}ProjectionSteps = ["
	));
	lines.push(steps);
	lines.push("];".to_owned());
	lines.push(String::new());
	lines.push(
		"/// Project current or historical bytes into the current shape, mirroring the".to_owned(),
	);
	lines.push(
		"/// runtime's `normalize_event_data`. Unknown, future, non-exact, and manual".to_owned(),
	);
	lines.push("/// transitions fail closed.".to_owned());
	lines.push(format!(
		"Normalized{pascal}Event normalize{pascal}Event(Uint8List data) {{"
	));
	lines.push(format!("\tif (data.length < {header_size}) {{"));
	lines.push(format!(
		"\t\tthrow RangeError('the provided data is too short for the \"{pascal}\" event \
		 envelope');"
	));
	lines.push("\t}".to_owned());
	lines.push(format!(
		"\tfor (var index = 0; index < {}; index++) {{",
		facts.discriminator.len()
	));
	lines.push(format!(
		"\t\tif (data[index] != _{camel}EventDiscriminatorBytes[index]) {{"
	));
	lines.push(format!(
		"\t\t\tthrow RangeError('the provided data does not match the \"{pascal}\" event \
		 discriminator');"
	));
	lines.push("\t\t}".to_owned());
	lines.push("\t}".to_owned());
	if wide_version {
		lines.push(format!(
			"\tfinal sourceVersion = readLittleEndian(data, {version_offset}, {});",
			envelope.version_bytes
		));
	} else {
		lines.push(format!("\tfinal sourceVersion = data[{version_offset}];"));
	}
	lines.push(format!("\tif (sourceVersion > {current}) {{"));
	lines.push("\t\tthrow RangeError(".to_owned());
	lines.push(format!(
		"\t\t\t'event migration version mismatch: expected {current}, received $sourceVersion \
		 (the log was written by a newer program; upgrade this client)',"
	));
	lines.push("\t\t);".to_owned());
	lines.push("\t}".to_owned());
	lines.push(format!("\tif (sourceVersion == {current}) {{"));
	lines.push(format!("\t\treturn Normalized{pascal}Event("));
	lines.push(format!("\t\t\tdata: decode{pascal}Event(data),"));
	lines.push("\t\t\tsourceVersion: sourceVersion,".to_owned());
	lines.push("\t\t\twasMigrated: false,".to_owned());
	lines.push("\t\t);".to_owned());
	lines.push("\t}".to_owned());
	lines.push(format!(
		"\tfinal projected = _project{pascal}Event(data, sourceVersion);"
	));
	lines.push(format!("\treturn Normalized{pascal}Event("));
	lines.push(format!("\t\tdata: decode{pascal}Event(projected),"));
	lines.push("\t\tsourceVersion: sourceVersion,".to_owned());
	lines.push("\t\twasMigrated: true,".to_owned());
	lines.push("\t);".to_owned());
	lines.push("}".to_owned());
	lines.push(String::new());
	lines.push(format!(
		"Uint8List _project{pascal}Event(Uint8List data, int sourceVersion) {{"
	));
	lines.push("\tvar version = sourceVersion;".to_owned());
	lines.push(format!(
		"\tvar payload = Uint8List.fromList(data.sublist({header_size}));"
	));
	lines.push(format!("\twhile (version != {current}) {{"));
	lines.push("\t\t(int, int, bool, int, int, List<(int, int, int)>)? step;".to_owned());
	lines.push(format!(
		"\t\tfor (final candidate in _{camel}ProjectionSteps) {{"
	));
	lines.push("\t\t\tif (candidate.$1 == version) {".to_owned());
	lines.push("\t\t\t\tstep = candidate;".to_owned());
	lines.push("\t\t\t\tbreak;".to_owned());
	lines.push("\t\t\t}".to_owned());
	lines.push("\t\t}".to_owned());
	lines.push("\t\tif (step == null) {".to_owned());
	lines.push("\t\t\tthrow RangeError(".to_owned());
	lines.push(format!(
		"\t\t\t\t'event migration version mismatch: expected {current}, received $version (this \
		 client has no checked-in projection for it)',"
	));
	lines.push("\t\t\t);".to_owned());
	lines.push("\t\t}".to_owned());
	lines.push("\t\tif (!step.$3) {".to_owned());
	lines.push("\t\t\tthrow RangeError(".to_owned());
	lines.push(format!(
		"\t\t\t\t'event migration version mismatch: expected {current}, received ${{step.$1}} \
		 (the v${{step.$1}} to v${{step.$2}} transition is manual, so only an on-chain projection \
		 or a client generated from that schema can represent it)',"
	));
	lines.push("\t\t\t);".to_owned());
	lines.push("\t\t}".to_owned());
	lines.push("\t\tif (payload.length != step.$4) {".to_owned());
	lines.push("\t\t\tthrow RangeError(".to_owned());
	lines.push(format!(
		"\t\t\t\t'event migration version mismatch: expected {current}, received $version (the \
		 log length does not match the v$version schema)',"
	));
	lines.push("\t\t\t);".to_owned());
	lines.push("\t\t}".to_owned());
	lines.push("\t\tfinal destination = Uint8List(step.$5);".to_owned());
	lines.push("\t\tfor (final (sourceOffset, destinationOffset, size) in step.$6) {".to_owned());
	lines.push(
		"\t\t\tdestination.setRange(destinationOffset, destinationOffset + size, payload, \
		 sourceOffset);"
			.to_owned(),
	);
	lines.push("\t\t}".to_owned());
	lines.push("\t\tpayload = destination;".to_owned());
	lines.push("\t\tversion = step.$2;".to_owned());
	lines.push("\t}".to_owned());
	lines.push(String::new());
	lines.push(format!(
		"\tfinal projected = Uint8List({header_size} + payload.length);"
	));
	lines.push(format!(
		"\tprojected.setRange(0, {}, _{camel}EventDiscriminatorBytes);",
		facts.discriminator.len()
	));
	if wide_version {
		lines.push(format!(
			"\tprojected.setRange({version_offset}, {version_end}, writeLittleEndian({current}, \
			 {}));",
			envelope.version_bytes
		));
	} else {
		lines.push(format!("\tprojected[{version_offset}] = {current};"));
	}
	lines.push(format!(
		"\tprojected.setRange({header_size}, projected.length, payload);"
	));
	lines.push("\treturn projected;".to_owned());
	lines.push("}".to_owned());
	lines
}

fn event_facts<'a>(
	event: &EventNode,
	program: &ProgramNode,
	histories: &'a EventClientHistoryIndex,
) -> Result<Option<EventFacts<'a>>, String> {
	let TypeNode::Struct(data) = event.data.as_ref() else {
		return Ok(None);
	};
	let Some((discriminator_value, discriminator_width)) = constant_discriminator(event) else {
		return Ok(None);
	};
	let discriminator = discriminator_value.to_le_bytes()[..discriminator_width].to_vec();
	let envelope = envelope_facts(event);
	let history = envelope.as_ref().and_then(|_| {
		let mut padded = [0_u8; 8];
		let width = discriminator.len().min(padded.len());
		padded[..width].copy_from_slice(&discriminator[..width]);
		histories.get(discriminator.len(), u64::from_le_bytes(padded))
	});
	let (fields, imports) = dart_fields(data, envelope.as_ref(), program)?;
	Ok(Some(EventFacts {
		name: event.name.as_ref().to_owned(),
		pascal: pascal_case(event.name.as_ref()),
		fields,
		imports,
		discriminator,
		discriminator_value,
		envelope,
		history,
	}))
}

fn envelope_facts(event: &EventNode) -> Option<DartEnvelope> {
	let TypeNode::Struct(data) = event.data.as_ref() else {
		return None;
	};
	let mut fields = data.fields.iter().filter_map(|field| {
		let default_value = field.default_value.as_ref().as_ref()?;
		let kind = match field.name.as_ref() {
			"discriminator" => "discriminator",
			"migrationVersion" => "migrationVersion",
			_ => return None,
		};
		Some((kind, field.r#type.as_ref(), default_value))
	});
	let number_facts = |field_type: &TypeNode, default_value: &ValueNode| -> Option<(u64, usize)> {
		let ValueNode::Number(number) = default_value else {
			return None;
		};
		let TypeNode::Number(number_type) = field_type else {
			return None;
		};
		let width = match number_type.format {
			NumberFormat::U8 => 1,
			NumberFormat::U16 => 2,
			NumberFormat::U32 => 4,
			_ => return None,
		};
		let codama_nodes::Number::UnsignedInteger(value) = number.number else {
			return None;
		};
		Some((value, width))
	};

	let Some(("discriminator", field_type, default_value)) = fields.next() else {
		return None;
	};
	number_facts(field_type, default_value)?;
	let Some(("migrationVersion", field_type, default_value)) = fields.next() else {
		return None;
	};
	let (version, version_bytes) = number_facts(field_type, default_value)?;

	Some(DartEnvelope {
		version,
		version_bytes,
	})
}

fn constant_discriminator(event: &EventNode) -> Option<(u64, usize)> {
	event.discriminators.iter().find_map(|discriminator| {
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
		let codama_nodes::Number::UnsignedInteger(value) = number.number else {
			return None;
		};
		Some((value, width))
	})
}

fn discriminator_literal(facts: &EventFacts<'_>) -> String {
	facts.discriminator_value.to_string()
}

/// `[4]`, `[0, 1]`, ... for byte-array comparisons.
fn discriminator_bytes_literal(facts: &EventFacts<'_>) -> String {
	let bytes = facts
		.discriminator
		.iter()
		.map(u8::to_string)
		.collect::<Vec<_>>()
		.join(", ");
	format!("[{bytes}]")
}

fn dart_fields(
	data: &StructTypeNode,
	envelope: Option<&DartEnvelope>,
	program: &ProgramNode,
) -> Result<(Vec<DartField>, DartImports), String> {
	let mut fields = Vec::new();
	let mut collected = DartImports::default();
	for field in &data.fields {
		let name = field.name.as_ref().to_owned();
		let kind = match name.as_str() {
			"discriminator" => DartFieldKind::Discriminator,
			"migrationVersion" if envelope.is_some() => DartFieldKind::Version,
			_ => DartFieldKind::Payload,
		};
		let mut imports = DartImports::default();
		let (ty, decoder, size) =
			dart_field(&field.r#type, program, &mut imports, 0).ok_or_else(|| {
				format!(
					"event field `{name}` uses a type the Dart event generator does not support \
					 yet; keep event fields fixed-size PinaPod primitives, bytes, strings, \
					 vectors, or options"
				)
			})?;
		imports.merge_into(&mut collected);
		fields.push(DartField {
			name,
			ty,
			decoder,
			size,
			kind,
		});
	}
	Ok((fields, collected))
}

/// Map one Codama type node to `(Dart type, decoder expression, byte size)`.
fn dart_field(
	node: &TypeNode,
	program: &ProgramNode,
	imports: &mut DartImports,
	depth: usize,
) -> Option<(String, String, usize)> {
	if depth > 8 {
		return None;
	}
	match node {
		TypeNode::Number(number) => {
			imports.numbers = true;
			let (decoder, ty, size) = match number.format {
				NumberFormat::U8 => ("getU8Decoder()", "int", 1),
				NumberFormat::I8 => ("getI8Decoder()", "int", 1),
				NumberFormat::U16 => ("getU16Decoder()", "int", 2),
				NumberFormat::I16 => ("getI16Decoder()", "int", 2),
				NumberFormat::U32 => ("getU32Decoder()", "int", 4),
				NumberFormat::I32 => ("getI32Decoder()", "int", 4),
				NumberFormat::U64 => ("getU64Decoder()", "BigInt", 8),
				NumberFormat::I64 => ("getI64Decoder()", "BigInt", 8),
				NumberFormat::U128 => ("getU128Decoder()", "BigInt", 16),
				NumberFormat::I128 => ("getI128Decoder()", "BigInt", 16),
				_ => return None,
			};
			Some((ty.to_owned(), decoder.to_owned(), size))
		}
		TypeNode::Boolean(_) => {
			imports.pod_helpers = true;
			Some((
				"bool".to_owned(),
				"getPinaPodBooleanDecoder()".to_owned(),
				1,
			))
		}
		TypeNode::PublicKey(_) => {
			imports.addresses = true;
			Some(("Address".to_owned(), "getAddressDecoder()".to_owned(), 32))
		}
		TypeNode::FixedSize(fixed) => {
			let inner = fixed.r#type.as_ref();
			if matches!(inner, TypeNode::Bytes(_)) {
				imports.core = true;
				imports.data_structures = true;
				return Some((
					"Uint8List".to_owned(),
					format!("fixDecoderSize(getBytesDecoder(), {})", fixed.size),
					fixed.size,
				));
			}
			if let TypeNode::SizePrefix(prefix) = inner
				&& matches!(prefix.r#type.as_ref(), TypeNode::String(_))
			{
				imports.core = true;
				imports.strings = true;
				let prefix_type = TypeNode::Number(prefix.prefix.get_nested_type_node().clone());
				let (_, prefix_decoder, _) = dart_field(&prefix_type, program, imports, depth + 1)?;
				return Some((
					"String".to_owned(),
					format!(
						"fixDecoderSize(addDecoderSizePrefix(getUtf8Decoder(), {prefix_decoder}), \
						 {})",
						fixed.size
					),
					fixed.size,
				));
			}
			if let TypeNode::Array(array) = inner {
				return dart_fixed_array(array, fixed.size, program, imports, depth);
			}
			let (ty, decoder, size) = dart_field(inner, program, imports, depth + 1)?;
			if size != fixed.size {
				return None;
			}
			imports.core = true;
			Some((
				ty,
				format!("fixDecoderSize({decoder}, {})", fixed.size),
				fixed.size,
			))
		}
		TypeNode::Array(array) => dart_array(array, program, imports, depth),
		TypeNode::Option(option) if option.fixed == Some(true) => {
			imports.data_structures = true;
			let (ty, decoder, item_size) = dart_field(&option.item, program, imports, depth + 1)?;
			let prefix_size = option_prefix_size(option.prefix.get_nested_type_node())?;
			Some((
				format!("{ty}?"),
				format!("getNullableDecoder<{ty}>({decoder}, noneValue: const ZeroesNoneValue())"),
				prefix_size + item_size,
			))
		}
		TypeNode::Link(link) => {
			let defined = program
				.defined_types
				.iter()
				.find(|defined| defined.name.as_ref() == link.name.as_ref())?;
			dart_field(defined.r#type.as_ref(), program, imports, depth + 1)
		}
		_ => None,
	}
}

/// A fixed-size array: the wrapper size is authoritative and includes padding.
fn dart_fixed_array(
	array: &codama_nodes::ArrayTypeNode,
	outer_size: usize,
	program: &ProgramNode,
	imports: &mut DartImports,
	depth: usize,
) -> Option<(String, String, usize)> {
	imports.core = true;
	imports.data_structures = true;
	let (item_ty, item_decoder, item_size) = dart_field(&array.item, program, imports, depth + 1)?;
	let size = match array.count.as_ref() {
		CountNode::Fixed(count) => format!("FixedArraySize({})", count.value),
		CountNode::Prefixed(count) => {
			let prefix_type = TypeNode::Number(count.prefix.get_nested_type_node().clone());
			let (_, prefix_decoder, _) = dart_field(&prefix_type, program, imports, depth + 1)?;
			let _ = item_size;
			format!("PrefixedArraySize({prefix_decoder})")
		}
		CountNode::Remainder(_) => return None,
	};
	Some((
		format!("List<{item_ty}>"),
		format!("fixDecoderSize(getArrayDecoder({item_decoder}, size: {size}), {outer_size})"),
		outer_size,
	))
}

fn dart_array(
	array: &codama_nodes::ArrayTypeNode,
	program: &ProgramNode,
	imports: &mut DartImports,
	depth: usize,
) -> Option<(String, String, usize)> {
	imports.data_structures = true;
	let (item_ty, item_decoder, item_size) = dart_field(&array.item, program, imports, depth + 1)?;
	match array.count.as_ref() {
		CountNode::Fixed(count) => {
			Some((
				format!("List<{item_ty}>"),
				format!(
					"getArrayDecoder({item_decoder}, size: FixedArraySize({}))",
					count.value
				),
				item_size.checked_mul(count.value as usize)?,
			))
		}
		// A prefixed array without a fixed-size wrapper has no IDL-derivable
		// capacity, so its byte size is not knowable from the IDL alone.
		_ => None,
	}
}

fn option_prefix_size(number: &codama_nodes::NumberTypeNode) -> Option<usize> {
	match number.format {
		NumberFormat::U8 => Some(1),
		NumberFormat::U16 => Some(2),
		NumberFormat::U32 => Some(4),
		_ => None,
	}
}

fn snake_to_camel(snake: &str) -> String {
	let mut out = String::with_capacity(snake.len());
	for (index, segment) in snake.split('_').enumerate() {
		if index == 0 {
			out.push_str(segment);
		} else {
			let mut characters = segment.chars();
			if let Some(first) = characters.next() {
				out.push(first.to_ascii_uppercase());
				out.push_str(characters.as_str());
			}
		}
	}
	out
}

fn lower_camel(pascal: &str) -> String {
	let mut characters = pascal.chars();
	match characters.next() {
		Some(first) => first.to_lowercase().collect::<String>() + characters.as_str(),
		None => String::new(),
	}
}

#[cfg(test)]
mod tests {
	use codama_nodes::ArrayTypeNode;
	use codama_nodes::BooleanTypeNode;
	use codama_nodes::BytesTypeNode;
	use codama_nodes::ConstantDiscriminatorNode;
	use codama_nodes::ConstantValueNode;
	use codama_nodes::DefaultValueStrategy;
	use codama_nodes::DefinedTypeLinkNode;
	use codama_nodes::DefinedTypeNode;
	use codama_nodes::DiscriminatorNode;
	use codama_nodes::EventNode;
	use codama_nodes::FixedCountNode;
	use codama_nodes::FixedSizeTypeNode;
	use codama_nodes::Number;
	use codama_nodes::NumberFormat;
	use codama_nodes::NumberTypeNode;
	use codama_nodes::NumberValueNode;
	use codama_nodes::OptionTypeNode;
	use codama_nodes::PrefixedCountNode;
	use codama_nodes::ProgramNode;
	use codama_nodes::PublicKeyTypeNode;
	use codama_nodes::RemainderCountNode;
	use codama_nodes::RootNode;
	use codama_nodes::SizeDiscriminatorNode;
	use codama_nodes::SizePrefixTypeNode;
	use codama_nodes::StringTypeNode;
	use codama_nodes::StringValueNode;
	use codama_nodes::StructFieldTypeNode;
	use codama_nodes::StructTypeNode;
	use codama_nodes::TypeNode;
	use codama_nodes::ValueNode;

	use super::*;

	fn read_idl(name: &str) -> RootNode {
		let path = Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("../..")
			.join("codama/idls")
			.join(name);
		let source = std::fs::read_to_string(&path)
			.unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
		serde_json::from_str(&source)
			.unwrap_or_else(|error| panic!("decode {}: {error}", path.display()))
	}

	fn history() -> EventClientHistory {
		EventClientHistory {
			rust_name: "ValueChangedEvent".to_owned(),
			discriminator: vec![4],
			current_version: 1,
			steps: vec![crate::client_events::EventProjectionStep {
				from: 0,
				to: 1,
				automatic: true,
				source_payload_size: 8,
				destination_payload_size: 10,
				moves: vec![crate::client_events::EventFieldMove {
					source_offset: 0,
					destination_offset: 0,
					size: 8,
				}],
			}],
		}
	}

	fn test_program() -> ProgramNode {
		ProgramNode::new("eventsProgram", "11111111111111111111111111111111")
	}

	fn number(format: NumberFormat) -> TypeNode {
		TypeNode::Number(NumberTypeNode::le(format))
	}

	fn plain_field(name: &str, r#type: TypeNode) -> StructFieldTypeNode {
		StructFieldTypeNode::new(name, r#type)
	}

	fn event_number_field(name: &str, format: NumberFormat, value: Number) -> StructFieldTypeNode {
		let mut field = StructFieldTypeNode::new(name, number(format));
		field.default_value = Box::new(Some(ValueNode::Number(NumberValueNode { number: value })));
		field.default_value_strategy = Some(DefaultValueStrategy::Omitted);
		field
	}

	fn event_discriminator(format: NumberFormat, value: Number) -> DiscriminatorNode {
		DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
			ConstantValueNode::new(
				NumberTypeNode::le(format),
				NumberValueNode { number: value },
			),
			0,
		))
	}

	fn envelope_event(name: &str, version_format: NumberFormat, version: Number) -> EventNode {
		let data = StructTypeNode::new(vec![
			event_number_field(
				"discriminator",
				NumberFormat::U8,
				Number::UnsignedInteger(4),
			),
			event_number_field("migrationVersion", version_format, version),
			plain_field("value", number(NumberFormat::U64)),
		]);
		let mut event = EventNode::new(name, data);
		event.discriminators = vec![event_discriminator(
			NumberFormat::U8,
			Number::UnsignedInteger(4),
		)];
		event
	}

	fn fixed(inner: TypeNode, size: usize) -> TypeNode {
		TypeNode::FixedSize(FixedSizeTypeNode::<TypeNode>::new(inner, size))
	}

	#[test]
	fn dart_fields_cover_every_supported_node_type() {
		let mut program = test_program();
		program
			.defined_types
			.push(DefinedTypeNode::new("Amount", number(NumberFormat::U64)));
		let data = StructTypeNode::new(vec![
			plain_field("u8", number(NumberFormat::U8)),
			plain_field("i8", number(NumberFormat::I8)),
			plain_field("u16", number(NumberFormat::U16)),
			plain_field("i16", number(NumberFormat::I16)),
			plain_field("u32", number(NumberFormat::U32)),
			plain_field("i32", number(NumberFormat::I32)),
			plain_field("u64", number(NumberFormat::U64)),
			plain_field("i64", number(NumberFormat::I64)),
			plain_field("u128", number(NumberFormat::U128)),
			plain_field("i128", number(NumberFormat::I128)),
			plain_field(
				"flag",
				TypeNode::Boolean(BooleanTypeNode::new(NumberTypeNode::le(NumberFormat::U8))),
			),
			plain_field("owner", TypeNode::PublicKey(PublicKeyTypeNode::new())),
			plain_field("raw", fixed(TypeNode::Bytes(BytesTypeNode::new()), 8)),
			plain_field(
				"label",
				fixed(
					TypeNode::SizePrefix(SizePrefixTypeNode::<TypeNode>::new(
						StringTypeNode::utf8(),
						NumberTypeNode::le(NumberFormat::U8),
					)),
					33,
				),
			),
			plain_field(
				"tags",
				fixed(
					TypeNode::Array(ArrayTypeNode::new(
						number(NumberFormat::U8),
						PrefixedCountNode::new(NumberTypeNode::le(NumberFormat::U16)),
					)),
					6,
				),
			),
			plain_field(
				"pair",
				fixed(
					TypeNode::Array(ArrayTypeNode::new(
						number(NumberFormat::U8),
						FixedCountNode::new(2),
					)),
					2,
				),
			),
			plain_field(
				"maybe",
				fixed(
					TypeNode::Option(OptionTypeNode {
						fixed: Some(true),
						item: Box::new(number(NumberFormat::U8)),
						prefix: NumberTypeNode::le(NumberFormat::U8).into(),
					}),
					2,
				),
			),
			plain_field(
				"maybe16",
				fixed(
					TypeNode::Option(OptionTypeNode {
						fixed: Some(true),
						item: Box::new(number(NumberFormat::U8)),
						prefix: NumberTypeNode::le(NumberFormat::U16).into(),
					}),
					3,
				),
			),
			plain_field(
				"maybe32",
				fixed(
					TypeNode::Option(OptionTypeNode {
						fixed: Some(true),
						item: Box::new(number(NumberFormat::U8)),
						prefix: NumberTypeNode::le(NumberFormat::U32).into(),
					}),
					5,
				),
			),
			plain_field(
				"items",
				TypeNode::Array(ArrayTypeNode::new(
					number(NumberFormat::U16),
					FixedCountNode::new(3),
				)),
			),
			plain_field("amount", TypeNode::Link(DefinedTypeLinkNode::new("Amount"))),
			plain_field("sized", fixed(number(NumberFormat::U8), 1)),
		]);
		let (fields, imports) =
			dart_fields(&data, None, &program).unwrap_or_else(|error| panic!("fields: {error}"));
		assert_eq!(fields.len(), 22);

		let field = |name: &str| {
			fields
				.iter()
				.find(|field| field.name == name)
				.unwrap_or_else(|| panic!("field `{name}` must exist"))
		};
		assert_eq!(field("u8").ty, "int");
		assert_eq!(field("i8").decoder, "getI8Decoder()");
		assert_eq!(field("i16").decoder, "getI16Decoder()");
		assert_eq!(field("u32").decoder, "getU32Decoder()");
		assert_eq!(field("i32").decoder, "getI32Decoder()");
		assert_eq!((field("u64").ty.as_str(), field("u64").size), ("BigInt", 8));
		assert_eq!(field("i64").decoder, "getI64Decoder()");
		assert_eq!(field("u128").decoder, "getU128Decoder()");
		assert_eq!(field("i128").ty, "BigInt");
		assert_eq!(field("flag").decoder, "getPinaPodBooleanDecoder()");
		assert_eq!(
			(field("owner").ty.as_str(), field("owner").size),
			("Address", 32)
		);
		assert_eq!(field("raw").decoder, "fixDecoderSize(getBytesDecoder(), 8)");
		assert_eq!(field("label").ty, "String");
		assert!(
			field("label")
				.decoder
				.contains("addDecoderSizePrefix(getUtf8Decoder(), getU8Decoder())")
		);
		assert_eq!(field("tags").ty, "List<int>");
		assert!(
			field("tags")
				.decoder
				.contains("PrefixedArraySize(getU16Decoder())")
		);
		assert!(field("pair").decoder.contains("FixedArraySize(2)"));
		assert!(field("maybe").decoder.contains("getNullableDecoder<int>"));
		assert_eq!(field("maybe").ty, "int?");
		assert_eq!(field("maybe16").size, 3);
		assert_eq!(field("maybe32").size, 5);
		assert_eq!(field("items").ty, "List<int>");
		assert!(field("items").decoder.contains("FixedArraySize(3)"));
		assert_eq!(field("amount").decoder, "getU64Decoder()");
		assert_eq!(field("sized").decoder, "fixDecoderSize(getU8Decoder(), 1)");

		assert!(imports.addresses);
		assert!(imports.core);
		assert!(imports.data_structures);
		assert!(imports.numbers);
		assert!(imports.strings);
		assert!(imports.pod_helpers);
	}

	#[test]
	fn event_module_renders_imports_for_every_supported_family() {
		let mut program = test_program();
		program
			.defined_types
			.push(DefinedTypeNode::new("Amount", number(NumberFormat::U64)));
		let data = StructTypeNode::new(vec![
			plain_field("owner", TypeNode::PublicKey(PublicKeyTypeNode::new())),
			plain_field(
				"label",
				fixed(
					TypeNode::SizePrefix(SizePrefixTypeNode::<TypeNode>::new(
						StringTypeNode::utf8(),
						NumberTypeNode::le(NumberFormat::U8),
					)),
					33,
				),
			),
			plain_field(
				"flag",
				TypeNode::Boolean(BooleanTypeNode::new(NumberTypeNode::le(NumberFormat::U8))),
			),
			plain_field("raw", fixed(TypeNode::Bytes(BytesTypeNode::new()), 4)),
			plain_field(
				"items",
				TypeNode::Array(ArrayTypeNode::new(
					number(NumberFormat::U64),
					FixedCountNode::new(2),
				)),
			),
		]);
		let mut event = EventNode::new("richEvent", data);
		event.discriminators = vec![event_discriminator(
			NumberFormat::U8,
			Number::UnsignedInteger(9),
		)];
		let histories = EventClientHistoryIndex::default();
		let facts = event_facts(&event, &program, &histories)
			.unwrap_or_else(|error| panic!("facts: {error}"))
			.unwrap_or_else(|| panic!("event facts"));
		let module = event_module("EventsProgram", &facts);

		assert!(module.contains("package:solana_kit_addresses"), "{module}");
		assert!(
			module.contains("package:solana_kit_codecs_strings"),
			"{module}"
		);
		assert!(
			module.contains("package:solana_kit_codecs_data_structures"),
			"{module}"
		);
		assert!(
			module.contains("package:solana_kit_codecs_core"),
			"{module}"
		);
		assert!(
			module.contains("package:solana_kit_codecs_numbers"),
			"{module}"
		);
		assert!(
			module.contains("import '../pina_pod_codecs.dart';"),
			"{module}"
		);
	}

	#[test]
	fn dart_field_rejects_unsupported_shapes() {
		let program = test_program();
		let mut imports = DartImports::default();
		let option = |fixed_flag: Option<bool>, item: TypeNode, prefix: NumberFormat| {
			TypeNode::Option(OptionTypeNode {
				fixed: fixed_flag,
				item: Box::new(item),
				prefix: NumberTypeNode::le(prefix).into(),
			})
		};
		let unsupported = [
			number(NumberFormat::F32),
			TypeNode::String(StringTypeNode::utf8()),
			TypeNode::Bytes(BytesTypeNode::new()),
			fixed(number(NumberFormat::U8), 2),
			fixed(
				TypeNode::Array(ArrayTypeNode::new(
					number(NumberFormat::U8),
					RemainderCountNode::new(),
				)),
				4,
			),
			TypeNode::Array(ArrayTypeNode::new(
				number(NumberFormat::U8),
				RemainderCountNode::new(),
			)),
			TypeNode::Array(ArrayTypeNode::new(
				number(NumberFormat::U8),
				PrefixedCountNode::new(NumberTypeNode::le(NumberFormat::U16)),
			)),
			option(None, number(NumberFormat::U8), NumberFormat::U8),
			option(Some(true), number(NumberFormat::U8), NumberFormat::U64),
			TypeNode::Link(DefinedTypeLinkNode::new("Missing")),
		];
		for node in &unsupported {
			assert!(
				dart_field(node, &program, &mut imports, 0).is_none(),
				"node {node:?} must be unsupported"
			);
		}
		assert!(dart_field(&number(NumberFormat::U8), &program, &mut imports, 9).is_none());
	}

	#[test]
	fn event_facts_reject_non_struct_events() {
		let mut event = EventNode::new("badEvent", BytesTypeNode::new());
		event.discriminators = vec![event_discriminator(
			NumberFormat::U8,
			Number::UnsignedInteger(4),
		)];
		let histories = EventClientHistoryIndex::default();
		assert!(matches!(
			event_facts(&event, &test_program(), &histories),
			Ok(None)
		));
		// The envelope reader also rejects non-struct data on its own.
		assert!(envelope_facts(&event).is_none());
	}

	#[test]
	fn envelope_facts_require_numeric_unsigned_versions() {
		let mut events = Vec::new();

		let mut event =
			envelope_event("valueChanged", NumberFormat::U8, Number::UnsignedInteger(1));
		if let TypeNode::Struct(data) = event.data.as_mut() {
			data.fields[0].r#type = Box::new(TypeNode::PublicKey(PublicKeyTypeNode::new()));
		}
		events.push(event);

		let mut event =
			envelope_event("valueChanged", NumberFormat::U8, Number::UnsignedInteger(1));
		if let TypeNode::Struct(data) = event.data.as_mut() {
			data.fields[0].default_value =
				Box::new(Some(ValueNode::String(StringValueNode::new("4"))));
		}
		events.push(event);

		let mut event =
			envelope_event("valueChanged", NumberFormat::U8, Number::UnsignedInteger(1));
		if let TypeNode::Struct(data) = event.data.as_mut() {
			data.fields[1].r#type = Box::new(TypeNode::PublicKey(PublicKeyTypeNode::new()));
		}
		events.push(event);

		let mut event =
			envelope_event("valueChanged", NumberFormat::U8, Number::UnsignedInteger(1));
		if let TypeNode::Struct(data) = event.data.as_mut() {
			data.fields[1].default_value =
				Box::new(Some(ValueNode::String(StringValueNode::new("1"))));
		}
		events.push(event);

		let mut event =
			envelope_event("valueChanged", NumberFormat::U8, Number::UnsignedInteger(1));
		if let TypeNode::Struct(data) = event.data.as_mut() {
			data.fields[1].r#type =
				Box::new(TypeNode::Number(NumberTypeNode::le(NumberFormat::F32)));
		}
		events.push(event);

		let mut event =
			envelope_event("valueChanged", NumberFormat::U8, Number::UnsignedInteger(1));
		if let TypeNode::Struct(data) = event.data.as_mut() {
			data.fields[1].default_value =
				Box::new(Some(ValueNode::Number(NumberValueNode::new(-1_i8))));
		}
		events.push(event);

		let mut event =
			envelope_event("valueChanged", NumberFormat::U8, Number::UnsignedInteger(1));
		if let TypeNode::Struct(data) = event.data.as_mut() {
			data.fields.truncate(1);
		}
		events.push(event);

		for event in &events {
			let label = format!("event `{}`", event.name.as_ref());
			assert!(envelope_facts(event).is_none(), "{label} has no envelope");
		}

		// The version field appearing before the discriminator is not an envelope.
		let mut reordered =
			envelope_event("valueChanged", NumberFormat::U8, Number::UnsignedInteger(1));
		if let TypeNode::Struct(data) = reordered.data.as_mut() {
			data.fields.swap(0, 1);
		}
		assert!(envelope_facts(&reordered).is_none());

		// Unrelated defaulted fields are skipped while scanning the envelope.
		let mut skipped =
			envelope_event("valueChanged", NumberFormat::U8, Number::UnsignedInteger(1));
		if let TypeNode::Struct(data) = skipped.data.as_mut() {
			let mut note = event_number_field("note", NumberFormat::U8, Number::UnsignedInteger(7));
			note.default_value_strategy = None;
			data.fields.insert(1, note);
		}
		assert!(envelope_facts(&skipped).is_some());

		for (format, width) in [(NumberFormat::U16, 2), (NumberFormat::U32, 4)] {
			let event = envelope_event("valueChanged", format, Number::UnsignedInteger(1));
			let envelope =
				envelope_facts(&event).unwrap_or_else(|| panic!("width {width} envelope"));
			assert_eq!(envelope.version, 1);
			assert_eq!(envelope.version_bytes, width);
		}
	}

	#[test]
	fn constant_discriminator_widths_and_rejections() {
		for (format, value, width) in [
			(NumberFormat::U8, Number::UnsignedInteger(4), 1),
			(NumberFormat::U16, Number::UnsignedInteger(0x0102), 2),
			(NumberFormat::U32, Number::UnsignedInteger(0x0102_0304), 4),
			(
				NumberFormat::U64,
				Number::UnsignedInteger(0x0102_0304_0506_0708),
				8,
			),
		] {
			let mut event =
				envelope_event("valueChanged", NumberFormat::U8, Number::UnsignedInteger(1));
			event.discriminators = vec![event_discriminator(format, value)];
			let (_, found) =
				constant_discriminator(&event).unwrap_or_else(|| panic!("width {width} must map"));
			assert_eq!(found, width);
		}

		let mut invalid = Vec::new();
		let mut event =
			envelope_event("valueChanged", NumberFormat::U8, Number::UnsignedInteger(1));
		event.discriminators = vec![DiscriminatorNode::Size(SizeDiscriminatorNode::new(4))];
		invalid.push(event);
		let mut event =
			envelope_event("valueChanged", NumberFormat::U8, Number::UnsignedInteger(1));
		event.discriminators = vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
			ConstantValueNode::new(
				NumberTypeNode::le(NumberFormat::U8),
				ValueNode::String(StringValueNode::new("4")),
			),
			0,
		))];
		invalid.push(event);
		let mut event =
			envelope_event("valueChanged", NumberFormat::U8, Number::UnsignedInteger(1));
		event.discriminators = vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
			ConstantValueNode::new(
				StringTypeNode::utf8(),
				ValueNode::Number(NumberValueNode::new(4_u8)),
			),
			0,
		))];
		invalid.push(event);
		let mut event =
			envelope_event("valueChanged", NumberFormat::U8, Number::UnsignedInteger(1));
		event.discriminators = vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
			ConstantValueNode::new(
				NumberTypeNode::le(NumberFormat::F32),
				ValueNode::Number(NumberValueNode::new(4_u8)),
			),
			0,
		))];
		invalid.push(event);
		let mut event =
			envelope_event("valueChanged", NumberFormat::U8, Number::UnsignedInteger(1));
		event.discriminators = vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
			ConstantValueNode::new(
				NumberTypeNode::le(NumberFormat::U8),
				ValueNode::Number(NumberValueNode::new(-4_i8)),
			),
			0,
		))];
		invalid.push(event);

		for event in &invalid {
			let label = format!("event `{}`", event.name.as_ref());
			assert!(
				constant_discriminator(event).is_none(),
				"{label} has no discriminator"
			);
		}
	}

	#[test]
	fn wide_versions_use_the_shared_little_endian_helpers() {
		let event = envelope_event(
			"valueChanged",
			NumberFormat::U16,
			Number::UnsignedInteger(2),
		);
		let mut histories = EventClientHistoryIndex::default();
		histories.insert_for_test(history());
		let program = test_program();
		let facts = event_facts(&event, &program, &histories)
			.unwrap_or_else(|error| panic!("facts: {error}"))
			.unwrap_or_else(|| panic!("event facts"));
		let module = event_module("EventsProgram", &facts);
		assert!(module.contains("readLittleEndian(data,"), "{module}");
		assert!(module.contains("writeLittleEndian("), "{module}");

		let support = support_module("EventsProgram", std::slice::from_ref(&facts));
		assert!(support.contains("int readLittleEndian("), "{support}");
		assert!(
			support.contains("Uint8List writeLittleEndian("),
			"{support}"
		);
	}

	#[test]
	fn emit_reports_unrenderable_events() {
		let mut root = read_idl("events_program.json");
		if let TypeNode::Struct(data) = root.program.events[0].data.as_mut() {
			data.fields[1].r#type = Box::new(TypeNode::String(StringTypeNode::utf8()));
		}
		let temporary = tempfile::tempdir().expect("temp dir");
		let generated = temporary.path().join("lib/src/generated/events_program");
		std::fs::create_dir_all(&generated).expect("generated dir");
		let error = emit_dart_event_modules(
			&generated,
			"events_program",
			&EventClientHistoryIndex::default(),
			&root,
		)
		.expect_err("unsupported fields must fail generation");
		assert!(
			error.to_string().contains("does not support yet"),
			"{error}"
		);

		let mut root = read_idl("events_program.json");
		for event in &mut root.program.events {
			event.discriminators.clear();
		}
		let error = emit_dart_event_modules(
			&generated,
			"events_program",
			&EventClientHistoryIndex::default(),
			&root,
		)
		.expect_err("events without discriminators must fail generation");
		assert!(
			error.to_string().contains("no constant discriminator"),
			"{error}"
		);
	}

	#[test]
	fn emit_reports_io_failures() {
		let root = read_idl("events_program.json");
		let histories = EventClientHistoryIndex::default();

		let temporary = tempfile::tempdir().expect("temp dir");
		let generated = temporary.path().join("lib/src/generated/events_program");
		std::fs::create_dir_all(&generated).expect("generated dir");
		std::fs::write(generated.join("events"), b"file").expect("blocked events path");
		assert!(
			emit_dart_event_modules(&generated, "events_program", &histories, &root).is_err(),
			"a blocked events directory must fail"
		);

		let temporary = tempfile::tempdir().expect("temp dir");
		let generated = temporary.path().join("lib/src/generated/events_program");
		std::fs::create_dir_all(generated.join("events/event_log.dart")).expect("blocked support");
		assert!(
			emit_dart_event_modules(&generated, "events_program", &histories, &root).is_err(),
			"a blocked support module must fail"
		);

		let temporary = tempfile::tempdir().expect("temp dir");
		let generated = temporary.path().join("lib/src/generated/events_program");
		std::fs::create_dir_all(generated.join("events/my_event.dart")).expect("blocked event");
		assert!(
			emit_dart_event_modules(&generated, "events_program", &histories, &root).is_err(),
			"a blocked event module must fail"
		);

		let temporary = tempfile::tempdir().expect("temp dir");
		let generated = temporary.path().join("lib/src/generated/events_program");
		std::fs::create_dir_all(generated.join("events/events.dart")).expect("blocked barrel");
		assert!(
			emit_dart_event_modules(&generated, "events_program", &histories, &root).is_err(),
			"a blocked barrel must fail"
		);
	}

	#[test]
	fn register_program_barrel_handles_missing_and_unreadable_paths() {
		// Root paths have no file name to anchor the barrel.
		register_program_barrel(Path::new("/"))
			.unwrap_or_else(|error| panic!("root path: {error}"));

		// A generated directory without a renderer barrel is left alone.
		let temporary = tempfile::tempdir().expect("temp dir");
		let generated = temporary.path().join("lib/src/generated/events_program");
		std::fs::create_dir_all(&generated).expect("generated dir");
		register_program_barrel(&generated).unwrap_or_else(|error| panic!("no barrel: {error}"));

		// A directory where the barrel should be fails the read.
		std::fs::create_dir_all(generated.join("events_program.dart")).expect("blocked barrel");
		assert!(register_program_barrel(&generated).is_err());
	}

	#[cfg(unix)]
	#[test]
	fn register_program_barrel_reports_unwritable_barrels() {
		use std::os::unix::fs::PermissionsExt;

		let temporary = tempfile::tempdir().expect("temp dir");
		let generated = temporary.path().join("lib/src/generated/events_program");
		std::fs::create_dir_all(&generated).expect("generated dir");
		let barrel = generated.join("events_program.dart");
		std::fs::write(&barrel, "export 'accounts/accounts.dart';\n").expect("barrel");
		std::fs::set_permissions(&barrel, std::fs::Permissions::from_mode(0o444))
			.expect("read-only barrel");

		let result = register_program_barrel(&generated);
		std::fs::set_permissions(&barrel, std::fs::Permissions::from_mode(0o644))
			.expect("restore barrel");
		assert!(result.is_err(), "a read-only barrel must fail");
	}

	#[test]
	fn name_helpers_handle_empty_names() {
		assert_eq!(lower_camel(""), "");
		assert_eq!(snake_to_camel(""), "");
	}

	#[test]
	fn emits_projection_for_migration_aware_events() {
		let root = read_idl("migrations_program.json");
		let mut histories = EventClientHistoryIndex::default();
		histories.insert_for_test(history());
		let program = pascal_case(&snake_to_camel("migrations_program"));
		let facts = event_facts(&root.program.events[0], &root.program, &histories)
			.unwrap_or_else(|error| panic!("facts: {error}"))
			.unwrap_or_else(|| panic!("event facts"));
		let module = event_module(&program, &facts);

		for expected in [
			"class ValueChangedEventEvent {",
			"final int discriminator;",
			"final int migrationVersion;",
			"final BigInt value;",
			"final int memo;",
			"const valueChangedEventEventDiscriminator = 4;",
			"const valueChangedEventEventMigrationVersion = 1;",
			"const valueChangedEventEventSize = 12;",
			"class NormalizedValueChangedEventEvent extends MigrationsProgramEvent {",
			"final int sourceVersion;",
			"final bool wasMigrated;",
			"(0, 1, true, 8, 10, [(0, 0, 8)]),",
			"upgrade this client",
			"import 'dart:typed_data';",
			"var cursor = 0;",
			"normalizeValueChangedEventEvent",
			"parseValueChangedEventEventFromLog",
		] {
			assert!(
				module.contains(expected),
				"missing `{expected}` in:\n{module}"
			);
		}
	}

	#[test]
	fn emits_current_only_decoders_for_events_without_history() {
		let root = read_idl("events_program.json");
		let program = pascal_case(&snake_to_camel("events_program"));
		let histories = EventClientHistoryIndex::default();
		let facts = event_facts(&root.program.events[0], &root.program, &histories)
			.unwrap_or_else(|error| panic!("facts: {error}"))
			.unwrap_or_else(|| panic!("event facts"));
		let module = event_module(&program, &facts);

		// The program envelopes events, so the emitted class is the enveloped
		// current shape: it carries the discriminator and migration version as
		// fields, and with no recorded history there are no projection steps.
		assert!(module.contains("class MyEventEvent {"));
		assert!(module.contains("required this.discriminator,"));
		assert!(module.contains("required this.migrationVersion,"));
		assert!(module.contains("Uint8List label;"));
		assert!(module.contains("const myEventEventMigrationVersion = 0;"));
		assert!(!module.contains("ProjectionSteps"));
	}

	#[test]
	fn rejects_events_without_a_constant_discriminator() {
		let mut root = read_idl("events_program.json");
		for event in &mut root.program.events {
			event.discriminators.clear();
		}
		let histories = EventClientHistoryIndex::default();
		let facts = event_facts(&root.program.events[0], &root.program, &histories);
		assert!(
			matches!(facts, Ok(None)),
			"an event without a discriminator has no log entry point",
		);
	}

	#[test]
	fn barrel_dispatches_every_event() {
		let root = read_idl("events_program.json");
		let program = pascal_case(&snake_to_camel("events_program"));
		let histories = EventClientHistoryIndex::default();
		let events = root
			.program
			.events
			.iter()
			.map(|event| {
				event_facts(event, &root.program, &histories)
					.unwrap_or_else(|error| panic!("facts: {error}"))
					.unwrap_or_else(|| panic!("event facts"))
			})
			.collect::<Vec<_>>();
		let exports = vec![
			"event_log.dart".to_owned(),
			"my_event.dart".to_owned(),
			"my_other_event.dart".to_owned(),
		];
		let barrel = events_barrel(&program, &events, &exports);

		assert!(barrel.contains(
			"List<EventsProgramEvent> parseEventsProgramEventsFromLogs(List<String> logs) {"
		));
		assert!(barrel.contains("final myEvent = parseMyEventEventFromLog(log);"));
		assert!(barrel.contains("final myOtherEvent = parseMyOtherEventEventFromLog(log);"));
		assert!(barrel.contains("export 'my_event.dart';"));
	}

	#[test]
	fn support_module_imports_are_minimal() {
		let module = support_module("MigrationsProgram", &[]);
		assert!(module.contains("abstract class MigrationsProgramEvent"));
		assert!(module.contains("decodeProgramDataLog"));
		assert!(!module.contains("readLittleEndian"));
	}

	#[test]
	fn unsupported_field_types_report_the_field() {
		let mut root = read_idl("events_program.json");
		if let TypeNode::Struct(data) = root.program.events[0].data.as_mut() {
			// Replace the first payload field with a link the emitter cannot map.
			data.fields[1].r#type = Box::new(TypeNode::Link(DefinedTypeLinkNode::new("Missing")));
		}
		let program = root.program.clone();
		let event = root.program.events.remove(0);
		let histories = EventClientHistoryIndex::default();
		let facts = event_facts(&event, &program, &histories);
		let message = facts
			.err()
			.unwrap_or_else(|| panic!("unsupported field must fail"));
		assert!(message.contains("does not support yet"), "{message}");
	}

	#[test]
	fn program_barrel_registration_is_idempotent() {
		let directory =
			std::env::temp_dir().join(format!("pina-dart-events-{}", std::process::id(),));
		let generated = directory.join("lib/src/generated/demo_program");
		std::fs::create_dir_all(&generated).unwrap_or_else(|error| panic!("mkdir: {error}"));
		let barrel = generated.join("demo_program.dart");
		std::fs::write(&barrel, "export 'accounts/accounts.dart';\n")
			.unwrap_or_else(|error| panic!("write: {error}"));

		register_program_barrel(&generated).unwrap_or_else(|error| panic!("register: {error}"));
		register_program_barrel(&generated)
			.unwrap_or_else(|error| panic!("register twice: {error}"));
		let source = std::fs::read_to_string(&barrel).unwrap_or_else(|error| panic!("{error}"));
		assert_eq!(source.matches("events/events.dart").count(), 1);
		let _ = std::fs::remove_dir_all(&directory);
	}

	#[test]
	fn writes_event_modules_and_registers_the_barrel() {
		let root = read_idl("migrations_program.json");
		let mut histories = EventClientHistoryIndex::default();
		histories.insert_for_test(history());
		let temporary = tempfile::tempdir().expect("temp dir");
		let generated = temporary
			.path()
			.join("lib/src/generated/migrations_program");
		std::fs::create_dir_all(&generated).expect("generated dir");
		let barrel = generated.join("migrations_program.dart");
		std::fs::write(&barrel, "export 'accounts/accounts.dart';\n").expect("barrel");

		let needs_helper =
			emit_dart_event_modules(&generated, "migrations_program", &histories, &root)
				.expect("emit modules");
		assert!(!needs_helper, "the event module does not use bool fields");

		let support = std::fs::read_to_string(generated.join("events/event_log.dart"))
			.expect("support module");
		assert!(support.contains("abstract class MigrationsProgramEvent"));
		let event = std::fs::read_to_string(generated.join("events/value_changed_event.dart"))
			.expect("event module");
		assert!(
			event.contains("class NormalizedValueChangedEventEvent extends MigrationsProgramEvent")
		);
		let barrel_source = std::fs::read_to_string(&barrel).expect("barrel");
		assert!(barrel_source.contains("export 'events/events.dart';"));

		// Re-running generation must not duplicate the barrel export.
		emit_dart_event_modules(&generated, "migrations_program", &histories, &root)
			.expect("emit modules twice");
		let barrel_source = std::fs::read_to_string(&barrel).expect("barrel");
		assert_eq!(barrel_source.matches("events/events.dart").count(), 1);
	}

	#[test]
	fn programs_without_events_write_nothing() {
		let root = read_idl("counter_program.json");
		let temporary = tempfile::tempdir().expect("temp dir");
		let generated = temporary.path().join("lib/src/generated/counter_program");
		std::fs::create_dir_all(&generated).expect("generated dir");

		let needs_helper = emit_dart_event_modules(
			&generated,
			"counter_program",
			&EventClientHistoryIndex::default(),
			&root,
		)
		.expect("no-op emit");
		assert!(!needs_helper);
		assert!(!generated.join("events/event_log.dart").exists());
	}

	#[test]
	fn boolean_event_fields_request_the_shared_helper() {
		let mut root = read_idl("events_program.json");
		if let TypeNode::Struct(data) = root.program.events[0].data.as_mut() {
			data.fields[1].r#type = Box::new(TypeNode::Boolean(BooleanTypeNode::new(
				NumberTypeNode::le(NumberFormat::U8),
			)));
		}
		let program = root.program.clone();
		let event = root.program.events.remove(0);
		let histories = EventClientHistoryIndex::default();
		let facts = event_facts(&event, &program, &histories)
			.unwrap_or_else(|error| panic!("facts: {error}"))
			.unwrap_or_else(|| panic!("event facts"));
		let module = event_module("EventsProgram", &facts);
		assert!(module.contains("getPinaPodBooleanDecoder"));
		assert!(module.contains("import '../pina_pod_codecs.dart';"));
	}

	#[test]
	fn path_helpers_convert_names() {
		assert_eq!(snake_to_camel("migrations_program"), "migrationsProgram");
		assert_eq!(lower_camel("ValueChangedEvent"), "valueChangedEvent");
	}
}
