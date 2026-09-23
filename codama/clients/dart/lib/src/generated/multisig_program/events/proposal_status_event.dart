// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';

import 'event_log.dart';

/// Event record `ProposalStatusEvent`.
class ProposalStatusEventEvent {
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

  String get name => 'proposalStatusEvent';

  String toString() =>
      'ProposalStatusEventEvent(discriminator: ${discriminator}, migrationVersion: ${migrationVersion}, multisig: ${multisig}, index: ${index}, status: ${status}, timestamp: ${timestamp})';
}

/// The discriminator this event is emitted under.
const proposalStatusEventEventDiscriminator = 1;

/// The discriminator bytes as stored at offset zero.
const List<int> _proposalStatusEventEventDiscriminatorBytes = [1];

/// The version this client was generated from.
const proposalStatusEventEventMigrationVersion = 0;

/// Exact current byte length of a `ProposalStatusEvent` record, envelope included.
const proposalStatusEventEventSize = 51;

/// Decode one current-version `ProposalStatusEvent` record.
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
          ? 'event migration version mismatch: expected 0, received $v1 (the log predates this client; project it through the checked-in event history or decode it with a client generated from the schema that wrote it)'
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

/// Event bytes projected into the current shape.
class NormalizedProposalStatusEventEvent extends MultisigProgramEvent {
  const NormalizedProposalStatusEventEvent({
    required this.data,
    required this.sourceVersion,
    required this.wasMigrated,
  });

  final ProposalStatusEventEvent data;

  /// The version carried by the immutable log record, matching the runtime's
  /// `CurrentEventData::source_version`.
  final int sourceVersion;

  /// Whether a historical projection ran.
  final bool wasMigrated;

  @override
  String get name => 'proposalStatusEvent';
}

/// Adjacent projections from the checked-in migration manifest: `(from, to,
/// automatic, source payload size, destination payload size, moves)`.
const List<(int, int, bool, int, int, List<(int, int, int)>)>
_proposalStatusEventProjectionSteps = [];

/// Project current or historical bytes into the current shape, mirroring the
/// runtime's `normalize_event_data`. Unknown, future, non-exact, and manual
/// transitions fail closed.
NormalizedProposalStatusEventEvent normalizeProposalStatusEventEvent(
  Uint8List data,
) {
  if (data.length < 2) {
    throw RangeError(
      'the provided data is too short for the "ProposalStatusEvent" event envelope',
    );
  }
  for (var index = 0; index < 1; index++) {
    if (data[index] != _proposalStatusEventEventDiscriminatorBytes[index]) {
      throw RangeError(
        'the provided data does not match the "ProposalStatusEvent" event discriminator',
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
    return NormalizedProposalStatusEventEvent(
      data: decodeProposalStatusEventEvent(data),
      sourceVersion: sourceVersion,
      wasMigrated: false,
    );
  }
  final projected = _projectProposalStatusEventEvent(data, sourceVersion);
  return NormalizedProposalStatusEventEvent(
    data: decodeProposalStatusEventEvent(projected),
    sourceVersion: sourceVersion,
    wasMigrated: true,
  );
}

Uint8List _projectProposalStatusEventEvent(Uint8List data, int sourceVersion) {
  var version = sourceVersion;
  var payload = Uint8List.fromList(data.sublist(2));
  while (version != 0) {
    (int, int, bool, int, int, List<(int, int, int)>)? step;
    for (final candidate in _proposalStatusEventProjectionSteps) {
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
  projected.setRange(0, 1, _proposalStatusEventEventDiscriminatorBytes);
  projected[1] = 0;
  projected.setRange(2, projected.length, payload);
  return projected;
}

/// A decoded `ProposalStatusEvent` log record.
typedef DecodedProposalStatusEventEvent = NormalizedProposalStatusEventEvent;

/// Decode a `Program data:` log line, or return null when the line is not
/// this event.
NormalizedProposalStatusEventEvent? parseProposalStatusEventEventFromLog(
  String log,
) {
  final bytes = decodeProgramDataLog(log);
  if (bytes == null || bytes.length < 1) {
    return null;
  }
  for (var index = 0; index < 1; index++) {
    if (bytes[index] != _proposalStatusEventEventDiscriminatorBytes[index]) {
      return null;
    }
  }
  return normalizeProposalStatusEventEvent(bytes);
}
