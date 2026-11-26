// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';

import 'event_log.dart';

/// Event record `NoteEncryptedEvent`.
class NoteEncryptedEventEvent extends PrivacyPoolProgramEvent {
  const NoteEncryptedEventEvent({
    required this.discriminator,
    required this.migrationVersion,
    required this.leafIndex,
    required this.commitment,
    required this.viewPubkey,
    required this.envelopeLen,
    required this.envelope,
    required this.shares,
  });

  final int discriminator;
  final int migrationVersion;
  final BigInt leafIndex;
  final Uint8List commitment;
  final Uint8List viewPubkey;
  final int envelopeLen;
  final Uint8List envelope;
  final Uint8List shares;

  @override
  String get name => 'noteEncryptedEvent';

  String toString() =>
      'NoteEncryptedEventEvent(discriminator: ${discriminator}, migrationVersion: ${migrationVersion}, leafIndex: ${leafIndex}, commitment: ${commitment}, viewPubkey: ${viewPubkey}, envelopeLen: ${envelopeLen}, envelope: ${envelope}, shares: ${shares})';
}

/// The discriminator this event is emitted under.
const noteEncryptedEventEventDiscriminator = 1;

/// The discriminator bytes as stored at offset zero.
const List<int> _noteEncryptedEventEventDiscriminatorBytes = [1];

/// The migration version this event decodes.
const noteEncryptedEventEventMigrationVersion = 0;

/// Exact current byte length of a `NoteEncryptedEvent` record, envelope included.
const noteEncryptedEventEventSize = 315;

/// Decode one `NoteEncryptedEvent` record.
NoteEncryptedEventEvent decodeNoteEncryptedEventEvent(Uint8List data) {
  if (data.length != noteEncryptedEventEventSize) {
    throw RangeError(
      'expected exactly ${noteEncryptedEventEventSize} bytes, received ${data.length}',
    );
  }
  var cursor = 0;
  final (v0, c0) = getU8Decoder().read(data, cursor);
  cursor = c0;
  if (v0 != 1) {
    throw RangeError(
      'the provided bytes do not match the "NoteEncryptedEvent" event discriminator',
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
  final (v3, c3) = fixDecoderSize(getBytesDecoder(), 32).read(data, cursor);
  cursor = c3;
  final (v4, c4) = fixDecoderSize(getBytesDecoder(), 32).read(data, cursor);
  cursor = c4;
  final (v5, c5) = getU8Decoder().read(data, cursor);
  cursor = c5;
  final (v6, c6) = fixDecoderSize(getBytesDecoder(), 96).read(data, cursor);
  cursor = c6;
  final (v7, c7) = fixDecoderSize(getBytesDecoder(), 144).read(data, cursor);
  cursor = c7;

  return NoteEncryptedEventEvent(
    discriminator: v0,
    migrationVersion: v1,
    leafIndex: v2,
    commitment: v3,
    viewPubkey: v4,
    envelopeLen: v5,
    envelope: v6,
    shares: v7,
  );
}

/// A decoded `NoteEncryptedEvent` log record.
typedef DecodedNoteEncryptedEventEvent = NoteEncryptedEventEvent;

/// Decode a `Program data:` log line, or return null when the line is not
/// this event.
NoteEncryptedEventEvent? parseNoteEncryptedEventEventFromLog(String log) {
  final bytes = decodeProgramDataLog(log);
  if (bytes == null || bytes.length < 2) {
    return null;
  }
  for (var index = 0; index < 1; index++) {
    if (bytes[index] != _noteEncryptedEventEventDiscriminatorBytes[index]) {
      return null;
    }
  }
  if (bytes[1] != noteEncryptedEventEventMigrationVersion) {
    return null;
  }
  return decodeNoteEncryptedEventEvent(bytes);
}
