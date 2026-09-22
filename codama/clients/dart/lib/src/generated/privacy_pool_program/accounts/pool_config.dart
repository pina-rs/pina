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
class PoolConfig {
  const PoolConfig({
    required this.bump,
    required this.authority,
    required this.custodianThreshold,
    required this.challengeWindowSecs,
    required this.depositLamports,
    required this.totalDeposits,
  }) : discriminator = 1,
       migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final Address authority;
  final int custodianThreshold;
  final int challengeWindowSecs;
  final BigInt depositLamports;
  final BigInt totalDeposits;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is PoolConfig &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          bump == other.bump &&
          authority == other.authority &&
          custodianThreshold == other.custodianThreshold &&
          challengeWindowSecs == other.challengeWindowSecs &&
          depositLamports == other.depositLamports &&
          totalDeposits == other.totalDeposits;

  @override
  int get hashCode => Object.hash(
    discriminator,
    migrationVersion,
    bump,
    authority,
    custodianThreshold,
    challengeWindowSecs,
    depositLamports,
    totalDeposits,
  );

  @override
  String toString() =>
      'PoolConfig(discriminator: $discriminator, migrationVersion: $migrationVersion, bump: $bump, authority: $authority, custodianThreshold: $custodianThreshold, challengeWindowSecs: $challengeWindowSecs, depositLamports: $depositLamports, totalDeposits: $totalDeposits)';
}

Encoder<PoolConfig> getPoolConfigEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('authority', getAddressEncoder()),
    ('custodianThreshold', getU16Encoder()),
    ('challengeWindowSecs', getU32Encoder()),
    ('depositLamports', getU64Encoder()),
    ('totalDeposits', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (PoolConfig value) => <String, Object?>{
      'discriminator': 1,
      'migrationVersion': 0,
      'bump': value.bump,
      'authority': value.authority,
      'custodianThreshold': value.custodianThreshold,
      'challengeWindowSecs': value.challengeWindowSecs,
      'depositLamports': value.depositLamports,
      'totalDeposits': value.totalDeposits,
    },
  );
}

Decoder<PoolConfig> getPoolConfigDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('authority', getAddressDecoder()),
    ('custodianThreshold', getU16Decoder()),
    ('challengeWindowSecs', getU32Decoder()),
    ('depositLamports', getU64Decoder()),
    ('totalDeposits', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'poolConfig account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (PoolConfig, int) readTopLevel(Uint8List bytes, int offset) {
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
      PoolConfig(
        bump: map['bump']! as int,
        authority: map['authority']! as Address,
        custodianThreshold: map['custodianThreshold']! as int,
        challengeWindowSecs: map['challengeWindowSecs']! as int,
        depositLamports: map['depositLamports']! as BigInt,
        totalDeposits: map['totalDeposits']! as BigInt,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() => FixedSizeDecoder<PoolConfig>(
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
      VariableSizeDecoder<PoolConfig>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<PoolConfig, PoolConfig> getPoolConfigCodec() {
  return combineCodec(getPoolConfigEncoder(), getPoolConfigDecoder());
}

Account<PoolConfig> decodePoolConfig(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getPoolConfigDecoder());
}

/// The account schema version this client was generated from.
const int poolConfigMigrationVersion = 0;

/// Cheap envelope check for fetched `PoolConfig` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool poolConfigNeedsMigration(List<int> data) {
  if (data.length < 2) {
    return false;
  }
  if (data[0] != 1) {
    return false;
  }
  return data[1] < 0;
}
