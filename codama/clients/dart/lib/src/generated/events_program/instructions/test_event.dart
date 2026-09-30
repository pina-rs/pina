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
class TestEventInstructionData {
  const TestEventInstructionData() : discriminator = 1;

  final int discriminator;
}

Encoder<TestEventInstructionData> getTestEventInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (TestEventInstructionData value) => <String, Object?>{'discriminator': 1},
  );
}

Decoder<TestEventInstructionData> getTestEventInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'testEvent instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (TestEventInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (TestEventInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<TestEventInstructionData>(
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
      VariableSizeDecoder<TestEventInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<TestEventInstructionData, TestEventInstructionData>
getTestEventInstructionDataCodec() {
  return combineCodec(
    getTestEventInstructionDataEncoder(),
    getTestEventInstructionDataDecoder(),
  );
}

/// Creates a [TestEvent] instruction.
Instruction getTestEventInstruction({required Address programAddress}) {
  final instructionData = TestEventInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [],
    data: getTestEventInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [TestEvent] instruction from raw instruction data.
TestEventInstructionData parseTestEventInstruction(Instruction instruction) {
  return getTestEventInstructionDataDecoder().decode(instruction.data!);
}
