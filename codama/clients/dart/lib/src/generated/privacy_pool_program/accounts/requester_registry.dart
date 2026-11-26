// Auto-generated. Do not edit.
// ignore_for_file: type=lint


import 'dart:typed_data';

import 'package:meta/meta.dart';
import 'package:solana_kit_accounts/solana_kit_accounts.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';


@immutable
class RequesterRegistry {
  const RequesterRegistry({
    required this.bump,
    required this.count,
    required this.keys,
    required this.maxTiers,
  }) :
      discriminator = 6,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final int count;
  final Uint8List keys;
  final Uint8List maxTiers;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is RequesterRegistry &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          bump == other.bump &&
          count == other.count &&
          keys == other.keys &&
          maxTiers == other.maxTiers;

  @override
  int get hashCode => Object.hash(discriminator, migrationVersion, bump, count, keys, maxTiers);

  @override
  String toString() => 'RequesterRegistry(discriminator: $discriminator, migrationVersion: $migrationVersion, bump: $bump, count: $count, keys: $keys, maxTiers: $maxTiers)';
}


Encoder<RequesterRegistry> getRequesterRegistryEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('count', getU8Encoder()),
    ('keys', fixEncoderSize(getBytesEncoder(), 512, allowTruncation: false)),
    ('maxTiers', fixEncoderSize(getBytesEncoder(), 16, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (RequesterRegistry value) => <String, Object?>{
      'discriminator': 6,
      'migrationVersion': 0,
      'bump': value.bump,
      'count': value.count,
      'keys': value.keys,
      'maxTiers': value.maxTiers,
    },
  );
}

Decoder<RequesterRegistry> getRequesterRegistryDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('count', getU8Decoder()),
    ('keys', fixDecoderSize(getBytesDecoder(), 512)),
    ('maxTiers', fixDecoderSize(getBytesDecoder(), 16)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'requesterRegistry account decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (RequesterRegistry, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(6),
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
      RequesterRegistry(
      bump: map['bump']! as int,
      count: map['count']! as int,
      keys: map['keys']! as Uint8List,
      maxTiers: map['maxTiers']! as Uint8List,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<RequesterRegistry>(
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
      VariableSizeDecoder<RequesterRegistry>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<RequesterRegistry, RequesterRegistry> getRequesterRegistryCodec() {
  return combineCodec(getRequesterRegistryEncoder(), getRequesterRegistryDecoder());
}

Account<RequesterRegistry> decodeRequesterRegistry(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getRequesterRegistryDecoder());
}

/// The account schema version this client was generated from.
const int requesterRegistryMigrationVersion = 0;

/// Cheap envelope check for fetched `RequesterRegistry` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool requesterRegistryNeedsMigration(List<int> data) {
	if (data.length < 2) {
		return false;
	}
	if (data[0] != 6) {
		return false;
	}
	return data[1] < 0;
}
