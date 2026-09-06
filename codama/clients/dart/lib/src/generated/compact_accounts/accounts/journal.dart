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
class Journal {
  const Journal({
    required this.bump,
    required this.authority,
    required this.revision,
    required this.entries,
    required this.markers,
  }) : discriminator = 1;

  final int discriminator;
  final int bump;
  final Address authority;
  final int revision;
  final List<BigInt> entries;
  final List<int> markers;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is Journal &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          bump == other.bump &&
          authority == other.authority &&
          revision == other.revision &&
          entries == other.entries &&
          markers == other.markers;

  @override
  int get hashCode =>
      Object.hash(discriminator, bump, authority, revision, entries, markers);

  @override
  String toString() =>
      'Journal(discriminator: $discriminator, bump: $bump, authority: $authority, revision: $revision, entries: $entries, markers: $markers)';
}

Encoder<Journal> getJournalEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('authority', getAddressEncoder()),
    ('revision', getU32Encoder()),
    (
      'entries',
      offsetEncoder(
        getArrayEncoder(
          transformEncoder(getU64Encoder(), (BigInt value) => value),
          size: PrefixedArraySize(
            offsetEncoder(
              offsetEncoder(
                getU16Encoder(),
                OffsetConfig(preOffset: (scope) => 38),
              ),
              OffsetConfig(postOffset: (scope) => scope.preOffset + 0),
            ),
          ),
        ),
        OffsetConfig(preOffset: (scope) => scope.preOffset + 10),
      ),
    ),
    (
      'markers',
      getArrayEncoder(
        transformEncoder(getU8Encoder(), (int value) => value),
        size: PrefixedArraySize(
          offsetEncoder(
            offsetEncoder(
              getU64Encoder(),
              OffsetConfig(preOffset: (scope) => 40),
            ),
            OffsetConfig(postOffset: (scope) => scope.preOffset + 0),
          ),
        ),
      ),
    ),
  ]);

  return transformEncoder(
    structEncoder,
    (Journal value) => <String, Object?>{
      'discriminator': 1,
      'bump': value.bump,
      'authority': value.authority,
      'revision': value.revision,
      'entries': value.entries,
      'markers': value.markers,
    },
  );
}

Decoder<Journal> getJournalDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('authority', getAddressDecoder()),
    ('revision', getU32Decoder()),
    (
      'entries',
      offsetDecoder(
        getArrayDecoder(
          getU64Decoder(),
          size: PrefixedArraySize(
            offsetDecoder(
              offsetDecoder(
                getU16Decoder(),
                OffsetConfig(preOffset: (scope) => 38),
              ),
              OffsetConfig(postOffset: (scope) => scope.preOffset + 0),
            ),
          ),
        ),
        OffsetConfig(preOffset: (scope) => scope.preOffset + 10),
      ),
    ),
    (
      'markers',
      getArrayDecoder(
        getU8Decoder(),
        size: PrefixedArraySize(
          offsetDecoder(
            offsetDecoder(
              getU64Decoder(),
              OffsetConfig(preOffset: (scope) => 40),
            ),
            OffsetConfig(postOffset: (scope) => scope.preOffset + 0),
          ),
        ),
      ),
    ),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'journal account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (Journal, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);

    return (
      Journal(
        bump: map['bump']! as int,
        authority: map['authority']! as Address,
        revision: map['revision']! as int,
        entries: map['entries']! as List<BigInt>,
        markers: map['markers']! as List<int>,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() => FixedSizeDecoder<Journal>(
      fixedSize: structDecoder.fixedSize,
      read: (bytes, offset) {
        final bytesLength = bytes.length - offset;
        if (bytesLength < structDecoder.fixedSize) {
          throwInvalidByteLength(structDecoder.fixedSize, bytesLength);
        }
        return readTopLevel(bytes, offset);
      },
    ),
    VariableSizeDecoder<Map<String, Object?>>() => VariableSizeDecoder<Journal>(
      read: readTopLevel,
      maxSize: structDecoder.maxSize,
    ),
  };
}

Codec<Journal, Journal> getJournalCodec() {
  return combineCodec(getJournalEncoder(), getJournalDecoder());
}

Account<Journal> decodeJournal(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getJournalDecoder());
}
