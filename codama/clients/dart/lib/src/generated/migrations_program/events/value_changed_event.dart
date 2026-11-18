// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';

import 'event_log.dart';

/// Event record `ValueChangedEvent`.
class ValueChangedEventEvent extends MigrationsProgramEvent {
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

  @override
  String get name => 'valueChangedEvent';

  String toString() =>
      'ValueChangedEventEvent(discriminator: ${discriminator}, migrationVersion: ${migrationVersion}, value: ${value}, memo: ${memo})';
}

/// The discriminator this event is emitted under.
const valueChangedEventEventDiscriminator = 4;

/// The discriminator bytes as stored at offset zero.
const List<int> _valueChangedEventEventDiscriminatorBytes = [4];

/// The migration version this event decodes.
const valueChangedEventEventMigrationVersion = 1;

/// Exact current byte length of a `ValueChangedEvent` record, envelope included.
const valueChangedEventEventSize = 12;

/// Decode one `ValueChangedEvent` record.
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
          ? 'event migration version mismatch: expected 1, received $v1 (decode it with the event for that version)'
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

/// A decoded `ValueChangedEvent` log record.
typedef DecodedValueChangedEventEvent = ValueChangedEventEvent;

/// Decode a `Program data:` log line, or return null when the line is not
/// this event.
ValueChangedEventEvent? parseValueChangedEventEventFromLog(String log) {
  final bytes = decodeProgramDataLog(log);
  if (bytes == null || bytes.length < 2) {
    return null;
  }
  for (var index = 0; index < 1; index++) {
    if (bytes[index] != _valueChangedEventEventDiscriminatorBytes[index]) {
      return null;
    }
  }
  if (bytes[1] != valueChangedEventEventMigrationVersion) {
    return null;
  }
  return decodeValueChangedEventEvent(bytes);
}
