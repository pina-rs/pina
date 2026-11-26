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
class DisclosureLog {
  const DisclosureLog({
    required this.bump,
    required this.count,
    required this.entries,
  }) : discriminator = 7,
       migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final BigInt count;
  final Uint8List entries;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is DisclosureLog &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          bump == other.bump &&
          count == other.count &&
          entries == other.entries;

  @override
  int get hashCode =>
      Object.hash(discriminator, migrationVersion, bump, count, entries);

  @override
  String toString() =>
      'DisclosureLog(discriminator: $discriminator, migrationVersion: $migrationVersion, bump: $bump, count: $count, entries: $entries)';
}

Encoder<DisclosureLog> getDisclosureLogEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('count', getU64Encoder()),
    (
      'entries',
      fixEncoderSize(getBytesEncoder(), 3072, allowTruncation: false),
    ),
  ]);

  return transformEncoder(
    structEncoder,
    (DisclosureLog value) => <String, Object?>{
      'discriminator': 7,
      'migrationVersion': 0,
      'bump': value.bump,
      'count': value.count,
      'entries': value.entries,
    },
  );
}

Decoder<DisclosureLog> getDisclosureLogDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('count', getU64Decoder()),
    ('entries', fixDecoderSize(getBytesDecoder(), 3072)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'disclosureLog account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (DisclosureLog, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(7)).read(bytes, offset + 0);
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
      DisclosureLog(
        bump: map['bump']! as int,
        count: map['count']! as BigInt,
        entries: map['entries']! as Uint8List,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() => FixedSizeDecoder<DisclosureLog>(
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
      VariableSizeDecoder<DisclosureLog>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<DisclosureLog, DisclosureLog> getDisclosureLogCodec() {
  return combineCodec(getDisclosureLogEncoder(), getDisclosureLogDecoder());
}

Account<DisclosureLog> decodeDisclosureLog(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getDisclosureLogDecoder());
}

/// The account schema version this client was generated from.
const int disclosureLogMigrationVersion = 0;

/// Cheap envelope check for fetched `DisclosureLog` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool disclosureLogNeedsMigration(List<int> data) {
  if (data.length < 2) {
    return false;
  }
  if (data[0] != 7) {
    return false;
  }
  return data[1] < 0;
}
