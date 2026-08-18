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
class IncrementInstructionData {
  const IncrementInstructionData() : discriminator = 1;

  final int discriminator;
}

Encoder<IncrementInstructionData> getIncrementInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (IncrementInstructionData value) => <String, Object?>{'discriminator': 1},
  );
}

Decoder<IncrementInstructionData> getIncrementInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'increment instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (IncrementInstructionData, int) readExact(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }
    return (IncrementInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<IncrementInstructionData>(
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
      VariableSizeDecoder<IncrementInstructionData>(
        read: readExact,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<IncrementInstructionData, IncrementInstructionData>
getIncrementInstructionDataCodec() {
  return combineCodec(
    getIncrementInstructionDataEncoder(),
    getIncrementInstructionDataDecoder(),
  );
}

/// Creates a [Increment] instruction.
Instruction getIncrementInstruction({
  required Address programAddress,
  required Address authority,
  required Address counter,
}) {
  final instructionData = IncrementInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: authority, role: AccountRole.readonlySigner),
      AccountMeta(address: counter, role: AccountRole.writable),
    ],
    data: getIncrementInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Increment] instruction from raw instruction data.
IncrementInstructionData parseIncrementInstruction(Instruction instruction) {
  return getIncrementInstructionDataDecoder().decode(instruction.data!);
}
