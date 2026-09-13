// Auto-generated. Do not edit.
// ignore_for_file: type=lint

export 'event_log.dart';
export 'my_event.dart';
export 'my_other_event.dart';

import 'event_log.dart';
import 'my_event.dart';
import 'my_other_event.dart';

/// Decode every `Program data:` line that names one of this program's events.
///
/// Unrelated lines and programs are skipped. A log that names an event but
/// carries an unknown, future, or non-projectable version throws instead of
/// being silently dropped.
List<EventsProgramEvent> parseEventsProgramEventsFromLogs(List<String> logs) {
  final discovered = <EventsProgramEvent>[];
  for (final log in logs) {
    final myEvent = parseMyEventEventFromLog(log);
    if (myEvent != null) {
      discovered.add(myEvent);
      continue;
    }
    final myOtherEvent = parseMyOtherEventEventFromLog(log);
    if (myOtherEvent != null) {
      discovered.add(myOtherEvent);
      continue;
    }
  }
  return discovered;
}
