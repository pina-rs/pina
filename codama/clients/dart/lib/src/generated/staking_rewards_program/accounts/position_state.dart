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
class PositionState {
  const PositionState({
    required this.pool,
    required this.owner,
    required this.stakedAmount,
    required this.rewardDebt,
    required this.pendingRewards,
    required this.bump,
  }) : discriminator = 2;

  final int discriminator;
  final Address pool;
  final Address owner;
  final BigInt stakedAmount;
  final BigInt rewardDebt;
  final BigInt pendingRewards;
  final int bump;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is PositionState &&
          runtimeType == other.runtimeType &&
          discriminator == other.discriminator &&
          pool == other.pool &&
          owner == other.owner &&
          stakedAmount == other.stakedAmount &&
          rewardDebt == other.rewardDebt &&
          pendingRewards == other.pendingRewards &&
          bump == other.bump;

  @override
  int get hashCode => Object.hash(
    discriminator,
    pool,
    owner,
    stakedAmount,
    rewardDebt,
    pendingRewards,
    bump,
  );

  @override
  String toString() =>
      'PositionState(discriminator: $discriminator, pool: $pool, owner: $owner, stakedAmount: $stakedAmount, rewardDebt: $rewardDebt, pendingRewards: $pendingRewards, bump: $bump)';
}

Encoder<PositionState> getPositionStateEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('pool', getAddressEncoder()),
    ('owner', getAddressEncoder()),
    ('stakedAmount', getU64Encoder()),
    ('rewardDebt', getU64Encoder()),
    ('pendingRewards', getU64Encoder()),
    ('bump', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (PositionState value) => <String, Object?>{
      'discriminator': 2,
      'pool': value.pool,
      'owner': value.owner,
      'stakedAmount': value.stakedAmount,
      'rewardDebt': value.rewardDebt,
      'pendingRewards': value.pendingRewards,
      'bump': value.bump,
    },
  );
}

Decoder<PositionState> getPositionStateDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('pool', getAddressDecoder()),
    ('owner', getAddressDecoder()),
    ('stakedAmount', getU64Decoder()),
    ('rewardDebt', getU64Decoder()),
    ('pendingRewards', getU64Decoder()),
    ('bump', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'positionState account decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (PositionState, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);

    return (
      PositionState(
        pool: map['pool']! as Address,
        owner: map['owner']! as Address,
        stakedAmount: map['stakedAmount']! as BigInt,
        rewardDebt: map['rewardDebt']! as BigInt,
        pendingRewards: map['pendingRewards']! as BigInt,
        bump: map['bump']! as int,
      ),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() => FixedSizeDecoder<PositionState>(
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
      VariableSizeDecoder<PositionState>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<PositionState, PositionState> getPositionStateCodec() {
  return combineCodec(getPositionStateEncoder(), getPositionStateDecoder());
}

Account<PositionState> decodePositionState(EncodedAccount encodedAccount) {
  return decodeAccount(encodedAccount, getPositionStateDecoder());
}
