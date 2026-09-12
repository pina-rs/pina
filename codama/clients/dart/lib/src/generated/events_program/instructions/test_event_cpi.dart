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
class TestEventCpiInstructionData {
  const TestEventCpiInstructionData() : discriminator = 2;

  final int discriminator;
}

Encoder<TestEventCpiInstructionData> getTestEventCpiInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (TestEventCpiInstructionData value) => <String, Object?>{
      'discriminator': 2,
    },
  );
}

Decoder<TestEventCpiInstructionData> getTestEventCpiInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'testEventCpi instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (TestEventCpiInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (TestEventCpiInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<TestEventCpiInstructionData>(
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
      VariableSizeDecoder<TestEventCpiInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<TestEventCpiInstructionData, TestEventCpiInstructionData>
getTestEventCpiInstructionDataCodec() {
  return combineCodec(
    getTestEventCpiInstructionDataEncoder(),
    getTestEventCpiInstructionDataDecoder(),
  );
}

/// Creates a [TestEventCpi] instruction.
Instruction getTestEventCpiInstruction({required Address programAddress}) {
  final instructionData = TestEventCpiInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [],
    data: getTestEventCpiInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [TestEventCpi] instruction from raw instruction data.
TestEventCpiInstructionData parseTestEventCpiInstruction(
  Instruction instruction,
) {
  return getTestEventCpiInstructionDataDecoder().decode(instruction.data!);
}
