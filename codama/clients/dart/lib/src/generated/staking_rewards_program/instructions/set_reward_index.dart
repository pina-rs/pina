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
class SetRewardIndexInstructionData {
  const SetRewardIndexInstructionData({required this.newIndex})
    : discriminator = 5,
      migrationVersion = 0;

  final int discriminator;
  final int migrationVersion;
  final BigInt newIndex;
}

Encoder<SetRewardIndexInstructionData>
getSetRewardIndexInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
    ('migrationVersion', getU8Encoder()),
    ('newIndex', getU64Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (SetRewardIndexInstructionData value) => <String, Object?>{
      'discriminator': 5,
      'migrationVersion': 0,
      'newIndex': value.newIndex,
    },
  );
}

Decoder<SetRewardIndexInstructionData>
getSetRewardIndexInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
    ('migrationVersion', getU8Decoder()),
    ('newIndex', getU64Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'setRewardIndex instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (SetRewardIndexInstructionData, int) readTopLevel(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(5)).read(bytes, offset + 0);
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 1);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (
      SetRewardIndexInstructionData(newIndex: map['newIndex']! as BigInt),
      newOffset,
    );
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<SetRewardIndexInstructionData>(
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
      VariableSizeDecoder<SetRewardIndexInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<SetRewardIndexInstructionData, SetRewardIndexInstructionData>
getSetRewardIndexInstructionDataCodec() {
  return combineCodec(
    getSetRewardIndexInstructionDataEncoder(),
    getSetRewardIndexInstructionDataDecoder(),
  );
}

/// Creates a [SetRewardIndex] instruction.
Instruction getSetRewardIndexInstruction({
  required Address programAddress,
  required Address admin,
  required Address poolState,
  required BigInt newIndex,
}) {
  final instructionData = SetRewardIndexInstructionData(newIndex: newIndex);

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: admin, role: AccountRole.readonlySigner),
      AccountMeta(address: poolState, role: AccountRole.writable),
    ],
    data: getSetRewardIndexInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [SetRewardIndex] instruction from raw instruction data.
SetRewardIndexInstructionData parseSetRewardIndexInstruction(
  Instruction instruction,
) {
  return getSetRewardIndexInstructionDataDecoder().decode(instruction.data!);
}
