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
class AssetVault {
  const AssetVault({
    required this.bump,
    required this.mint,
    required this.tokenProgram,
    required this.vaultTokenAccount,
  }) :
      discriminator = 8,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final Address mint;
  final Address tokenProgram;
  final Address vaultTokenAccount;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is AssetVault &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          bump == other.bump &&
          mint == other.mint &&
          tokenProgram == other.tokenProgram &&
          vaultTokenAccount == other.vaultTokenAccount;

  @override
  int get hashCode => Object.hash(discriminator, migrationVersion, bump, mint, tokenProgram, vaultTokenAccount);

  @override
  String toString() => 'AssetVault(discriminator: $discriminator, migrationVersion: $migrationVersion, bump: $bump, mint: $mint, tokenProgram: $tokenProgram, vaultTokenAccount: $vaultTokenAccount)';
}


Encoder<AssetVault> getAssetVaultEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('mint', getAddressEncoder()),
    ('tokenProgram', getAddressEncoder()),
    ('vaultTokenAccount', getAddressEncoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (AssetVault value) => <String, Object?>{
      'discriminator': 8,
      'migrationVersion': 0,
      'bump': value.bump,
      'mint': value.mint,
      'tokenProgram': value.tokenProgram,
      'vaultTokenAccount': value.vaultTokenAccount,
    },
  );
}

Decoder<AssetVault> getAssetVaultDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('mint', getAddressDecoder()),
    ('tokenProgram', getAddressDecoder()),
    ('vaultTokenAccount', getAddressDecoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'assetVault account decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (AssetVault, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(8),
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
      AssetVault(
      bump: map['bump']! as int,
      mint: map['mint']! as Address,
      tokenProgram: map['tokenProgram']! as Address,
      vaultTokenAccount: map['vaultTokenAccount']! as Address,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<AssetVault>(
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
      VariableSizeDecoder<AssetVault>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<AssetVault, AssetVault> getAssetVaultCodec() {
  return combineCodec(getAssetVaultEncoder(), getAssetVaultDecoder());
}

Account<AssetVault> decodeAssetVault(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getAssetVaultDecoder());
}

/// The account schema version this client was generated from.
const int assetVaultMigrationVersion = 0;

/// Cheap envelope check for fetched `AssetVault` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool assetVaultNeedsMigration(List<int> data) {
	if (data.length < 2) {
		return false;
	}
	if (data[0] != 8) {
		return false;
	}
	return data[1] < 0;
}
