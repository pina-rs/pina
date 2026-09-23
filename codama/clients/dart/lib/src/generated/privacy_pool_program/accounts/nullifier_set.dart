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
class NullifierSet {
  const NullifierSet({
    required this.bump,
    required this.count,
    required this.nullifiers,
  }) :
      discriminator = 4,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final BigInt count;
  final Uint8List nullifiers;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is NullifierSet &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          bump == other.bump &&
          count == other.count &&
          nullifiers == other.nullifiers;

  @override
  int get hashCode => Object.hash(discriminator, migrationVersion, bump, count, nullifiers);

  @override
  String toString() => 'NullifierSet(discriminator: $discriminator, migrationVersion: $migrationVersion, bump: $bump, count: $count, nullifiers: $nullifiers)';
}


Encoder<NullifierSet> getNullifierSetEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('count', getU64Encoder()),
    ('nullifiers', fixEncoderSize(getBytesEncoder(), 4096, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (NullifierSet value) => <String, Object?>{
      'discriminator': 4,
      'migrationVersion': 0,
      'bump': value.bump,
      'count': value.count,
      'nullifiers': value.nullifiers,
    },
  );
}

Decoder<NullifierSet> getNullifierSetDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('count', getU64Decoder()),
    ('nullifiers', fixDecoderSize(getBytesDecoder(), 4096)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'nullifierSet account decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (NullifierSet, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(4),
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
      NullifierSet(
      bump: map['bump']! as int,
      count: map['count']! as BigInt,
      nullifiers: map['nullifiers']! as Uint8List,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<NullifierSet>(
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
      VariableSizeDecoder<NullifierSet>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<NullifierSet, NullifierSet> getNullifierSetCodec() {
  return combineCodec(getNullifierSetEncoder(), getNullifierSetDecoder());
}

Account<NullifierSet> decodeNullifierSet(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getNullifierSetDecoder());
}

/// The account schema version this client was generated from.
const int nullifierSetMigrationVersion = 0;

/// Cheap envelope check for fetched `NullifierSet` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool nullifierSetNeedsMigration(List<int> data) {
	if (data.length < 2) {
		return false;
	}
	if (data[0] != 4) {
		return false;
	}
	return data[1] < 0;
}
