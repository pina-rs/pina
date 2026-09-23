// Auto-generated. Do not edit.
// ignore_for_file: type=lint

export 'event_log.dart';
export 'value_changed_event.dart';

import 'event_log.dart';
import 'value_changed_event.dart';

/// Decode every `Program data:` line that names one of this program's events.
///
/// Unrelated lines and programs are skipped. A log that names an event but
/// carries an unknown, future, or non-projectable version throws instead of
/// being silently dropped.
List<MigrationsProgramEvent> parseMigrationsProgramEventsFromLogs(List<String> logs) {
  final discovered = <MigrationsProgramEvent>[];
  for (final log in logs) {
    final valueChangedEvent = parseValueChangedEventEventFromLog(log);
    if (valueChangedEvent != null) {
      discovered.add(valueChangedEvent);
      continue;
    }
  }
  return discovered;
}
