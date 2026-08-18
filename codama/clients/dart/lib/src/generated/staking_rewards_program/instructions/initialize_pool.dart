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
class InitializePoolInstructionData {
  const InitializePoolInstructionData({required this.bump}) : discriminator = 0;

  final int discriminator;
  final int bump;
}

Encoder<InitializePoolInstructionData>
getInitializePoolInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('bump', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (InitializePoolInstructionData value) => <String, Object?>{
      'discriminator': 0,
      'bump': value.bump,
    },
  );
}

Decoder<InitializePoolInstructionData>
getInitializePoolInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('bump', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'initializePool instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (InitializePoolInstructionData, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (
      InitializePoolInstructionData(bump: map['bump']! as int),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<InitializePoolInstructionData>(
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
      VariableSizeDecoder<InitializePoolInstructionData>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<InitializePoolInstructionData, InitializePoolInstructionData>
getInitializePoolInstructionDataCodec() {
  return combineCodec(
    getInitializePoolInstructionDataEncoder(),
    getInitializePoolInstructionDataDecoder(),
  );
}

/// Creates a [InitializePool] instruction.
Instruction getInitializePoolInstruction({
  required Address programAddress,
  required Address admin,
  required Address stakeMint,
  required Address rewardMint,
  required Address poolState,
  required Address stakeVault,
  required Address rewardVault,
  required Address associatedTokenProgram,
  required Address systemProgram,
  required Address tokenProgram,
  required int bump,
}) {
  final instructionData = InitializePoolInstructionData(bump: bump);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: admin, role: AccountRole.writableSigner),
      AccountMeta(address: stakeMint, role: AccountRole.readonly),
      AccountMeta(address: rewardMint, role: AccountRole.readonly),
      AccountMeta(address: poolState, role: AccountRole.writable),
      AccountMeta(address: stakeVault, role: AccountRole.writable),
      AccountMeta(address: rewardVault, role: AccountRole.writable),
      AccountMeta(address: associatedTokenProgram, role: AccountRole.readonly),
      AccountMeta(address: systemProgram, role: AccountRole.readonly),
      AccountMeta(address: tokenProgram, role: AccountRole.readonly),
    ],
    data: getInitializePoolInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [InitializePool] instruction from raw instruction data.
InitializePoolInstructionData parseInitializePoolInstruction(
  Instruction instruction,
) {
  return getInitializePoolInstructionDataDecoder().decode(instruction.data!);
}
