// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';

import 'event_log.dart';

/// Event record `MyOtherEvent`.
class MyOtherEventEvent extends EventsProgramEvent {
  const MyOtherEventEvent({
    required this.discriminator,
    required this.migrationVersion,
    required this.data,
    required this.label,
  });

  final int discriminator;
  final int migrationVersion;
  final BigInt data;
  final Uint8List label;

  @override
  String get name => 'myOtherEvent';

  String toString() =>
      'MyOtherEventEvent(discriminator: ${discriminator}, migrationVersion: ${migrationVersion}, data: ${data}, label: ${label})';
}

/// The discriminator this event is emitted under.
const myOtherEventEventDiscriminator = 2;

/// The discriminator bytes as stored at offset zero.
const List<int> _myOtherEventEventDiscriminatorBytes = [2];

/// The migration version this event decodes.
const myOtherEventEventMigrationVersion = 0;

/// Exact current byte length of a `MyOtherEvent` record, envelope included.
const myOtherEventEventSize = 18;

/// Decode one `MyOtherEvent` record.
MyOtherEventEvent decodeMyOtherEventEvent(Uint8List data) {
  if (data.length != myOtherEventEventSize) {
    throw RangeError(
      'expected exactly ${myOtherEventEventSize} bytes, received ${data.length}',
    );
  }
  var cursor = 0;
  final (v0, c0) = getU8Decoder().read(data, cursor);
  cursor = c0;
  if (v0 != 2) {
    throw RangeError(
      'the provided bytes do not match the "MyOtherEvent" event discriminator',
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
  final (v3, c3) = fixDecoderSize(getBytesDecoder(), 8).read(data, cursor);
  cursor = c3;

  return MyOtherEventEvent(
    discriminator: v0,
    migrationVersion: v1,
    data: v2,
    label: v3,
  );
}

/// A decoded `MyOtherEvent` log record.
typedef DecodedMyOtherEventEvent = MyOtherEventEvent;

/// Decode a `Program data:` log line, or return null when the line is not
/// this event.
MyOtherEventEvent? parseMyOtherEventEventFromLog(String log) {
  final bytes = decodeProgramDataLog(log);
  if (bytes == null || bytes.length < 2) {
    return null;
  }
  for (var index = 0; index < 1; index++) {
    if (bytes[index] != _myOtherEventEventDiscriminatorBytes[index]) {
      return null;
    }
  }
  if (bytes[1] != myOtherEventEventMigrationVersion) {
    return null;
  }
  return decodeMyOtherEventEvent(bytes);
}
