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
class HelloNoMsgInstructionData {
  const HelloNoMsgInstructionData() : discriminator = 1;

  final int discriminator;
}

Encoder<HelloNoMsgInstructionData> getHelloNoMsgInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (HelloNoMsgInstructionData value) => <String, Object?>{'discriminator': 1},
  );
}

Decoder<HelloNoMsgInstructionData> getHelloNoMsgInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'helloNoMsg instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (HelloNoMsgInstructionData, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (HelloNoMsgInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<HelloNoMsgInstructionData>(
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
      VariableSizeDecoder<HelloNoMsgInstructionData>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<HelloNoMsgInstructionData, HelloNoMsgInstructionData>
getHelloNoMsgInstructionDataCodec() {
  return combineCodec(
    getHelloNoMsgInstructionDataEncoder(),
    getHelloNoMsgInstructionDataDecoder(),
  );
}

/// Creates a [HelloNoMsg] instruction.
Instruction getHelloNoMsgInstruction({required Address programAddress}) {
  final instructionData = HelloNoMsgInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [],
    data: getHelloNoMsgInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [HelloNoMsg] instruction from raw instruction data.
HelloNoMsgInstructionData parseHelloNoMsgInstruction(Instruction instruction) {
  return getHelloNoMsgInstructionDataDecoder().decode(instruction.data!);
}
