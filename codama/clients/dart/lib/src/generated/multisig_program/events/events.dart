// Auto-generated. Do not edit.
// ignore_for_file: type=lint

export 'event_log.dart';
export 'proposal_status_event.dart';

import 'event_log.dart';
import 'proposal_status_event.dart';

/// Decode every `Program data:` line that names one of this program's events.
///
/// Unrelated lines and programs are skipped. A log that names an event but
/// carries an unknown, future, or non-projectable version throws instead of
/// being silently dropped.
List<MultisigProgramEvent> parseMultisigProgramEventsFromLogs(
  List<String> logs,
) {
  final discovered = <MultisigProgramEvent>[];
  for (final log in logs) {
    final proposalStatusEvent = parseProposalStatusEventEventFromLog(log);
    if (proposalStatusEvent != null) {
      discovered.add(proposalStatusEvent);
      continue;
    }
  }
  return discovered;
}
