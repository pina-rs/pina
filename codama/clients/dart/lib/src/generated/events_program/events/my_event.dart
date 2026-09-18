// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';

import 'event_log.dart';

/// Event record `MyEvent`.
class MyEventEvent {
  const MyEventEvent({
    required this.discriminator,
    required this.migrationVersion,
    required this.data,
    required this.label,
  });

  final int discriminator;
  final int migrationVersion;
  final BigInt data;
  final Uint8List label;

  String get name => 'myEvent';

  String toString() =>
      'MyEventEvent(discriminator: ${discriminator}, migrationVersion: ${migrationVersion}, data: ${data}, label: ${label})';
}

/// The discriminator this event is emitted under.
const myEventEventDiscriminator = 1;

/// The discriminator bytes as stored at offset zero.
const List<int> _myEventEventDiscriminatorBytes = [1];

/// The version this client was generated from.
const myEventEventMigrationVersion = 0;

/// Exact current byte length of a `MyEvent` record, envelope included.
const myEventEventSize = 18;

/// Decode one current-version `MyEvent` record.
MyEventEvent decodeMyEventEvent(Uint8List data) {
  if (data.length != myEventEventSize) {
    throw RangeError(
      'expected exactly ${myEventEventSize} bytes, received ${data.length}',
    );
  }
  var cursor = 0;
  final (v0, c0) = getU8Decoder().read(data, cursor);
  cursor = c0;
  if (v0 != 1) {
    throw RangeError(
      'the provided bytes do not match the "MyEvent" event discriminator',
    );
  }
  final (v1, c1) = getU8Decoder().read(data, cursor);
  cursor = c1;
  if (v1 != 0) {
    throw RangeError(
      v1 < 0
          ? 'event migration version mismatch: expected 0, received $v1 (the log predates this client; project it through the checked-in event history or decode it with a client generated from the schema that wrote it)'
          : 'event migration version mismatch: expected 0, received $v1 (the log was written by a newer program; upgrade this client)',
    );
  }
  final (v2, c2) = getU64Decoder().read(data, cursor);
  cursor = c2;
  final (v3, c3) = fixDecoderSize(getBytesDecoder(), 8).read(data, cursor);
  cursor = c3;

  return MyEventEvent(
    discriminator: v0,
    migrationVersion: v1,
    data: v2,
    label: v3,
  );
}

/// Event bytes projected into the current shape.
class NormalizedMyEventEvent extends EventsProgramEvent {
  const NormalizedMyEventEvent({
    required this.data,
    required this.sourceVersion,
    required this.wasMigrated,
  });

  final MyEventEvent data;

  /// The version carried by the immutable log record, matching the runtime's
  /// `CurrentEventData::source_version`.
  final int sourceVersion;

  /// Whether a historical projection ran.
  final bool wasMigrated;

  @override
  String get name => 'myEvent';
}

/// Adjacent projections from the checked-in migration manifest: `(from, to,
/// automatic, source payload size, destination payload size, moves)`.
const List<(int, int, bool, int, int, List<(int, int, int)>)>
_myEventProjectionSteps = [];

/// Project current or historical bytes into the current shape, mirroring the
/// runtime's `normalize_event_data`. Unknown, future, non-exact, and manual
/// transitions fail closed.
NormalizedMyEventEvent normalizeMyEventEvent(Uint8List data) {
  if (data.length < 2) {
    throw RangeError(
      'the provided data is too short for the "MyEvent" event envelope',
    );
  }
  for (var index = 0; index < 1; index++) {
    if (data[index] != _myEventEventDiscriminatorBytes[index]) {
      throw RangeError(
        'the provided data does not match the "MyEvent" event discriminator',
      );
    }
  }
  final sourceVersion = data[1];
  if (sourceVersion > 0) {
    throw RangeError(
      'event migration version mismatch: expected 0, received $sourceVersion (the log was written by a newer program; upgrade this client)',
    );
  }
  if (sourceVersion == 0) {
    return NormalizedMyEventEvent(
      data: decodeMyEventEvent(data),
      sourceVersion: sourceVersion,
      wasMigrated: false,
    );
  }
  final projected = _projectMyEventEvent(data, sourceVersion);
  return NormalizedMyEventEvent(
    data: decodeMyEventEvent(projected),
    sourceVersion: sourceVersion,
    wasMigrated: true,
  );
}

Uint8List _projectMyEventEvent(Uint8List data, int sourceVersion) {
  var version = sourceVersion;
  var payload = Uint8List.fromList(data.sublist(2));
  while (version != 0) {
    (int, int, bool, int, int, List<(int, int, int)>)? step;
    for (final candidate in _myEventProjectionSteps) {
      if (candidate.$1 == version) {
        step = candidate;
        break;
      }
    }
    if (step == null) {
      throw RangeError(
        'event migration version mismatch: expected 0, received $version (this client has no checked-in projection for it)',
      );
    }
    if (!step.$3) {
      throw RangeError(
        'event migration version mismatch: expected 0, received ${step.$1} (the v${step.$1} to v${step.$2} transition is manual, so only an on-chain projection or a client generated from that schema can represent it)',
      );
    }
    if (payload.length != step.$4) {
      throw RangeError(
        'event migration version mismatch: expected 0, received $version (the log length does not match the v$version schema)',
      );
    }
    final destination = Uint8List(step.$5);
    for (final (sourceOffset, destinationOffset, size) in step.$6) {
      destination.setRange(
        destinationOffset,
        destinationOffset + size,
        payload,
        sourceOffset,
      );
    }
    payload = destination;
    version = step.$2;
  }

  final projected = Uint8List(2 + payload.length);
  projected.setRange(0, 1, _myEventEventDiscriminatorBytes);
  projected[1] = 0;
  projected.setRange(2, projected.length, payload);
  return projected;
}

/// A decoded `MyEvent` log record.
typedef DecodedMyEventEvent = NormalizedMyEventEvent;

/// Decode a `Program data:` log line, or return null when the line is not
/// this event.
NormalizedMyEventEvent? parseMyEventEventFromLog(String log) {
  final bytes = decodeProgramDataLog(log);
  if (bytes == null || bytes.length < 1) {
    return null;
  }
  for (var index = 0; index < 1; index++) {
    if (bytes[index] != _myEventEventDiscriminatorBytes[index]) {
      return null;
    }
  }
  return normalizeMyEventEvent(bytes);
}
