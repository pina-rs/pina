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
class CustodianRegistry {
  const CustodianRegistry({
    required this.bump,
    required this.custodians,
  }) :
      discriminator = 5,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final Uint8List custodians;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is CustodianRegistry &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          bump == other.bump &&
          custodians == other.custodians;

  @override
  int get hashCode => Object.hash(discriminator, migrationVersion, bump, custodians);

  @override
  String toString() => 'CustodianRegistry(discriminator: $discriminator, migrationVersion: $migrationVersion, bump: $bump, custodians: $custodians)';
}


Encoder<CustodianRegistry> getCustodianRegistryEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('custodians', fixEncoderSize(getBytesEncoder(), 96, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (CustodianRegistry value) => <String, Object?>{
      'discriminator': 5,
      'migrationVersion': 0,
      'bump': value.bump,
      'custodians': value.custodians,
    },
  );
}

Decoder<CustodianRegistry> getCustodianRegistryDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('custodians', fixDecoderSize(getBytesDecoder(), 96)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'custodianRegistry account decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (CustodianRegistry, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(5),
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
      CustodianRegistry(
      bump: map['bump']! as int,
      custodians: map['custodians']! as Uint8List,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<CustodianRegistry>(
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
      VariableSizeDecoder<CustodianRegistry>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<CustodianRegistry, CustodianRegistry> getCustodianRegistryCodec() {
  return combineCodec(getCustodianRegistryEncoder(), getCustodianRegistryDecoder());
}

Account<CustodianRegistry> decodeCustodianRegistry(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getCustodianRegistryDecoder());
}

/// The account schema version this client was generated from.
const int custodianRegistryMigrationVersion = 0;

/// Cheap envelope check for fetched `CustodianRegistry` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool custodianRegistryNeedsMigration(List<int> data) {
	if (data.length < 2) {
		return false;
	}
	if (data[0] != 5) {
		return false;
	}
	return data[1] < 0;
}
