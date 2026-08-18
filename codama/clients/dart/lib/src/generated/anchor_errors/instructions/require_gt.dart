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
class RequireGtInstructionData {
  const RequireGtInstructionData() : discriminator = 5;

  final int discriminator;
}

Encoder<RequireGtInstructionData> getRequireGtInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (RequireGtInstructionData value) => <String, Object?>{'discriminator': 5},
  );
}

Decoder<RequireGtInstructionData> getRequireGtInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'requireGt instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (RequireGtInstructionData, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(5)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (RequireGtInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<RequireGtInstructionData>(
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
      VariableSizeDecoder<RequireGtInstructionData>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<RequireGtInstructionData, RequireGtInstructionData>
getRequireGtInstructionDataCodec() {
  return combineCodec(
    getRequireGtInstructionDataEncoder(),
    getRequireGtInstructionDataDecoder(),
  );
}

/// Creates a [RequireGt] instruction.
Instruction getRequireGtInstruction({required Address programAddress}) {
  final instructionData = RequireGtInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [],
    data: getRequireGtInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [RequireGt] instruction from raw instruction data.
RequireGtInstructionData parseRequireGtInstruction(Instruction instruction) {
  return getRequireGtInstructionDataDecoder().decode(instruction.data!);
}
