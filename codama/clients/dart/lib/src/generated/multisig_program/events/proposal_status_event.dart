// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';

import 'event_log.dart';

/// Event record `ProposalStatusEvent`.
class ProposalStatusEventEvent extends MultisigProgramEvent {
  const ProposalStatusEventEvent({
    required this.discriminator,
    required this.migrationVersion,
    required this.multisig,
    required this.index,
    required this.status,
    required this.timestamp,
  });

  final int discriminator;
  final int migrationVersion;
  final Address multisig;
  final BigInt index;
  final int status;
  final BigInt timestamp;

  @override
  String get name => 'proposalStatusEvent';

  String toString() =>
      'ProposalStatusEventEvent(discriminator: ${discriminator}, migrationVersion: ${migrationVersion}, multisig: ${multisig}, index: ${index}, status: ${status}, timestamp: ${timestamp})';
}

/// The discriminator this event is emitted under.
const proposalStatusEventEventDiscriminator = 1;

/// The discriminator bytes as stored at offset zero.
const List<int> _proposalStatusEventEventDiscriminatorBytes = [1];

/// The migration version this event decodes.
const proposalStatusEventEventMigrationVersion = 0;

/// Exact current byte length of a `ProposalStatusEvent` record, envelope included.
const proposalStatusEventEventSize = 51;

/// Decode one `ProposalStatusEvent` record.
ProposalStatusEventEvent decodeProposalStatusEventEvent(Uint8List data) {
  if (data.length != proposalStatusEventEventSize) {
    throw RangeError(
      'expected exactly ${proposalStatusEventEventSize} bytes, received ${data.length}',
    );
  }
  var cursor = 0;
  final (v0, c0) = getU8Decoder().read(data, cursor);
  cursor = c0;
  if (v0 != 1) {
    throw RangeError(
      'the provided bytes do not match the "ProposalStatusEvent" event discriminator',
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
  final (v2, c2) = getAddressDecoder().read(data, cursor);
  cursor = c2;
  final (v3, c3) = getU64Decoder().read(data, cursor);
  cursor = c3;
  final (v4, c4) = getU8Decoder().read(data, cursor);
  cursor = c4;
  final (v5, c5) = getI64Decoder().read(data, cursor);
  cursor = c5;

  return ProposalStatusEventEvent(
    discriminator: v0,
    migrationVersion: v1,
    multisig: v2,
    index: v3,
    status: v4,
    timestamp: v5,
  );
}

/// A decoded `ProposalStatusEvent` log record.
typedef DecodedProposalStatusEventEvent = ProposalStatusEventEvent;

/// Decode a `Program data:` log line, or return null when the line is not
/// this event.
ProposalStatusEventEvent? parseProposalStatusEventEventFromLog(String log) {
  final bytes = decodeProgramDataLog(log);
  if (bytes == null || bytes.length < 2) {
    return null;
  }
  for (var index = 0; index < 1; index++) {
    if (bytes[index] != _proposalStatusEventEventDiscriminatorBytes[index]) {
      return null;
    }
  }
  if (bytes[1] != proposalStatusEventEventMigrationVersion) {
    return null;
  }
  return decodeProposalStatusEventEvent(bytes);
}
