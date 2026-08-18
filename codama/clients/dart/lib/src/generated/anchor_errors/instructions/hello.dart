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
class HelloInstructionData {
  const HelloInstructionData() : discriminator = 0;

  final int discriminator;
}

Encoder<HelloInstructionData> getHelloInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (HelloInstructionData value) => <String, Object?>{'discriminator': 0},
  );
}

Decoder<HelloInstructionData> getHelloInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'hello instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (HelloInstructionData, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(0)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (HelloInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<HelloInstructionData>(
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
      VariableSizeDecoder<HelloInstructionData>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<HelloInstructionData, HelloInstructionData>
getHelloInstructionDataCodec() {
  return combineCodec(
    getHelloInstructionDataEncoder(),
    getHelloInstructionDataDecoder(),
  );
}

/// Creates a [Hello] instruction.
Instruction getHelloInstruction({required Address programAddress}) {
  final instructionData = HelloInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [],
    data: getHelloInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Hello] instruction from raw instruction data.
HelloInstructionData parseHelloInstruction(Instruction instruction) {
  return getHelloInstructionDataDecoder().decode(instruction.data!);
}
