// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import '../pina_pod_codecs.dart';
import 'dart:typed_data';

import 'package:meta/meta.dart';
import 'package:solana_kit_accounts/solana_kit_accounts.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_codecs_strings/solana_kit_codecs_strings.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';

@immutable
class ManualState {
  const ManualState({required this.code})
    : discriminator = 2,
      migrationVersion = 2;

  final int discriminator;
  final int migrationVersion;
  final String code;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is ManualState &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          code == other.code;

  @override
  int get hashCode => Object.hash(discriminator, migrationVersion, code);

  @override
  String toString() =>
      'ManualState(discriminator: $discriminator, migrationVersion: $migrationVersion, code: $code)';
}

Encoder<ManualState> getManualStateEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    (
      'code',
      getPinaPodBoundedStringEncoder(
        addEncoderSizePrefix(getUtf8Encoder(), getU8Encoder()),
        5,
      ),
    ),
  ]);

  return transformEncoder(
    structEncoder,
    (ManualState value) => <String, Object?>{
      'discriminator': 2,
      'migrationVersion': 2,
      'code': value.code,
    },
  );
}

Decoder<ManualState> getManualStateDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    (
      'code',
      getPinaPodBoundedStringDecoder(
        addDecoderSizePrefix(
          getUtf8Decoder(),
          getPinaPodBoundedCountDecoder(getU8Decoder(), 5),
        ),
        5,
      ),
    ),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'manualState account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (ManualState, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 0);
    final (storedMigrationVersion, _) = getU8Decoder().read(bytes, offset + 1);
    if (storedMigrationVersion != 2) {
      throw StateError(
        storedMigrationVersion < 2
            ? 'migration version mismatch: expected 2, received $storedMigrationVersion (the data predates this client; migrate it by sending a transaction to the program, or decode it with a client generated from an older IDL)'
            : 'migration version mismatch: expected 2, received $storedMigrationVersion (the data was written by a newer program; upgrade this client)',
      );
    }
    final (map, newOffset) = structDecoder.read(bytes, offset);

    return (ManualState(code: map['code']! as String), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() => FixedSizeDecoder<ManualState>(
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
      VariableSizeDecoder<ManualState>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<ManualState, ManualState> getManualStateCodec() {
  return combineCodec(getManualStateEncoder(), getManualStateDecoder());
}

Account<ManualState> decodeManualState(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getManualStateDecoder());
}

/// The account schema version this client was generated from.
const int manualStateMigrationVersion = 2;

/// Cheap envelope check for fetched `ManualState` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool manualStateNeedsMigration(List<int> data) {
  if (data.length < 2) {
    return false;
  }
  if (data[0] != 2) {
    return false;
  }
  return data[1] < 2;
}
