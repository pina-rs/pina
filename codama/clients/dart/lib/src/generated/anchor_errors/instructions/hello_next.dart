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
class HelloNextInstructionData {
  const HelloNextInstructionData() : discriminator = 2;

  final int discriminator;
}

Encoder<HelloNextInstructionData> getHelloNextInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (HelloNextInstructionData value) => <String, Object?>{'discriminator': 2},
  );
}

Decoder<HelloNextInstructionData> getHelloNextInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'helloNext instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (HelloNextInstructionData, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(2)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (HelloNextInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<HelloNextInstructionData>(
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
      VariableSizeDecoder<HelloNextInstructionData>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<HelloNextInstructionData, HelloNextInstructionData>
getHelloNextInstructionDataCodec() {
  return combineCodec(
    getHelloNextInstructionDataEncoder(),
    getHelloNextInstructionDataDecoder(),
  );
}

/// Creates a [HelloNext] instruction.
Instruction getHelloNextInstruction({required Address programAddress}) {
  final instructionData = HelloNextInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [],
    data: getHelloNextInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [HelloNext] instruction from raw instruction data.
HelloNextInstructionData parseHelloNextInstruction(Instruction instruction) {
  return getHelloNextInstructionDataDecoder().decode(instruction.data!);
}
