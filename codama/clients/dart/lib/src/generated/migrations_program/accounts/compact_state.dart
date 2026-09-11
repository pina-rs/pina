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
class CompactState {
  const CompactState({required this.name, required this.tags})
    : discriminator = 3,
      migrationVersion = 1;

  final int discriminator;
  final int migrationVersion;
  final String name;
  final List<int> tags;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is CompactState &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          migrationVersion == other.migrationVersion &&
          name == other.name &&
          tags == other.tags;

  @override
  int get hashCode => Object.hash(discriminator, migrationVersion, name, tags);

  @override
  String toString() =>
      'CompactState(discriminator: $discriminator, migrationVersion: $migrationVersion, name: $name, tags: $tags)';
}

Encoder<CompactState> getCompactStateEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    (
      'name',
      offsetEncoder(
        getPinaPodBoundedStringEncoder(
          addEncoderSizePrefix(
            getUtf8Encoder(),
            offsetEncoder(
              offsetEncoder(
                getU8Encoder(),
                OffsetConfig(preOffset: (scope) => 2),
              ),
              OffsetConfig(postOffset: (scope) => scope.preOffset + 0),
            ),
          ),
          4,
        ),
        OffsetConfig(preOffset: (scope) => scope.preOffset + 3),
      ),
    ),
    (
      'tags',
      getPinaPodBoundedArrayEncoder(
        getArrayEncoder(
          transformEncoder(getU16Encoder(), (int value) => value),
          size: PrefixedArraySize(
            offsetEncoder(
              offsetEncoder(
                getU16Encoder(),
                OffsetConfig(preOffset: (scope) => 3),
              ),
              OffsetConfig(postOffset: (scope) => scope.preOffset + 0),
            ),
          ),
        ),
        2,
      ),
    ),
  ]);

  return transformEncoder(
    structEncoder,
    (CompactState value) => <String, Object?>{
      'discriminator': 3,
      'migrationVersion': 1,
      'name': value.name,
      'tags': value.tags,
    },
  );
}

Decoder<CompactState> getCompactStateDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    (
      'name',
      offsetDecoder(
        getPinaPodBoundedStringDecoder(
          addDecoderSizePrefix(
            getUtf8Decoder(),
            getPinaPodBoundedCountDecoder(
              offsetDecoder(
                offsetDecoder(
                  getU8Decoder(),
                  OffsetConfig(preOffset: (scope) => 2),
                ),
                OffsetConfig(postOffset: (scope) => scope.preOffset + 0),
              ),
              4,
            ),
          ),
          4,
        ),
        OffsetConfig(preOffset: (scope) => scope.preOffset + 3),
      ),
    ),
    (
      'tags',
      getPinaPodBoundedArrayDecoder(
        getArrayDecoder(
          getU16Decoder(),
          size: PrefixedArraySize(
            offsetDecoder(
              offsetDecoder(
                getU16Decoder(),
                OffsetConfig(preOffset: (scope) => 3),
              ),
              OffsetConfig(postOffset: (scope) => scope.preOffset + 0),
            ),
          ),
        ),
        getPinaPodBoundedCountDecoder(
          offsetDecoder(
            offsetDecoder(
              getU16Decoder(),
              OffsetConfig(preOffset: (scope) => 3),
            ),
            OffsetConfig(postOffset: (scope) => scope.preOffset + 0),
          ),
          2,
        ),
        2,
      ),
    ),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'compactState account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (CompactState, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(3)).read(bytes, offset + 0);
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);

    return (
      CompactState(
        name: map['name']! as String,
        tags: map['tags']! as List<int>,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() => FixedSizeDecoder<CompactState>(
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
      VariableSizeDecoder<CompactState>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<CompactState, CompactState> getCompactStateCodec() {
  return combineCodec(getCompactStateEncoder(), getCompactStateDecoder());
}

Account<CompactState> decodeCompactState(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getCompactStateDecoder());
}

/// The account schema version this client was generated from.
const int compactStateMigrationVersion = 1;

/// Cheap envelope check for fetched `CompactState` bytes: returns true only when
/// the bytes carry this account's discriminator and a migration version older
/// than this client's schema — exactly the accounts [getMigrateInstruction]
/// can bring current. Decoding reports every other mismatch.
bool compactStateNeedsMigration(List<int> data) {
  if (data.length < 2) {
    return false;
  }
  if (data[0] != 3) {
    return false;
  }
  return data[1] < 1;
}
