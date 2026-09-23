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
class VerifyingKeyAccount {
  const VerifyingKeyAccount({
    required this.bump,
    required this.slot,
    required this.alphaG1,
    required this.betaG2,
    required this.gammaG2,
    required this.deltaG2,
    required this.icLen,
    required this.ic0,
    required this.ic1,
    required this.ic2,
    required this.ic3,
  }) :
      discriminator = 10,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final int bump;
  final int slot;
  final Uint8List alphaG1;
  final Uint8List betaG2;
  final Uint8List gammaG2;
  final Uint8List deltaG2;
  final int icLen;
  final Uint8List ic0;
  final Uint8List ic1;
  final Uint8List ic2;
  final Uint8List ic3;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is VerifyingKeyAccount &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          bump == other.bump &&
          slot == other.slot &&
          alphaG1 == other.alphaG1 &&
          betaG2 == other.betaG2 &&
          gammaG2 == other.gammaG2 &&
          deltaG2 == other.deltaG2 &&
          icLen == other.icLen &&
          ic0 == other.ic0 &&
          ic1 == other.ic1 &&
          ic2 == other.ic2 &&
          ic3 == other.ic3;

  @override
  int get hashCode => Object.hash(discriminator, migrationVersion, bump, slot, alphaG1, betaG2, gammaG2, deltaG2, icLen, ic0, ic1, ic2, ic3);

  @override
  String toString() => 'VerifyingKeyAccount(discriminator: $discriminator, migrationVersion: $migrationVersion, bump: $bump, slot: $slot, alphaG1: $alphaG1, betaG2: $betaG2, gammaG2: $gammaG2, deltaG2: $deltaG2, icLen: $icLen, ic0: $ic0, ic1: $ic1, ic2: $ic2, ic3: $ic3)';
}


Encoder<VerifyingKeyAccount> getVerifyingKeyAccountEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('slot', getU8Encoder()),
    ('alphaG1', fixEncoderSize(getBytesEncoder(), 64, allowTruncation: false)),
    ('betaG2', fixEncoderSize(getBytesEncoder(), 128, allowTruncation: false)),
    ('gammaG2', fixEncoderSize(getBytesEncoder(), 128, allowTruncation: false)),
    ('deltaG2', fixEncoderSize(getBytesEncoder(), 128, allowTruncation: false)),
    ('icLen', getU8Encoder()),
    ('ic0', fixEncoderSize(getBytesEncoder(), 64, allowTruncation: false)),
    ('ic1', fixEncoderSize(getBytesEncoder(), 64, allowTruncation: false)),
    ('ic2', fixEncoderSize(getBytesEncoder(), 64, allowTruncation: false)),
    ('ic3', fixEncoderSize(getBytesEncoder(), 64, allowTruncation: false)),
  ]);

  return transformEncoder(
    structEncoder,
    (VerifyingKeyAccount value) => <String, Object?>{
      'discriminator': 10,
      'migrationVersion': 0,
      'bump': value.bump,
      'slot': value.slot,
      'alphaG1': value.alphaG1,
      'betaG2': value.betaG2,
      'gammaG2': value.gammaG2,
      'deltaG2': value.deltaG2,
      'icLen': value.icLen,
      'ic0': value.ic0,
      'ic1': value.ic1,
      'ic2': value.ic2,
      'ic3': value.ic3,
    },
  );
}

Decoder<VerifyingKeyAccount> getVerifyingKeyAccountDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('slot', getU8Decoder()),
    ('alphaG1', fixDecoderSize(getBytesDecoder(), 64)),
    ('betaG2', fixDecoderSize(getBytesDecoder(), 128)),
    ('gammaG2', fixDecoderSize(getBytesDecoder(), 128)),
    ('deltaG2', fixDecoderSize(getBytesDecoder(), 128)),
    ('icLen', getU8Decoder()),
    ('ic0', fixDecoderSize(getBytesDecoder(), 64)),
    ('ic1', fixDecoderSize(getBytesDecoder(), 64)),
    ('ic2', fixDecoderSize(getBytesDecoder(), 64)),
    ('ic3', fixDecoderSize(getBytesDecoder(), 64)),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'verifyingKeyAccount account decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (VerifyingKeyAccount, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(10),
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
      VerifyingKeyAccount(
      bump: map['bump']! as int,
      slot: map['slot']! as int,
      alphaG1: map['alphaG1']! as Uint8List,
      betaG2: map['betaG2']! as Uint8List,
      gammaG2: map['gammaG2']! as Uint8List,
      deltaG2: map['deltaG2']! as Uint8List,
      icLen: map['icLen']! as int,
      ic0: map['ic0']! as Uint8List,
      ic1: map['ic1']! as Uint8List,
      ic2: map['ic2']! as Uint8List,
      ic3: map['ic3']! as Uint8List,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<VerifyingKeyAccount>(
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
      VariableSizeDecoder<VerifyingKeyAccount>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<VerifyingKeyAccount, VerifyingKeyAccount> getVerifyingKeyAccountCodec() {
  return combineCodec(getVerifyingKeyAccountEncoder(), getVerifyingKeyAccountDecoder());
}

Account<VerifyingKeyAccount> decodeVerifyingKeyAccount(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getVerifyingKeyAccountDecoder());
}

/// The account schema version this client was generated from.
const int verifyingKeyAccountMigrationVersion = 0;

/// Cheap envelope check for fetched `VerifyingKeyAccount` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool verifyingKeyAccountNeedsMigration(List<int> data) {
	if (data.length < 2) {
		return false;
	}
	if (data[0] != 10) {
		return false;
	}
	return data[1] < 0;
}
