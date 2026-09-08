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
class RequireGteInstructionData {
  const RequireGteInstructionData() : discriminator = 6;

  final int discriminator;
}

Encoder<RequireGteInstructionData> getRequireGteInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (RequireGteInstructionData value) => <String, Object?>{'discriminator': 6},
  );
}

Decoder<RequireGteInstructionData> getRequireGteInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'requireGte instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (RequireGteInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(6)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (RequireGteInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<RequireGteInstructionData>(
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
      VariableSizeDecoder<RequireGteInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<RequireGteInstructionData, RequireGteInstructionData>
getRequireGteInstructionDataCodec() {
  return combineCodec(
    getRequireGteInstructionDataEncoder(),
    getRequireGteInstructionDataDecoder(),
  );
}

/// Creates a [RequireGte] instruction.
Instruction getRequireGteInstruction({required Address programAddress}) {
  final instructionData = RequireGteInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [],
    data: getRequireGteInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [RequireGte] instruction from raw instruction data.
RequireGteInstructionData parseRequireGteInstruction(Instruction instruction) {
  return getRequireGteInstructionDataDecoder().decode(instruction.data!);
}
