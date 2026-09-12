// Auto-generated. Do not edit.
// ignore_for_file: type=lint


import '../pina_pod_codecs.dart';
import 'dart:typed_data';

import 'package:meta/meta.dart';
import 'package:solana_kit_accounts/solana_kit_accounts.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_codecs_strings/solana_kit_codecs_strings.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';


@immutable
class Journal {
  const Journal({
    required this.bump,
    required this.authority,
    required this.revision,
    required this.featuredEntry,
    required this.title,
    required this.entries,
    required this.markers,
    required this.note,
  }) :
      discriminator = 1;

  final int discriminator;
  final int bump;
  final Address authority;
  final int revision;
  final BigInt? featuredEntry;
  final String title;
  final List<BigInt> entries;
  final List<int> markers;
  final String? note;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is Journal &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          bump == other.bump &&
          authority == other.authority &&
          revision == other.revision &&
          featuredEntry == other.featuredEntry &&
          title == other.title &&
          entries == other.entries &&
          markers == other.markers &&
          note == other.note;

  @override
  int get hashCode => Object.hash(discriminator, bump, authority, revision, featuredEntry, title, entries, markers, note);

  @override
  String toString() => 'Journal(discriminator: $discriminator, bump: $bump, authority: $authority, revision: $revision, featuredEntry: $featuredEntry, title: $title, entries: $entries, markers: $markers, note: $note)';
}


Encoder<Journal> getJournalEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('authority', getAddressEncoder()),
    ('revision', getU32Encoder()),
    ('featuredEntry', getNullableEncoder<BigInt>(transformEncoder(getU64Encoder(), (BigInt value) => value), noneValue: const ZeroesNoneValue())),
    ('title', offsetEncoder(getPinaPodBoundedStringEncoder(addEncoderSizePrefix(getUtf8Encoder(), offsetEncoder(offsetEncoder(getU8Encoder(), OffsetConfig(preOffset: (scope) => 47)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0))), 24), OffsetConfig(preOffset: (scope) => scope.preOffset + 12))),
    ('entries', getPinaPodBoundedArrayEncoder(getArrayEncoder(transformEncoder(getU64Encoder(), (BigInt value) => value), size: PrefixedArraySize(offsetEncoder(offsetEncoder(getU16Encoder(), OffsetConfig(preOffset: (scope) => 48)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)))), 8)),
    ('markers', getPinaPodBoundedArrayEncoder(getArrayEncoder(transformEncoder(getU8Encoder(), (int value) => value), size: PrefixedArraySize(offsetEncoder(offsetEncoder(getU64Encoder(), OffsetConfig(preOffset: (scope) => 50)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)))), 8)),
    ('note', getNullableEncoder<String>(transformEncoder(getPinaPodBoundedStringEncoder(addEncoderSizePrefix(getUtf8Encoder(), getU8Encoder()), 64), (String value) => value), prefix: offsetEncoder(offsetEncoder(getU8Encoder(), OffsetConfig(preOffset: (scope) => 58)), OffsetConfig(postOffset: (scope) => scope.preOffset)))),
  ]);

  return transformEncoder(
    structEncoder,
    (Journal value) => <String, Object?>{
      'discriminator': 1,
      'bump': value.bump,
      'authority': value.authority,
      'revision': value.revision,
      'featuredEntry': value.featuredEntry,
      'title': value.title,
      'entries': value.entries,
      'markers': value.markers,
      'note': value.note,
    },
  );
}

Decoder<Journal> getJournalDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('authority', getAddressDecoder()),
    ('revision', getU32Decoder()),
    ('featuredEntry', getNullableDecoder<BigInt>(getU64Decoder(), noneValue: const ZeroesNoneValue())),
    ('title', offsetDecoder(getPinaPodBoundedStringDecoder(addDecoderSizePrefix(getUtf8Decoder(), getPinaPodBoundedCountDecoder(offsetDecoder(offsetDecoder(getU8Decoder(), OffsetConfig(preOffset: (scope) => 47)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)), 24)), 24), OffsetConfig(preOffset: (scope) => scope.preOffset + 12))),
    ('entries', getPinaPodBoundedArrayDecoder(getArrayDecoder(getU64Decoder(), size: PrefixedArraySize(offsetDecoder(offsetDecoder(getU16Decoder(), OffsetConfig(preOffset: (scope) => 48)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)))), getPinaPodBoundedCountDecoder(offsetDecoder(offsetDecoder(getU16Decoder(), OffsetConfig(preOffset: (scope) => 48)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)), 8), 8)),
    ('markers', getPinaPodBoundedArrayDecoder(getArrayDecoder(getU8Decoder(), size: PrefixedArraySize(offsetDecoder(offsetDecoder(getU64Decoder(), OffsetConfig(preOffset: (scope) => 50)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)))), getPinaPodBoundedCountDecoder(offsetDecoder(offsetDecoder(getU64Decoder(), OffsetConfig(preOffset: (scope) => 50)), OffsetConfig(postOffset: (scope) => scope.preOffset + 0)), 8), 8)),
    ('note', getNullableDecoder<String>(getPinaPodBoundedStringDecoder(addDecoderSizePrefix(getUtf8Decoder(), getPinaPodBoundedCountDecoder(getU8Decoder(), 64)), 64), prefix: offsetDecoder(offsetDecoder(getU8Decoder(), OffsetConfig(preOffset: (scope) => 58)), OffsetConfig(postOffset: (scope) => scope.preOffset)))),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'journal account decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (Journal, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(1),
    ).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);

    return (
      Journal(
      bump: map['bump']! as int,
      authority: map['authority']! as Address,
      revision: map['revision']! as int,
      featuredEntry: map['featuredEntry'] as BigInt?,
      title: map['title']! as String,
      entries: map['entries']! as List<BigInt>,
      markers: map['markers']! as List<int>,
      note: map['note'] as String?,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<Journal>(
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
      VariableSizeDecoder<Journal>(
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
