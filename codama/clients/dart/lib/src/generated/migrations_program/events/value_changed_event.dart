// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';

import 'event_log.dart';

/// Event record `ValueChangedEvent`.
class ValueChangedEventEvent {
  const ValueChangedEventEvent({
    required this.discriminator,
    required this.migrationVersion,
    required this.value,
    required this.memo,
  });

  final int discriminator;
  final int migrationVersion;
  final BigInt value;
  final int memo;

  String get name => 'valueChangedEvent';

  String toString() =>
      'ValueChangedEventEvent(discriminator: ${discriminator}, migrationVersion: ${migrationVersion}, value: ${value}, memo: ${memo})';
}

/// The discriminator this event is emitted under.
const valueChangedEventEventDiscriminator = 4;

/// The discriminator bytes as stored at offset zero.
const List<int> _valueChangedEventEventDiscriminatorBytes = [4];

/// The version this client was generated from.
const valueChangedEventEventMigrationVersion = 1;

/// Exact current byte length of a `ValueChangedEvent` record, envelope included.
const valueChangedEventEventSize = 12;

/// Decode one current-version `ValueChangedEvent` record.
ValueChangedEventEvent decodeValueChangedEventEvent(Uint8List data) {
  if (data.length != valueChangedEventEventSize) {
    throw RangeError(
      'expected exactly ${valueChangedEventEventSize} bytes, received ${data.length}',
    );
  }
  var cursor = 0;
  final (v0, c0) = getU8Decoder().read(data, cursor);
  cursor = c0;
  if (v0 != 4) {
    throw RangeError(
      'the provided bytes do not match the "ValueChangedEvent" event discriminator',
    );
  }
  final (v1, c1) = getU8Decoder().read(data, cursor);
  cursor = c1;
  if (v1 != 1) {
    throw RangeError(
      v1 < 1
          ? 'event migration version mismatch: expected 1, received $v1 (the log predates this client; project it through the checked-in event history or decode it with a client generated from the schema that wrote it)'
          : 'event migration version mismatch: expected 1, received $v1 (the log was written by a newer program; upgrade this client)',
    );
  }
  final (v2, c2) = getU64Decoder().read(data, cursor);
  cursor = c2;
  final (v3, c3) = getU16Decoder().read(data, cursor);
  cursor = c3;

  return ValueChangedEventEvent(
    discriminator: v0,
    migrationVersion: v1,
    value: v2,
    memo: v3,
  );
}

/// Event bytes projected into the current shape.
class NormalizedValueChangedEventEvent extends MigrationsProgramEvent {
  const NormalizedValueChangedEventEvent({
    required this.data,
    required this.sourceVersion,
    required this.wasMigrated,
  });

  final ValueChangedEventEvent data;

  /// The version carried by the immutable log record, matching the runtime's
  /// `CurrentEventData::source_version`.
  final int sourceVersion;

  /// Whether a historical projection ran.
  final bool wasMigrated;

  @override
  String get name => 'valueChangedEvent';
}

/// Adjacent projections from the checked-in migration manifest: `(from, to,
/// automatic, source payload size, destination payload size, moves)`.
const List<(int, int, bool, int, int, List<(int, int, int)>)>
_valueChangedEventProjectionSteps = [
  (0, 1, true, 8, 10, [(0, 0, 8)]),
];

/// Project current or historical bytes into the current shape, mirroring the
/// runtime's `normalize_event_data`. Unknown, future, non-exact, and manual
/// transitions fail closed.
NormalizedValueChangedEventEvent normalizeValueChangedEventEvent(
  Uint8List data,
) {
  if (data.length < 2) {
    throw RangeError(
      'the provided data is too short for the "ValueChangedEvent" event envelope',
    );
  }
  for (var index = 0; index < 1; index++) {
    if (data[index] != _valueChangedEventEventDiscriminatorBytes[index]) {
      throw RangeError(
        'the provided data does not match the "ValueChangedEvent" event discriminator',
      );
    }
  }
  final sourceVersion = data[1];
  if (sourceVersion > 1) {
    throw RangeError(
      'event migration version mismatch: expected 1, received $sourceVersion (the log was written by a newer program; upgrade this client)',
    );
  }
  if (sourceVersion == 1) {
    return NormalizedValueChangedEventEvent(
      data: decodeValueChangedEventEvent(data),
      sourceVersion: sourceVersion,
      wasMigrated: false,
    );
  }
  final projected = _projectValueChangedEventEvent(data, sourceVersion);
  return NormalizedValueChangedEventEvent(
    data: decodeValueChangedEventEvent(projected),
    sourceVersion: sourceVersion,
    wasMigrated: true,
  );
}

Uint8List _projectValueChangedEventEvent(Uint8List data, int sourceVersion) {
  var version = sourceVersion;
  var payload = Uint8List.fromList(data.sublist(2));
  while (version != 1) {
    (int, int, bool, int, int, List<(int, int, int)>)? step;
    for (final candidate in _valueChangedEventProjectionSteps) {
      if (candidate.$1 == version) {
        step = candidate;
        break;
      }
    }
    if (step == null) {
      throw RangeError(
        'event migration version mismatch: expected 1, received $version (this client has no checked-in projection for it)',
      );
    }
    if (!step.$3) {
      throw RangeError(
        'event migration version mismatch: expected 1, received ${step.$1} (the v${step.$1} to v${step.$2} transition is manual, so only an on-chain projection or a client generated from that schema can represent it)',
      );
    }
    if (payload.length != step.$4) {
      throw RangeError(
        'event migration version mismatch: expected 1, received $version (the log length does not match the v$version schema)',
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
  projected.setRange(0, 1, _valueChangedEventEventDiscriminatorBytes);
  projected[1] = 1;
  projected.setRange(2, projected.length, payload);
  return projected;
}

/// A decoded `ValueChangedEvent` log record.
typedef DecodedValueChangedEventEvent = NormalizedValueChangedEventEvent;

/// Decode a `Program data:` log line, or return null when the line is not
/// this event.
NormalizedValueChangedEventEvent? parseValueChangedEventEventFromLog(
  String log,
) {
  final bytes = decodeProgramDataLog(log);
  if (bytes == null || bytes.length < 1) {
    return null;
  }
  for (var index = 0; index < 1; index++) {
    if (bytes[index] != _valueChangedEventEventDiscriminatorBytes[index]) {
      return null;
    }
  }
  return normalizeValueChangedEventEvent(bytes);
}
