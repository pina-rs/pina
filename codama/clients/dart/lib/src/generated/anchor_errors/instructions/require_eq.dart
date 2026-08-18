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
class RequireEqInstructionData {
  const RequireEqInstructionData() : discriminator = 3;

  final int discriminator;
}

Encoder<RequireEqInstructionData> getRequireEqInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (RequireEqInstructionData value) => <String, Object?>{'discriminator': 3},
  );
}

Decoder<RequireEqInstructionData> getRequireEqInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'requireEq instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (RequireEqInstructionData, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(3)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (RequireEqInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<RequireEqInstructionData>(
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
      VariableSizeDecoder<RequireEqInstructionData>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<RequireEqInstructionData, RequireEqInstructionData>
getRequireEqInstructionDataCodec() {
  return combineCodec(
    getRequireEqInstructionDataEncoder(),
    getRequireEqInstructionDataDecoder(),
  );
}

/// Creates a [RequireEq] instruction.
Instruction getRequireEqInstruction({required Address programAddress}) {
  final instructionData = RequireEqInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [],
    data: getRequireEqInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [RequireEq] instruction from raw instruction data.
RequireEqInstructionData parseRequireEqInstruction(Instruction instruction) {
  return getRequireEqInstructionDataDecoder().decode(instruction.data!);
}
