// Auto-generated. Do not edit.
// ignore_for_file: type=lint

export 'event_log.dart';
export 'value_changed_event.dart';

import 'event_log.dart';
import 'value_changed_event.dart';

/// The program whose invocation frames emit the events decoded here.
const migrationsProgramEventSourceAddress =
    'GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS';

final _programInvokeLog = RegExp(r'^Program (\S+) invoke \[\d+\]$');
final _programExitLog = RegExp(r'^Program (\S+) (?:success|failed: .*)$');

/// Decode every `Program data:` line this program emitted in a transaction's
/// logs.
///
/// [logs] must be the complete, ordered log messages of one transaction. The
/// parser follows the runtime's `Program <address> invoke [n]` and
/// `Program <address> success` / `failed` frames and decodes a data line only
/// while [programAddress] is the innermost invoked program. Any program can
/// write a `Program data:` line with this program's discriminator, so data
/// lines from other programs (including ones this program invokes through CPI)
/// and lines outside any frame are skipped rather than trusted.
///
/// Unrelated lines are skipped. A line this program emitted that names an event
/// but carries an unknown, future, or non-projectable version throws instead of
/// being silently dropped. The per-event `parse*FromLog` helpers decode one line
/// without this attribution and are only safe for data already known to come
/// from this program.
List<MigrationsProgramEvent> parseMigrationsProgramEventsFromLogs(
  List<String> logs, {
  String programAddress = migrationsProgramEventSourceAddress,
}) {
  final discovered = <MigrationsProgramEvent>[];
  final frames = <String>[];
  for (final log in logs) {
    final invoke = _programInvokeLog.firstMatch(log);
    if (invoke != null) {
      frames.add(invoke.group(1)!);
      continue;
    }
    if (_programExitLog.hasMatch(log)) {
      if (frames.isNotEmpty) {
        frames.removeLast();
      }
      continue;
    }
    if (frames.isEmpty || frames.last != programAddress) {
      continue;
    }
    final valueChangedEvent = parseValueChangedEventEventFromLog(log);
    if (valueChangedEvent != null) {
      discovered.add(valueChangedEvent);
      continue;
    }
  }
  return discovered;
}
