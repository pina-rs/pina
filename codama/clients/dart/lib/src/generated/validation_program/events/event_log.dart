// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:convert';
import 'dart:typed_data';

/// One event decoded from this program's transaction logs.
///
/// Every generated event class implements this interface so one log parser can
/// return a list of mixed events.
abstract class ValidationProgramEvent {
  /// Const constructor for generated subclasses.
  const ValidationProgramEvent();

  /// The IDL event name this record was decoded as.
  String get name;
}

/// Decode the base64 payload of one `Program data:` log line.
///
/// Returns `null` when the line is not a program-data record. Malformed base64
/// throws a [FormatException] naming the log line instead of silently dropping
/// the event.
Uint8List? decodeProgramDataLog(String log) {
  const prefix = 'Program data: ';
  if (!log.startsWith(prefix)) {
    return null;
  }
  try {
    return base64Decode(log.substring(prefix.length));
  } on FormatException catch (error) {
    throw FormatException(
      'invalid base64 in `Program data:` log line: ${error.message}',
    );
  }
}
