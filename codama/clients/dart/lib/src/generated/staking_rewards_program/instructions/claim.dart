// Auto-generated. Do not edit.
// ignore_for_file: type=lint

import 'dart:typed_data';

import 'package:meta/meta.dart';
import 'package:solana_kit_addresses/solana_kit_addresses.dart';
import 'package:solana_kit_codecs_core/solana_kit_codecs_core.dart';
import 'package:solana_kit_codecs_data_structures/solana_kit_codecs_data_structures.dart';
import 'package:solana_kit_codecs_numbers/solana_kit_codecs_numbers.dart';
import 'package:solana_kit_errors/solana_kit_errors.dart';
import 'package:solana_kit_instructions/solana_kit_instructions.dart';

@immutable
class ClaimInstructionData {
  const ClaimInstructionData() : discriminator = 4;

  final int discriminator;
}

Encoder<ClaimInstructionData> getClaimInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (ClaimInstructionData value) => <String, Object?>{'discriminator': 4},
  );
}

Decoder<ClaimInstructionData> getClaimInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'claim instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (ClaimInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(4)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (ClaimInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<ClaimInstructionData>(
        fixedSize: structDecoder.fixedSize,
        read: (bytes, offset) {
          final bytesLength = bytes.length - offset;
          if (bytesLength != structDecoder.fixedSize) {
            throwInvalidByteLength(structDecoder.fixedSize, bytesLength);
          }
          return readTopLevel(bytes, offset);
        },
      ),
    VariableSizeDecoder<Map<String, Object?>>() =>
      VariableSizeDecoder<ClaimInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<ClaimInstructionData, ClaimInstructionData>
getClaimInstructionDataCodec() {
  return combineCodec(
    getClaimInstructionDataEncoder(),
    getClaimInstructionDataDecoder(),
  );
}

/// Creates a [Claim] instruction.
Instruction getClaimInstruction({
  required Address programAddress,
  required Address user,
  required Address rewardMint,
  required Address poolState,
  required Address positionState,
  required Address userRewardAta,
  required Address associatedTokenProgram,
  required Address tokenProgram,
  required Address systemProgram,
}) {
  final instructionData = ClaimInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: user, role: AccountRole.writableSigner),
      AccountMeta(address: rewardMint, role: AccountRole.readonly),
      AccountMeta(address: poolState, role: AccountRole.readonly),
      AccountMeta(address: positionState, role: AccountRole.writable),
      AccountMeta(address: userRewardAta, role: AccountRole.writable),
      AccountMeta(address: associatedTokenProgram, role: AccountRole.readonly),
      AccountMeta(address: tokenProgram, role: AccountRole.readonly),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
    ],
    data: getClaimInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Claim] instruction from raw instruction data.
ClaimInstructionData parseClaimInstruction(Instruction instruction) {
  return getClaimInstructionDataDecoder().decode(instruction.data!);
}
