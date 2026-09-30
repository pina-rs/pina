// Auto-generated. Do not edit.
// ignore_for_file: type=lint

export 'event_log.dart';
export 'policy_checked.dart';

import 'event_log.dart';
import 'policy_checked.dart';

/// The program whose invocation frames emit the events decoded here.
const validationProgramEventSourceAddress =
    'GKYaKKaAJvuzkH2GKkaEFAqESh9NEobZ3V2Ub7qbpVYn';

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
/// but carries a version no generated event describes throws instead of being
/// silently dropped. The per-event `parse*FromLog` helpers decode one line
/// without this attribution and are only safe for data already known to come
/// from this program.
List<ValidationProgramEvent> parseValidationProgramEventsFromLogs(
  List<String> logs, {
  String programAddress = validationProgramEventSourceAddress,
}) {
  final discovered = <ValidationProgramEvent>[];
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
    final policyChecked = parsePolicyCheckedEventFromLog(log);
    if (policyChecked != null) {
      discovered.add(policyChecked);
      continue;
    }
    final unknownVersion = _unrecognizedEventVersion(log);
    if (unknownVersion != null) {
      throw RangeError(unknownVersion);
    }
  }
  return discovered;
}

/// Explain a `Program data:` line that names a migration-aware event but that
/// no generated event claimed, or return null for an unrelated line.
String? _unrecognizedEventVersion(String log) {
  final bytes = decodeProgramDataLog(log);
  if (bytes == null) {
    return null;
  }
  if (bytes.length >= 1 && bytes[0] == 1) {
    return bytes.length < 2
        ? 'event "policyChecked" log is too short for its version envelope'
        : 'event "policyChecked" log carries migration version ${bytes[1]}, which this client cannot decode; regenerate it';
  }
  return null;
}
