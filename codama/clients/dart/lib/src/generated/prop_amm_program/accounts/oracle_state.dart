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
class OracleState {
  const OracleState({required this.authority, required this.price})
    : discriminator = 1;

  final int discriminator;
  final Address authority;
  final BigInt price;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is OracleState &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          authority == other.authority &&
          price == other.price;

  @override
  int get hashCode => Object.hash(discriminator, authority, price);

  @override
  String toString() =>
      'OracleState(discriminator: $discriminator, authority: $authority, price: $price)';
}

Encoder<OracleState> getOracleStateEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('authority', getAddressEncoder()),
    ('price', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (OracleState value) => <String, Object?>{
      'discriminator': 1,
      'authority': value.authority,
      'price': value.price,
    },
  );
}

Decoder<OracleState> getOracleStateDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('authority', getAddressDecoder()),
    ('price', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'oracleState account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (OracleState, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);

    return (
      OracleState(
        authority: map['authority']! as Address,
        price: map['price']! as BigInt,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() => FixedSizeDecoder<OracleState>(
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
      VariableSizeDecoder<OracleState>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<OracleState, OracleState> getOracleStateCodec() {
  return combineCodec(getOracleStateEncoder(), getOracleStateDecoder());
}

Account<OracleState> decodeOracleState(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getOracleStateDecoder());
}
