// Auto-generated. Do not edit.
// ignore_for_file: type=lint

export 'event_log.dart';
export 'my_event.dart';
export 'my_other_event.dart';

import 'event_log.dart';
import 'my_event.dart';
import 'my_other_event.dart';

/// The program whose invocation frames emit the events decoded here.
const eventsProgramEventSourceAddress =
    '2dhGsWUzy5YKUsjZdLHLmkNpUDAXkNa9MYWsPc4Ziqzy';

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
List<EventsProgramEvent> parseEventsProgramEventsFromLogs(
  List<String> logs, {
  String programAddress = eventsProgramEventSourceAddress,
}) {
  final discovered = <EventsProgramEvent>[];
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
