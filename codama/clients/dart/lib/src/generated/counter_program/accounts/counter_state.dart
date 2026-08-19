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
class CounterState {
  const CounterState({required this.bump, required this.count})
    : discriminator = 1;

  final int discriminator;
  final int bump;
  final BigInt count;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is CounterState &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          bump == other.bump &&
          count == other.count;

  @override
  int get hashCode => Object.hash(discriminator, bump, count);

  @override
  String toString() =>
      'CounterState(discriminator: $discriminator, bump: $bump, count: $count)';
}

Encoder<CounterState> getCounterStateEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('count', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (CounterState value) => <String, Object?>{
      'discriminator': 1,
      'bump': value.bump,
      'count': value.count,
    },
  );
}

Decoder<CounterState> getCounterStateDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('count', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'counterState account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (CounterState, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);

    return (
      CounterState(bump: map['bump']! as int, count: map['count']! as BigInt),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() => FixedSizeDecoder<CounterState>(
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
      VariableSizeDecoder<CounterState>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<CounterState, CounterState> getCounterStateCodec() {
  return combineCodec(getCounterStateEncoder(), getCounterStateDecoder());
}

Account<CounterState> decodeCounterState(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getCounterStateDecoder());
}
