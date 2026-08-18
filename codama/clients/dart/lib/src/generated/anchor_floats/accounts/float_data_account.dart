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
class FloatDataAccount {
  const FloatDataAccount({
    required this.dataF64,
    required this.dataF32,
    required this.authority,
  }) : discriminator = 1;

  final int discriminator;
  final BigInt dataF64;
  final int dataF32;
  final Address authority;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is FloatDataAccount &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          dataF64 == other.dataF64 &&
          dataF32 == other.dataF32 &&
          authority == other.authority;

  @override
  int get hashCode => Object.hash(discriminator, dataF64, dataF32, authority);

  @override
  String toString() =>
      'FloatDataAccount(discriminator: $discriminator, dataF64: $dataF64, dataF32: $dataF32, authority: $authority)';
}

Encoder<FloatDataAccount> getFloatDataAccountEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('dataF64', getU64Encoder()),
    ('dataF32', getU32Encoder()),
    ('authority', getAddressEncoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (FloatDataAccount value) => <String, Object?>{
      'discriminator': 1,
      'dataF64': value.dataF64,
      'dataF32': value.dataF32,
      'authority': value.authority,
    },
  );
}

Decoder<FloatDataAccount> getFloatDataAccountDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('dataF64', getU64Decoder()),
    ('dataF32', getU32Decoder()),
    ('authority', getAddressDecoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'floatDataAccount account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (FloatDataAccount, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (
      FloatDataAccount(
        dataF64: map['dataF64']! as BigInt,
        dataF32: map['dataF32']! as int,
        authority: map['authority']! as Address,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<FloatDataAccount>(
        fixedSize: structDecoder.fixedSize,
        read: (bytes, offset) {
          final bytesLength = bytes.length - offset;
          if (bytesLength != structDecoder.fixedSize) {
            throwInvalidByteLength(structDecoder.fixedSize, bytesLength);
          }
          return readExact(bytes, offset);
        },
      ),
    VariableSizeDecoder<Map<String, Object?>>() =>
      VariableSizeDecoder<FloatDataAccount>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<FloatDataAccount, FloatDataAccount> getFloatDataAccountCodec() {
  return combineCodec(
    getFloatDataAccountEncoder(),
    getFloatDataAccountDecoder(),
  );
}

Account<FloatDataAccount> decodeFloatDataAccount(
  EncodedAccount encodedAccount,
) {
  return decodeAccount(encodedAccount, getFloatDataAccountDecoder());
}
