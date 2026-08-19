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
class EscrowState {
  const EscrowState({
    required this.maker,
    required this.mintA,
    required this.mintB,
    required this.amountA,
    required this.amountB,
    required this.seed,
    required this.bump,
  }) : discriminator = 1;

  final int discriminator;
  final Address maker;
  final Address mintA;
  final Address mintB;
  final BigInt amountA;
  final BigInt amountB;
  final BigInt seed;
  final int bump;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is EscrowState &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          maker == other.maker &&
          mintA == other.mintA &&
          mintB == other.mintB &&
          amountA == other.amountA &&
          amountB == other.amountB &&
          seed == other.seed &&
          bump == other.bump;

  @override
  int get hashCode => Object.hash(
    discriminator,
    maker,
    mintA,
    mintB,
    amountA,
    amountB,
    seed,
    bump,
  );

  @override
  String toString() =>
      'EscrowState(discriminator: $discriminator, maker: $maker, mintA: $mintA, mintB: $mintB, amountA: $amountA, amountB: $amountB, seed: $seed, bump: $bump)';
}

Encoder<EscrowState> getEscrowStateEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('maker', getAddressEncoder()),
    ('mintA', getAddressEncoder()),
    ('mintB', getAddressEncoder()),
    ('amountA', getU64Encoder()),
    ('amountB', getU64Encoder()),
    ('seed', getU64Encoder()),
    ('bump', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (EscrowState value) => <String, Object?>{
      'discriminator': 1,
      'maker': value.maker,
      'mintA': value.mintA,
      'mintB': value.mintB,
      'amountA': value.amountA,
      'amountB': value.amountB,
      'seed': value.seed,
      'bump': value.bump,
    },
  );
}

Decoder<EscrowState> getEscrowStateDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('maker', getAddressDecoder()),
    ('mintA', getAddressDecoder()),
    ('mintB', getAddressDecoder()),
    ('amountA', getU64Decoder()),
    ('amountB', getU64Decoder()),
    ('seed', getU64Decoder()),
    ('bump', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'escrowState account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (EscrowState, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);

    return (
      EscrowState(
        maker: map['maker']! as Address,
        mintA: map['mintA']! as Address,
        mintB: map['mintB']! as Address,
        amountA: map['amountA']! as BigInt,
        amountB: map['amountB']! as BigInt,
        seed: map['seed']! as BigInt,
        bump: map['bump']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() => FixedSizeDecoder<EscrowState>(
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
      VariableSizeDecoder<EscrowState>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<EscrowState, EscrowState> getEscrowStateCodec() {
  return combineCodec(getEscrowStateEncoder(), getEscrowStateDecoder());
}

Account<EscrowState> decodeEscrowState(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getEscrowStateDecoder());
}
