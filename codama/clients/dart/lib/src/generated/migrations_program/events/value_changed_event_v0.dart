// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';

import 'event_log.dart';

/// Event record `ValueChangedEventV0`.
class ValueChangedEventV0Event extends MigrationsProgramEvent {
  const ValueChangedEventV0Event({
    required this.discriminator,
    required this.migrationVersion,
    required this.value,
  });

  final int discriminator;
  final int migrationVersion;
  final BigInt value;

  @override
  String get name => 'valueChangedEventV0';

  String toString() =>
      'ValueChangedEventV0Event(discriminator: ${discriminator}, migrationVersion: ${migrationVersion}, value: ${value})';
}

/// The discriminator this event is emitted under.
const valueChangedEventV0EventDiscriminator = 4;

/// The discriminator bytes as stored at offset zero.
const List<int> _valueChangedEventV0EventDiscriminatorBytes = [4];

/// The migration version this event decodes.
const valueChangedEventV0EventMigrationVersion = 0;

/// Exact current byte length of a `ValueChangedEventV0` record, envelope included.
const valueChangedEventV0EventSize = 10;

/// Decode one `ValueChangedEventV0` record.
ValueChangedEventV0Event decodeValueChangedEventV0Event(Uint8List data) {
  if (data.length != valueChangedEventV0EventSize) {
    throw RangeError(
      'expected exactly ${valueChangedEventV0EventSize} bytes, received ${data.length}',
    );
  }
  var cursor = 0;
  final (v0, c0) = getU8Decoder().read(data, cursor);
  cursor = c0;
  if (v0 != 4) {
    throw RangeError(
      'the provided bytes do not match the "ValueChangedEventV0" event discriminator',
    );
  }
  final (v1, c1) = getU8Decoder().read(data, cursor);
  cursor = c1;
  if (v1 != 0) {
    throw RangeError(
      v1 < 0
          ? 'event migration version mismatch: expected 0, received $v1 (decode it with the event for that version)'
          : 'event migration version mismatch: expected 0, received $v1 (the log was written by a newer program; upgrade this client)',
    );
  }
  final (v2, c2) = getU64Decoder().read(data, cursor);
  cursor = c2;

  return ValueChangedEventV0Event(
    discriminator: v0,
    migrationVersion: v1,
    value: v2,
  );
}

/// A decoded `ValueChangedEventV0` log record.
typedef DecodedValueChangedEventV0Event = ValueChangedEventV0Event;

/// Decode a `Program data:` log line, or return null when the line is not
/// this event.
ValueChangedEventV0Event? parseValueChangedEventV0EventFromLog(String log) {
  final bytes = decodeProgramDataLog(log);
  if (bytes == null || bytes.length < 2) {
    return null;
  }
  for (var index = 0; index < 1; index++) {
    if (bytes[index] != _valueChangedEventV0EventDiscriminatorBytes[index]) {
      return null;
    }
  }
  if (bytes[1] != valueChangedEventV0EventMigrationVersion) {
    return null;
  }
  return decodeValueChangedEventV0Event(bytes);
}
