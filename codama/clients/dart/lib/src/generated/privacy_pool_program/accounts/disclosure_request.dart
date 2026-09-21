// Auto-generated. Do not edit.
// ignore_for_file: type=lint


import 'dart:typed_data';

import 'package:meta/meta.dart';
import 'package:solana_kit_accounts/solana_kit_accounts.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';


@immutable
class DisclosureRequest {
  const DisclosureRequest({
    required this.bump,
    required this.requester,
    required this.nonce,
    required this.tier,
    required this.commitment,
    required this.noticeLen,
    required this.notice,
    required this.legalBasisHash,
    required this.createdAt,
    required this.challengeDeadline,
    required this.status,
    required this.granted,
    required this.approvals,
  }) :
      discriminator = 9,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final Address requester;
  final BigInt nonce;
  final int tier;
  final Uint8List commitment;
  final int noticeLen;
  final Uint8List notice;
  final Uint8List legalBasisHash;
  final BigInt createdAt;
  final BigInt challengeDeadline;
  final int status;
  final int granted;
  final int approvals;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is DisclosureRequest &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          bump == other.bump &&
          requester == other.requester &&
          nonce == other.nonce &&
          tier == other.tier &&
          commitment == other.commitment &&
          noticeLen == other.noticeLen &&
          notice == other.notice &&
          legalBasisHash == other.legalBasisHash &&
          createdAt == other.createdAt &&
          challengeDeadline == other.challengeDeadline &&
          status == other.status &&
          granted == other.granted &&
          approvals == other.approvals;

  @override
  int get hashCode => Object.hash(discriminator, migrationVersion, bump, requester, nonce, tier, commitment, noticeLen, notice, legalBasisHash, createdAt, challengeDeadline, status, granted, approvals);

  @override
  String toString() => 'DisclosureRequest(discriminator: $discriminator, migrationVersion: $migrationVersion, bump: $bump, requester: $requester, nonce: $nonce, tier: $tier, commitment: $commitment, noticeLen: $noticeLen, notice: $notice, legalBasisHash: $legalBasisHash, createdAt: $createdAt, challengeDeadline: $challengeDeadline, status: $status, granted: $granted, approvals: $approvals)';
}


Encoder<DisclosureRequest> getDisclosureRequestEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('requester', getAddressEncoder()),
    ('nonce', getU64Encoder()),
    ('tier', getU8Encoder()),
    ('commitment', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
    ('noticeLen', getU8Encoder()),
    ('notice', fixEncoderSize(getBytesEncoder(), 96, allowTruncation: false)),
    ('legalBasisHash', fixEncoderSize(getBytesEncoder(), 32, allowTruncation: false)),
    ('createdAt', getU64Encoder()),
    ('challengeDeadline', getU64Encoder()),
    ('status', getU8Encoder()),
    ('granted', getU8Encoder()),
    ('approvals', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (DisclosureRequest value) => <String, Object?>{
      'discriminator': 9,
      'migrationVersion': 0,
      'bump': value.bump,
      'requester': value.requester,
      'nonce': value.nonce,
      'tier': value.tier,
      'commitment': value.commitment,
      'noticeLen': value.noticeLen,
      'notice': value.notice,
      'legalBasisHash': value.legalBasisHash,
      'createdAt': value.createdAt,
      'challengeDeadline': value.challengeDeadline,
      'status': value.status,
      'granted': value.granted,
      'approvals': value.approvals,
    },
  );
}

Decoder<DisclosureRequest> getDisclosureRequestDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('requester', getAddressDecoder()),
    ('nonce', getU64Decoder()),
    ('tier', getU8Decoder()),
    ('commitment', fixDecoderSize(getBytesDecoder(), 32)),
    ('noticeLen', getU8Decoder()),
    ('notice', fixDecoderSize(getBytesDecoder(), 96)),
    ('legalBasisHash', fixDecoderSize(getBytesDecoder(), 32)),
    ('createdAt', getU64Decoder()),
    ('challengeDeadline', getU64Decoder()),
    ('status', getU8Decoder()),
    ('granted', getU8Decoder()),
    ('approvals', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'disclosureRequest account decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (DisclosureRequest, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(9),
    ).read(bytes, offset + 0);
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
      DisclosureRequest(
      bump: map['bump']! as int,
      requester: map['requester']! as Address,
      nonce: map['nonce']! as BigInt,
      tier: map['tier']! as int,
      commitment: map['commitment']! as Uint8List,
      noticeLen: map['noticeLen']! as int,
      notice: map['notice']! as Uint8List,
      legalBasisHash: map['legalBasisHash']! as Uint8List,
      createdAt: map['createdAt']! as BigInt,
      challengeDeadline: map['challengeDeadline']! as BigInt,
      status: map['status']! as int,
      granted: map['granted']! as int,
      approvals: map['approvals']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<DisclosureRequest>(
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
      VariableSizeDecoder<DisclosureRequest>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<DisclosureRequest, DisclosureRequest> getDisclosureRequestCodec() {
  return combineCodec(getDisclosureRequestEncoder(), getDisclosureRequestDecoder());
}

Account<DisclosureRequest> decodeDisclosureRequest(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getDisclosureRequestDecoder());
}

/// The account schema version this client was generated from.
const int disclosureRequestMigrationVersion = 0;

/// Cheap envelope check for fetched `DisclosureRequest` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool disclosureRequestNeedsMigration(List<int> data) {
	if (data.length < 2) {
		return false;
	}
	if (data[0] != 9) {
		return false;
	}
	return data[1] < 0;
}
