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
class PoolState {
  const PoolState({
    required this.admin,
    required this.stakeMint,
    required this.rewardMint,
    required this.totalStaked,
    required this.rewardIndex,
    required this.paused,
    required this.bump,
  }) : discriminator = 1;

  final int discriminator;
  final Address admin;
  final Address stakeMint;
  final Address rewardMint;
  final BigInt totalStaked;
  final BigInt rewardIndex;
  final bool paused;
  final int bump;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is PoolState &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          admin == other.admin &&
          stakeMint == other.stakeMint &&
          rewardMint == other.rewardMint &&
          totalStaked == other.totalStaked &&
          rewardIndex == other.rewardIndex &&
          paused == other.paused &&
          bump == other.bump;

  @override
  int get hashCode => Object.hash(
    discriminator,
    admin,
    stakeMint,
    rewardMint,
    totalStaked,
    rewardIndex,
    paused,
    bump,
  );

  @override
  String toString() =>
      'PoolState(discriminator: $discriminator, admin: $admin, stakeMint: $stakeMint, rewardMint: $rewardMint, totalStaked: $totalStaked, rewardIndex: $rewardIndex, paused: $paused, bump: $bump)';
}

Encoder<PoolState> getPoolStateEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('admin', getAddressEncoder()),
    ('stakeMint', getAddressEncoder()),
    ('rewardMint', getAddressEncoder()),
    ('totalStaked', getU64Encoder()),
    ('rewardIndex', getU64Encoder()),
    ('paused', getBooleanEncoder()),
    ('bump', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (PoolState value) => <String, Object?>{
      'discriminator': 1,
      'admin': value.admin,
      'stakeMint': value.stakeMint,
      'rewardMint': value.rewardMint,
      'totalStaked': value.totalStaked,
      'rewardIndex': value.rewardIndex,
      'paused': value.paused,
      'bump': value.bump,
    },
  );
}

Decoder<PoolState> getPoolStateDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('admin', getAddressDecoder()),
    ('stakeMint', getAddressDecoder()),
    ('rewardMint', getAddressDecoder()),
    ('totalStaked', getU64Decoder()),
    ('rewardIndex', getU64Decoder()),
    ('paused', getBooleanDecoder()),
    ('bump', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'poolState account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (PoolState, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);

    return (
      PoolState(
        admin: map['admin']! as Address,
        stakeMint: map['stakeMint']! as Address,
        rewardMint: map['rewardMint']! as Address,
        totalStaked: map['totalStaked']! as BigInt,
        rewardIndex: map['rewardIndex']! as BigInt,
        paused: map['paused']! as bool,
        bump: map['bump']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() => FixedSizeDecoder<PoolState>(
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
      VariableSizeDecoder<PoolState>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<PoolState, PoolState> getPoolStateCodec() {
  return combineCodec(getPoolStateEncoder(), getPoolStateDecoder());
}

Account<PoolState> decodePoolState(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getPoolStateDecoder());
}
