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
class ToggleCompletedInstructionData {
  const ToggleCompletedInstructionData() : discriminator = 1;

  final int discriminator;
}

Encoder<ToggleCompletedInstructionData>
getToggleCompletedInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (ToggleCompletedInstructionData value) => <String, Object?>{
      'discriminator': 1,
    },
  );
}

Decoder<ToggleCompletedInstructionData>
getToggleCompletedInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'toggleCompleted instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (ToggleCompletedInstructionData, int) readTopLevel(
    Uint8List bytes,
    int offset,
  ) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (ToggleCompletedInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<ToggleCompletedInstructionData>(
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
      VariableSizeDecoder<ToggleCompletedInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<ToggleCompletedInstructionData, ToggleCompletedInstructionData>
getToggleCompletedInstructionDataCodec() {
  return combineCodec(
    getToggleCompletedInstructionDataEncoder(),
    getToggleCompletedInstructionDataDecoder(),
  );
}

/// Creates a [ToggleCompleted] instruction.
Instruction getToggleCompletedInstruction({
  required Address programAddress,
  required Address owner,
  required Address todo,
}) {
  final instructionData = ToggleCompletedInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: owner, role: AccountRole.readonlySigner),
      AccountMeta(address: todo, role: AccountRole.writable),
    ],
    data: getToggleCompletedInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [ToggleCompleted] instruction from raw instruction data.
ToggleCompletedInstructionData parseToggleCompletedInstruction(
  Instruction instruction,
) {
  return getToggleCompletedInstructionDataDecoder().decode(instruction.data!);
}
