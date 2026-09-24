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
class RegistryConfig {
  const RegistryConfig({
    required this.admin,
    required this.roleCount,
    required this.bump,
  }) : discriminator = 1,
       migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final Address admin;
  final BigInt roleCount;
  final int bump;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is RegistryConfig &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          admin == other.admin &&
          roleCount == other.roleCount &&
          bump == other.bump;

  @override
  int get hashCode =>
      Object.hash(discriminator, migrationVersion, admin, roleCount, bump);

  @override
  String toString() =>
      'RegistryConfig(discriminator: $discriminator, migrationVersion: $migrationVersion, admin: $admin, roleCount: $roleCount, bump: $bump)';
}

Encoder<RegistryConfig> getRegistryConfigEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('admin', getAddressEncoder()),
    ('roleCount', getU64Encoder()),
    ('bump', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (RegistryConfig value) => <String, Object?>{
      'discriminator': 1,
      'migrationVersion': 0,
      'admin': value.admin,
      'roleCount': value.roleCount,
      'bump': value.bump,
    },
  );
}

Decoder<RegistryConfig> getRegistryConfigDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('admin', getAddressDecoder()),
    ('roleCount', getU64Decoder()),
    ('bump', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'registryConfig account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (RegistryConfig, int) readTopLevel(Uint8List bytes, int offset) {
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
      RegistryConfig(
        admin: map['admin']! as Address,
        roleCount: map['roleCount']! as BigInt,
        bump: map['bump']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<RegistryConfig>(
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
      VariableSizeDecoder<RegistryConfig>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<RegistryConfig, RegistryConfig> getRegistryConfigCodec() {
  return combineCodec(getRegistryConfigEncoder(), getRegistryConfigDecoder());
}

Account<RegistryConfig> decodeRegistryConfig(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getRegistryConfigDecoder());
}

/// The account schema version this client was generated from.
const int registryConfigMigrationVersion = 0;

/// Cheap envelope check for fetched `RegistryConfig` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool registryConfigNeedsMigration(List<int> data) {
  if (data.length < 2) {
    return false;
  }
  if (data[0] != 1) {
    return false;
  }
  return data[1] < 0;
}
