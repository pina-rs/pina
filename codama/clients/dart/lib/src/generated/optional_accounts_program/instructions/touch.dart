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
class TouchInstructionData {
  const TouchInstructionData() : discriminator = 1;

  final int discriminator;
}

Encoder<TouchInstructionData> getTouchInstructionDataEncoder() {
  final structEncoder = getStructEncoder(<(String, Encoder<Object?>)>[
    ('discriminator', getU8Encoder()),
  ]);

  return transformEncoder(
    structEncoder,
    (TouchInstructionData value) => <String, Object?>{'discriminator': 1},
  );
}

Decoder<TouchInstructionData> getTouchInstructionDataDecoder() {
  final structDecoder = getStructDecoder(<(String, Decoder<Object?>)>[
    ('discriminator', getU8Decoder()),
  ]);

  Never throwInvalidByteLength(int expected, int bytesLength) {
    throw SolanaError(SolanaErrorCode.codecsInvalidByteLength, {
      'codecDescription': 'touch instruction decoder',
      'expected': expected,
      'bytesLength': bytesLength,
    });
  }

  (TouchInstructionData, int) readTopLevel(Uint8List bytes, int offset) {
    getConstantDecoder(getU8Encoder().encode(1)).read(bytes, offset + 0);
    final (map, newOffset) = structDecoder.read(bytes, offset);
    if (newOffset != bytes.length) {
      throwInvalidByteLength(newOffset - offset, bytes.length - offset);
    }

    return (TouchInstructionData(), newOffset);
  }

  return switch (structDecoder) {
    FixedSizeDecoder<Map<String, Object?>>() =>
      FixedSizeDecoder<TouchInstructionData>(
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
      VariableSizeDecoder<TouchInstructionData>(
        read: readTopLevel,
        maxSize: structDecoder.maxSize,
      ),
  };
}

Codec<TouchInstructionData, TouchInstructionData>
getTouchInstructionDataCodec() {
  return combineCodec(
    getTouchInstructionDataEncoder(),
    getTouchInstructionDataDecoder(),
  );
}

/// Creates a [Touch] instruction.
Instruction getTouchInstruction({
  required Address programAddress,
  required Address authority,
  Address? store,
}) {
  final instructionData = TouchInstructionData();

  return Instruction(
    programAddress: programAddress,
    accounts: [
      AccountMeta(address: authority, role: AccountRole.readonlySigner),
      if (store != null)
        AccountMeta(address: store, role: AccountRole.writable)
      else
        AccountMeta(address: programAddress, role: AccountRole.readonly),
    ],
    data: getTouchInstructionDataEncoder().encode(instructionData),
  );
}

/// Parses a [Touch] instruction from raw instruction data.
TouchInstructionData parseTouchInstruction(Instruction instruction) {
  return getTouchInstructionDataDecoder().decode(instruction.data!);
}
