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
class Sample {
  const Sample({required this.bump, required this.authority})
    : discriminator = 1;

  final int discriminator;
  final int bump;
  final Address authority;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is Sample &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          bump == other.bump &&
          authority == other.authority;

  @override
  int get hashCode => Object.hash(discriminator, bump, authority);

  @override
  String toString() =>
      'Sample(discriminator: $discriminator, bump: $bump, authority: $authority)';
}

Encoder<Sample> getSampleEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('authority', getAddressEncoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (Sample value) => <String, Object?>{
      'discriminator': 1,
      'bump': value.bump,
      'authority': value.authority,
    },
  );
}

Decoder<Sample> getSampleDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('authority', getAddressDecoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'sample account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (Sample, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);

    return (
      Sample(
        bump: map['bump']! as int,
        authority: map['authority']! as Address,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() => FixedSizeDecoder<Sample>(
      fixedSize: structDecoder.fixedSize,
      read: (bytes, offset) {
        final bytesLength = bytes.length - offset;
        if (bytesLength < structDecoder.fixedSize) {
          throwInvalidByteLength(structDecoder.fixedSize, bytesLength);
        }
        return readTopLevel(bytes, offset);
      },
    ),
    VariableSizeDecoder<Map<String, Object?>>() => VariableSizeDecoder<Sample>(
      read: readTopLevel,
      maxSize: structDecoder.maxSize,
    ),
  };
}

Codec<Sample, Sample> getSampleCodec() {
  return combineCodec(getSampleEncoder(), getSampleDecoder());
}

Account<Sample> decodeSample(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getSampleDecoder());
}
