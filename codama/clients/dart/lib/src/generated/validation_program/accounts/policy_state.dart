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
class PolicyState {
  const PolicyState({
    required this.bump,
    required this.minimum,
    required this.maximum,
    required this.requiredApprovals,
  }) :
      discriminator = 1;

  final int discriminator;
  final int bump;
  final BigInt minimum;
  final BigInt maximum;
  final int requiredApprovals;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is PolicyState &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          bump == other.bump &&
          minimum == other.minimum &&
          maximum == other.maximum &&
          requiredApprovals == other.requiredApprovals;

  @override
  int get hashCode => Object.hash(discriminator, bump, minimum, maximum, requiredApprovals);

  @override
  String toString() => 'PolicyState(discriminator: $discriminator, bump: $bump, minimum: $minimum, maximum: $maximum, requiredApprovals: $requiredApprovals)';
}


Encoder<PolicyState> getPolicyStateEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('bump', getU8Encoder()),
    ('minimum', getU64Encoder()),
    ('maximum', getU64Encoder()),
    ('requiredApprovals', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (PolicyState value) => <String, Object?>{
      'discriminator': 1,
      'bump': value.bump,
      'minimum': value.minimum,
      'maximum': value.maximum,
      'requiredApprovals': value.requiredApprovals,
    },
  );
}

Decoder<PolicyState> getPolicyStateDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('bump', getU8Decoder()),
    ('minimum', getU64Decoder()),
    ('maximum', getU64Decoder()),
    ('requiredApprovals', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(
      SolanaErrorCode.codecsInvalidByteLength,
      {
        'codecDescription': 'policyState account decoder',
        'expected': expected,
        'bytesLength': bytesLength,
      },
    );
  }

  (PolicyState, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(
      getU8Encoder().encode(1),
    ).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);

    return (
      PolicyState(
      bump: map['bump']! as int,
      minimum: map['minimum']! as BigInt,
      maximum: map['maximum']! as BigInt,
      requiredApprovals: map['requiredApprovals']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<PolicyState>(
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
      VariableSizeDecoder<PolicyState>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<PolicyState, PolicyState> getPolicyStateCodec() {
  return combineCodec(getPolicyStateEncoder(), getPolicyStateDecoder());
}

Account<PolicyState> decodePolicyState(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getPolicyStateDecoder());
}
