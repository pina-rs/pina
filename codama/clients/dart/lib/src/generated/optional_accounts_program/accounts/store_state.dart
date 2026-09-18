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
class StoreState {
  const StoreState({required this.bump, required this.count})
    : discriminator = 1,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final BigInt count;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is StoreState &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          bump == other.bump &&
          count == other.count;

  @override
  int get hashCode => Object.hash(discriminator, migrationVersion, bump, count);

  @override
  String toString() =>
      'StoreState(discriminator: $discriminator, migrationVersion: $migrationVersion, bump: $bump, count: $count)';
}

Encoder<StoreState> getStoreStateEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('count', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (StoreState value) => <String, Object?>{
      'discriminator': 1,
      'migrationVersion': 0,
      'bump': value.bump,
      'count': value.count,
    },
  );
}

Decoder<StoreState> getStoreStateDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('count', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'storeState account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (StoreState, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
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
      StoreState(bump: map['bump']! as int, count: map['count']! as BigInt),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() => FixedSizeDecoder<StoreState>(
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
      VariableSizeDecoder<StoreState>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<StoreState, StoreState> getStoreStateCodec() {
  return combineCodec(getStoreStateEncoder(), getStoreStateDecoder());
}

Account<StoreState> decodeStoreState(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getStoreStateDecoder());
}

/// The account schema version this client was generated from.
const int storeStateMigrationVersion = 0;

/// Cheap envelope check for fetched `StoreState` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool storeStateNeedsMigration(List<int> data) {
  if (data.length < 2) {
    return false;
  }
  if (data[0] != 1) {
    return false;
  }
  return data[1] < 0;
}
