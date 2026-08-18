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
class RequireNeqInstructionData {
  const RequireNeqInstructionData() : discriminator = 4;

  final int discriminator;
}

Encoder<RequireNeqInstructionData> getRequireNeqInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (RequireNeqInstructionData value) => <String, Object?>{'discriminator': 4},
  );
}

Decoder<RequireNeqInstructionData> getRequireNeqInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'requireNeq instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (RequireNeqInstructionData, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(4)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (RequireNeqInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<RequireNeqInstructionData>(
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
      VariableSizeDecoder<RequireNeqInstructionData>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<RequireNeqInstructionData, RequireNeqInstructionData>
getRequireNeqInstructionDataCodec() {
  return combineCodec(
    getRequireNeqInstructionDataEncoder(),
    getRequireNeqInstructionDataDecoder(),
  );
}

/// Creates a [RequireNeq] instruction.
Instruction getRequireNeqInstruction({required Address programAddress}) {
  final instructionData = RequireNeqInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [],
    data: getRequireNeqInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [RequireNeq] instruction from raw instruction data.
RequireNeqInstructionData parseRequireNeqInstruction(Instruction instruction) {
  return getRequireNeqInstructionDataDecoder().decode(instruction.data!);
}
