// Auto-generated. Do not edit.
// ignore_for_file: type=lint

export 'event_log.dart';
export 'policy_checked.dart';

import 'event_log.dart';
import 'policy_checked.dart';

/// Decode every `Program data:` line that names one of this program's events.
///
/// Unrelated lines and programs are skipped. A log that names an event but
/// carries an unknown, future, or non-projectable version throws instead of
/// being silently dropped.
List<ValidationProgramEvent> parseValidationProgramEventsFromLogs(List<String> logs) {
  final discovered = <ValidationProgramEvent>[];
  for (final log in logs) {
    final policyChecked = parsePolicyCheckedEventFromLog(log);
    if (policyChecked != null) {
      discovered.add(policyChecked);
      continue;
    }
  }
  return discovered;
}
