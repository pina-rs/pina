// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import '../pina_pod_codecs.dart';
import 'dart:typed_data';

import 'package:meta/meta.dart';
import 'package:solana_kit_accounts/solana_kit_accounts.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';

@immutable
class Proposal {
  const Proposal({
    required this.bump,
    required this.multisig,
    required this.creator,
    required this.index,
    required this.kind,
    required this.vaultIndex,
    required this.vaultBump,
    required this.status,
    required this.statusAt,
    required this.expiresAt,
    required this.approvedMask,
    required this.rejectedMask,
    required this.ephemeralBumps,
    required this.message,
    required this.actions,
  }) : discriminator = 3,
       migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final Address multisig;
  final Address creator;
  final BigInt index;
  final int kind;
  final int vaultIndex;
  final int vaultBump;
  final int status;
  final BigInt statusAt;
  final BigInt expiresAt;
  final int approvedMask;
  final int rejectedMask;
  final List<int> ephemeralBumps;
  final List<int> message;
  final List<int> actions;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is Proposal &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          bump == other.bump &&
          multisig == other.multisig &&
          creator == other.creator &&
          index == other.index &&
          kind == other.kind &&
          vaultIndex == other.vaultIndex &&
          vaultBump == other.vaultBump &&
          status == other.status &&
          statusAt == other.statusAt &&
          expiresAt == other.expiresAt &&
          approvedMask == other.approvedMask &&
          rejectedMask == other.rejectedMask &&
          ephemeralBumps == other.ephemeralBumps &&
          message == other.message &&
          actions == other.actions;

  @override
  int get hashCode => Object.hash(
    discriminator,
    migrationVersion,
    bump,
    multisig,
    creator,
    index,
    kind,
    vaultIndex,
    vaultBump,
    status,
    statusAt,
    expiresAt,
    approvedMask,
    rejectedMask,
    ephemeralBumps,
    message,
    actions,
  );

  @override
  String toString() =>
      'Proposal(discriminator: $discriminator, migrationVersion: $migrationVersion, bump: $bump, multisig: $multisig, creator: $creator, index: $index, kind: $kind, vaultIndex: $vaultIndex, vaultBump: $vaultBump, status: $status, statusAt: $statusAt, expiresAt: $expiresAt, approvedMask: $approvedMask, rejectedMask: $rejectedMask, ephemeralBumps: $ephemeralBumps, message: $message, actions: $actions)';
}

Encoder<Proposal> getProposalEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('multisig', getAddressEncoder()),
    ('creator', getAddressEncoder()),
    ('index', getU64Encoder()),
    ('kind', getU8Encoder()),
    ('vaultIndex', getU8Encoder()),
    ('vaultBump', getU8Encoder()),
    ('status', getU8Encoder()),
    ('statusAt', getI64Encoder()),
    ('expiresAt', getI64Encoder()),
    ('approvedMask', getU32Encoder()),
    ('rejectedMask', getU32Encoder()),
    (
      'ephemeralBumps',
      offsetEncoder(
        getPinaPodBoundedArrayEncoder(
          getArrayEncoder(
            transformEncoder(getU8Encoder(), (int value) => value),
            size: PrefixedArraySize(
              offsetEncoder(
                offsetEncoder(
                  getU8Encoder(),
                  OffsetConfig(preOffset: (scope) => 103),
                ),
                OffsetConfig(postOffset: (scope) => scope.preOffset + 0),
              ),
            ),
          ),
          4,
        ),
        OffsetConfig(preOffset: (scope) => scope.preOffset + 5),
      ),
    ),
    (
      'message',
      getPinaPodBoundedArrayEncoder(
        getArrayEncoder(
          transformEncoder(getU8Encoder(), (int value) => value),
          size: PrefixedArraySize(
            offsetEncoder(
              offsetEncoder(
                getU16Encoder(),
                OffsetConfig(preOffset: (scope) => 104),
              ),
              OffsetConfig(postOffset: (scope) => scope.preOffset + 0),
            ),
          ),
        ),
        640,
      ),
    ),
    (
      'actions',
      getPinaPodBoundedArrayEncoder(
        getArrayEncoder(
          transformEncoder(getU8Encoder(), (int value) => value),
          size: PrefixedArraySize(
            offsetEncoder(
              offsetEncoder(
                getU16Encoder(),
                OffsetConfig(preOffset: (scope) => 106),
              ),
              OffsetConfig(postOffset: (scope) => scope.preOffset + 0),
            ),
          ),
        ),
        128,
      ),
    ),
  ]);

  return transformEncoder(
    structEncoder,
    (Proposal value) => <String, Object?>{
      'discriminator': 3,
      'migrationVersion': 0,
      'bump': value.bump,
      'multisig': value.multisig,
      'creator': value.creator,
      'index': value.index,
      'kind': value.kind,
      'vaultIndex': value.vaultIndex,
      'vaultBump': value.vaultBump,
      'status': value.status,
      'statusAt': value.statusAt,
      'expiresAt': value.expiresAt,
      'approvedMask': value.approvedMask,
      'rejectedMask': value.rejectedMask,
      'ephemeralBumps': value.ephemeralBumps,
      'message': value.message,
      'actions': value.actions,
    },
  );
}

Decoder<Proposal> getProposalDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('multisig', getAddressDecoder()),
    ('creator', getAddressDecoder()),
    ('index', getU64Decoder()),
    ('kind', getU8Decoder()),
    ('vaultIndex', getU8Decoder()),
    ('vaultBump', getU8Decoder()),
    ('status', getU8Decoder()),
    ('statusAt', getI64Decoder()),
    ('expiresAt', getI64Decoder()),
    ('approvedMask', getU32Decoder()),
    ('rejectedMask', getU32Decoder()),
    (
      'ephemeralBumps',
      offsetDecoder(
        getPinaPodBoundedArrayDecoder(
          getArrayDecoder(
            getU8Decoder(),
            size: PrefixedArraySize(
              offsetDecoder(
                offsetDecoder(
                  getU8Decoder(),
                  OffsetConfig(preOffset: (scope) => 103),
                ),
                OffsetConfig(postOffset: (scope) => scope.preOffset + 0),
              ),
            ),
          ),
          getPinaPodBoundedCountDecoder(
            offsetDecoder(
              offsetDecoder(
                getU8Decoder(),
                OffsetConfig(preOffset: (scope) => 103),
              ),
              OffsetConfig(postOffset: (scope) => scope.preOffset + 0),
            ),
            4,
          ),
          4,
        ),
        OffsetConfig(preOffset: (scope) => scope.preOffset + 5),
      ),
    ),
    (
      'message',
      getPinaPodBoundedArrayDecoder(
        getArrayDecoder(
          getU8Decoder(),
          size: PrefixedArraySize(
            offsetDecoder(
              offsetDecoder(
                getU16Decoder(),
                OffsetConfig(preOffset: (scope) => 104),
              ),
              OffsetConfig(postOffset: (scope) => scope.preOffset + 0),
            ),
          ),
        ),
        getPinaPodBoundedCountDecoder(
          offsetDecoder(
            offsetDecoder(
              getU16Decoder(),
              OffsetConfig(preOffset: (scope) => 104),
            ),
            OffsetConfig(postOffset: (scope) => scope.preOffset + 0),
          ),
          640,
        ),
        640,
      ),
    ),
    (
      'actions',
      getPinaPodBoundedArrayDecoder(
        getArrayDecoder(
          getU8Decoder(),
          size: PrefixedArraySize(
            offsetDecoder(
              offsetDecoder(
                getU16Decoder(),
                OffsetConfig(preOffset: (scope) => 106),
              ),
              OffsetConfig(postOffset: (scope) => scope.preOffset + 0),
            ),
          ),
        ),
        getPinaPodBoundedCountDecoder(
          offsetDecoder(
            offsetDecoder(
              getU16Decoder(),
              OffsetConfig(preOffset: (scope) => 106),
            ),
            OffsetConfig(postOffset: (scope) => scope.preOffset + 0),
          ),
          128,
        ),
        128,
      ),
    ),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'proposal account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (Proposal, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(3)).read(bytes, offset + 0);
    final (storedMigrationVersion, _) = getU8Decoder().read(bytes, offset + 1);
    if (storedMigrationVersion != 0) {
      throw StateError(
        storedMigrationVersion < 0
            ? 'migration version mismatch: expected 0, received $storedMigrationVersion (the data predates this client; migrate it by sending a transaction to the program, or decode it with a client generated from an older IDL)'
            : 'migration version mismatch: expected 0, received $storedMigrationVersion (the data was written by a newer program; upgrade this client)',
      );
    }
    final (map, newOffset) = structDecoder.read(bytes, offset);

    return (
      Proposal(
        bump: map['bump']! as int,
        multisig: map['multisig']! as Address,
        creator: map['creator']! as Address,
        index: map['index']! as BigInt,
        kind: map['kind']! as int,
        vaultIndex: map['vaultIndex']! as int,
        vaultBump: map['vaultBump']! as int,
        status: map['status']! as int,
        statusAt: map['statusAt']! as BigInt,
        expiresAt: map['expiresAt']! as BigInt,
        approvedMask: map['approvedMask']! as int,
        rejectedMask: map['rejectedMask']! as int,
        ephemeralBumps: map['ephemeralBumps']! as List<int>,
        message: map['message']! as List<int>,
        actions: map['actions']! as List<int>,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() => FixedSizeDecoder<Proposal>(
      fixedSize: structDecoder.fixedSize,
      read: (bytes, offset) {
        final bytesLength = bytes.length - offset;
        if (bytesLength < structDecoder.fixedSize) {
          throwInvalidByteLength(structDecoder.fixedSize, bytesLength);
        }
        return readTopLevel(bytes, offset);
      },
    ),
    VariableSizeDecoder<Map<String, Object?>>() =>
      VariableSizeDecoder<Proposal>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<Proposal, Proposal> getProposalCodec() {
  return combineCodec(getProposalEncoder(), getProposalDecoder());
}

Account<Proposal> decodeProposal(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getProposalDecoder());
}

/// The account schema version this client was generated from.
const int proposalMigrationVersion = 0;

/// Cheap envelope check for fetched `Proposal` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool proposalNeedsMigration(List<int> data) {
  if (data.length < 2) {
    return false;
  }
  if (data[0] != 3) {
    return false;
  }
  return data[1] < 0;
}
